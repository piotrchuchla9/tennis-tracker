mod support;

use std::sync::Arc;
use support::{FakeCapture, FakeStorage, FakeThermal};
use tracker_core::clock::{ClockFit, SyncGap, SyncSnapshot};
use tracker_core::device::{CapabilityProbeResult, TimestampSource};
use tracker_core::session::{
    CaptureProfile, DeviceInfo, MatchSetup, Platform, RecorderConfig, RecorderState, SessionEvent,
    SessionRecorder, SessionRole, StopReason,
};

struct Harness {
    capture: Arc<FakeCapture>,
    storage: Arc<FakeStorage>,
    thermal: Arc<FakeThermal>,
    recorder: SessionRecorder,
}

fn snapshot() -> SyncSnapshot {
    SyncSnapshot {
        model: ClockFit {
            reference_time: 1000.0,
            offset_seconds: 0.014,
            skew_ppm: 7.3,
            residual_std_ms: 0.9,
            sample_count: 60,
        },
        samples: vec![],
        gaps: vec![],
    }
}

fn harness(role: SessionRole) -> Harness {
    let capture = Arc::new(FakeCapture::new());
    let storage = Arc::new(FakeStorage::new());
    let thermal = Arc::new(FakeThermal::new());
    let capabilities = CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
    .evaluate();

    let recorder = SessionRecorder::new(
        role,
        Platform::Ios,
        DeviceInfo {
            model: "iPhone17,2".into(),
            os_version: "26.0".into(),
            app_version: "0.1.0".into(),
        },
        Some("iPhone15,4".into()),
        MatchSetup::default_singles(),
        capabilities,
        "multipeer".into(),
        capture.clone(),
        storage.clone(),
        thermal.clone(),
        RecorderConfig::default(),
    );

    Harness { capture, storage, thermal, recorder }
}

fn started(role: SessionRole, now: f64) -> Harness {
    let mut h = harness(role);
    h.recorder.arm().unwrap();
    h.recorder.start("s-1", CaptureProfile::P1080p120, now).unwrap();
    h
}

// --- przejscia stanow ---

#[test]
fn starts_idle() {
    assert_eq!(harness(SessionRole::Master).recorder.state(), RecorderState::Idle);
}

#[test]
fn arm_locks_camera_settings() {
    let mut h = harness(SessionRole::Master);
    h.recorder.arm().unwrap();
    assert_eq!(h.recorder.state(), RecorderState::Armed);
    assert!(h.capture.did_lock());
}

#[test]
fn start_requires_armed_state() {
    let mut h = harness(SessionRole::Master);
    assert!(h.recorder.start("s-1", CaptureProfile::P1080p120, 1000.0).is_err());
}

#[test]
fn lock_failure_leaves_recorder_idle() {
    let mut h = harness(SessionRole::Master);
    h.capture.set_lock_should_fail(true);
    assert!(h.recorder.arm().is_err());
    assert_eq!(h.recorder.state(), RecorderState::Idle);
}

#[test]
fn start_moves_to_recording() {
    let h = started(SessionRole::Master, 1000.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
    assert!(h.capture.did_start());
}

// --- segmentacja ---

#[test]
fn segment_rolls_after_configured_duration() {
    let mut h = started(SessionRole::Master, 1000.0);

    h.recorder.tick(1200.0);
    assert_eq!(h.capture.roll_count(), 0);

    h.recorder.tick(1300.1);
    assert_eq!(h.capture.roll_count(), 1);

    h.recorder.tick(1600.2);
    assert_eq!(h.capture.roll_count(), 2);
}

// --- awarie ---

#[test]
fn critical_thermal_stops_session() {
    let mut h = started(SessionRole::Master, 1000.0);

    h.thermal.set_level(tracker_core::session::ThermalLevel::Serious);
    h.recorder.tick(1010.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);

    h.thermal.set_level(tracker_core::session::ThermalLevel::Critical);
    h.recorder.tick(1020.0);
    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::ThermalCritical });
    assert!(h.capture.did_stop());
}

#[test]
fn serious_thermal_is_recorded_as_event() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.thermal.set_level(tracker_core::session::ThermalLevel::Serious);
    h.recorder.tick(1010.0);

    let manifest = h.recorder.build_manifest(1010.0, &snapshot()).unwrap();
    assert!(manifest
        .events
        .iter()
        .any(|e| e.event_type == "thermal" && e.state.as_deref() == Some("serious")));
}

#[test]
fn low_storage_stops_session() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.storage.set_free_bytes(100_000_000);
    h.recorder.tick(1010.0);

    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::StorageExhausted });
}

#[test]
fn arm_rejects_insufficient_storage() {
    let mut h = harness(SessionRole::Master);
    h.storage.set_free_bytes(10_000_000);
    assert!(h.recorder.arm().is_err());
}

#[test]
fn slave_stops_after_peer_silence_timeout() {
    let mut h = started(SessionRole::Slave, 1000.0);

    h.recorder.note_peer_heartbeat(1050.0);
    h.recorder.tick(1150.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);

    h.recorder.tick(1180.0);
    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::PeerSilence });
}

#[test]
fn master_ignores_peer_silence() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_peer_heartbeat(1000.0);
    h.recorder.tick(2000.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
}

#[test]
fn link_loss_does_not_stop_recording() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_event(SessionEvent::link_lost(1100.0));
    h.recorder.tick(1110.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
}

// --- manifest ---

#[test]
fn stop_produces_valid_manifest() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.tick(1300.1);
    let manifest = h.recorder.stop(1450.0, &snapshot()).unwrap();

    assert!(manifest.validate().is_ok(), "manifest niepoprawny: {:?}", manifest.validate());
    assert_eq!(manifest.session_id, "s-1");
    assert_eq!(manifest.role, SessionRole::Master);
    assert_eq!(manifest.platform, Platform::Ios);
    assert_eq!(manifest.camera.profile, CaptureProfile::P1080p120);
    assert_eq!(manifest.camera.fps, 120);
    assert_eq!(manifest.segments.len(), 2);
    assert_eq!(manifest.timing.clock_domain, "mach_absolute_time");
    assert_eq!(manifest.timing.transport, "multipeer");
    assert_eq!(manifest.motion_log, "motion.jsonl");
    assert_eq!(manifest.schema_version, 2);
}

#[test]
fn manifest_carries_sync_model_and_gaps() {
    let mut h = started(SessionRole::Master, 1000.0);
    let mut snap = snapshot();
    snap.gaps.push(SyncGap {
        start_host_time: 1100.0,
        end_host_time: Some(1160.0),
        reason: "link-lost".into(),
    });

    let manifest = h.recorder.stop(1200.0, &snap).unwrap();
    assert!((manifest.timing.sync_model.skew_ppm - 7.3).abs() < 1e-9);
    assert_eq!(manifest.timing.sync_gaps.len(), 1);
    assert_eq!(manifest.timing.sync_gaps[0].reason, "link-lost");
}

#[test]
fn stop_is_idempotent_after_automatic_stop() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.thermal.set_level(tracker_core::session::ThermalLevel::Critical);
    h.recorder.tick(1010.0);

    let manifest = h.recorder.stop(1020.0, &snapshot()).unwrap();
    assert!(manifest.validate().is_ok());
    assert_eq!(h.capture.roll_count(), 0);
}

#[test]
fn marks_from_the_watch_reach_the_manifest() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_event(SessionEvent::mark(1100.0, "interesting"));
    h.recorder.note_event(SessionEvent::mark(1200.0, "changeover"));

    let manifest = h.recorder.stop(1300.0, &snapshot()).unwrap();
    let tags: Vec<&str> = manifest
        .events
        .iter()
        .filter(|e| e.event_type == "mark")
        .filter_map(|e| e.tag.as_deref())
        .collect();
    assert_eq!(tags, vec!["interesting", "changeover"]);
}
