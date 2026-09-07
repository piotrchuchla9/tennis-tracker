use tracker_core::clock::{ClockFit, ClockSample, SyncGap};
use tracker_core::device::{CapabilityProbeResult, TimestampSource};
use tracker_core::session::{
    AudioInfo, CameraInfo, CaptureProfile, DeviceInfo, LockedCameraSettings, MatchSetup, Platform,
    SegmentInfo, SessionEvent, SessionManifest, SessionRole, SyncModel, SyncSampleRecord,
    TimingInfo, WhiteBalanceGains,
};

fn capabilities() -> tracker_core::device::CapabilityReport {
    CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
    .evaluate()
}

fn sample_manifest() -> SessionManifest {
    SessionManifest {
        schema_version: 2,
        session_id: "2026-09-14T17:32:10Z-a3f9".into(),
        role: SessionRole::Master,
        platform: Platform::Ios,
        peer_device_model: Some("iPhone15,4".into()),
        match_setup: MatchSetup::default_singles(),
        device: DeviceInfo {
            model: "iPhone17,2".into(),
            os_version: "26.0".into(),
            app_version: "0.1.0".into(),
        },
        capabilities: capabilities(),
        camera: CameraInfo::new(
            "builtInWideAngleCamera".into(),
            CaptureProfile::P1080p120,
            "hevc".into(),
            40,
            AudioInfo { sample_rate_hz: 48000, channels: 1, codec: "pcm".into() },
            LockedCameraSettings {
                exposure_duration_seconds: 0.001,
                iso: 64.0,
                focus_lens_position: 0.82,
                white_balance_gains: WhiteBalanceGains { r: 1.9, g: 1.0, b: 1.6 },
            },
            Some(vec![
                vec![1580.0, 0.0, 960.0],
                vec![0.0, 1580.0, 540.0],
                vec![0.0, 0.0, 1.0],
            ]),
        ),
        timing: TimingInfo {
            clock_domain: "mach_absolute_time".into(),
            first_frame_host_time: 123456.789012,
            transport: "multipeer".into(),
            sync_model: SyncModel::from_fit(&ClockFit {
                reference_time: 123456.0,
                offset_seconds: 0.0142,
                skew_ppm: 7.3,
                residual_std_ms: 0.9,
                sample_count: 60,
            }),
            sync_samples: vec![SyncSampleRecord::from_sample(&ClockSample::new(
                123456.0, 123456.019, 123456.020, 123456.006,
            ))],
            sync_gaps: vec![SyncGap {
                start_host_time: 124000.0,
                end_host_time: Some(124035.0),
                reason: "link-lost".into(),
            }],
        },
        segments: vec![SegmentInfo {
            file: "video-000.mov".into(),
            start_host_time: 123456.789,
            end_host_time: 123756.789,
            frame_count: 36000,
        }],
        events: vec![SessionEvent::motion_spike(123500.1, 0.42)],
        motion_log: "motion.jsonl".into(),
    }
}

#[test]
fn json_round_trip() {
    let original = sample_manifest();
    let decoded = SessionManifest::decode(&original.encode().unwrap()).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn player_genitive_survives_round_trip() {
    let mut m = sample_manifest();
    m.match_setup.players[0].genitive = Some("Piotra".into());

    let decoded = SessionManifest::decode(&m.encode().unwrap()).unwrap();
    assert_eq!(decoded.match_setup.players[0].genitive.as_deref(), Some("Piotra"));
}

#[test]
fn encoded_json_uses_spec_keys() {
    let json = sample_manifest().encode().unwrap();
    for key in [
        "schemaVersion", "sessionId", "platform", "peerDeviceModel", "match",
        "capabilities", "timestampSource", "intrinsicMatrix", "firstFrameHostTime",
        "transport", "syncModel", "referenceHostTime", "skewPpm", "residualStdMs",
        "syncGaps", "targetBitrateMbps", "motionLog", "firstServer",
    ] {
        assert!(json.contains(&format!("\"{key}\"")), "brak klucza {key}");
    }
}

#[test]
fn profile_and_tier_are_encoded_as_spec_strings() {
    let json = sample_manifest().encode().unwrap();
    assert!(json.contains("\"1080p120\""));
    assert!(json.contains("\"full\""));
    assert!(json.contains("\"REALTIME\""));
    assert!(json.contains("\"singles-ad-tiebreak\""));
}

#[test]
fn camera_geometry_is_derived_from_profile() {
    let manifest = sample_manifest();
    assert_eq!(manifest.camera.width, 1920);
    assert_eq!(manifest.camera.height, 1080);
    assert_eq!(manifest.camera.fps, 120);
}

#[test]
fn validate_accepts_well_formed_manifest() {
    assert!(sample_manifest().validate().is_ok());
}

#[test]
fn validate_rejects_wrong_schema_version() {
    let mut m = sample_manifest();
    m.schema_version = 1;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_empty_segments() {
    let mut m = sample_manifest();
    m.segments.clear();
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_non_contiguous_segments() {
    let mut m = sample_manifest();
    m.segments = vec![
        SegmentInfo { file: "video-000.mov".into(), start_host_time: 100.0, end_host_time: 400.0, frame_count: 36000 },
        SegmentInfo { file: "video-001.mov".into(), start_host_time: 420.0, end_host_time: 720.0, frame_count: 36000 },
    ];
    m.timing.first_frame_host_time = 100.0;
    assert!(m.validate().is_err());
}

#[test]
fn validate_accepts_segments_within_one_frame() {
    let mut m = sample_manifest();
    m.segments = vec![
        SegmentInfo { file: "video-000.mov".into(), start_host_time: 100.0, end_host_time: 400.0, frame_count: 36000 },
        // 4 ms przerwy przy 120 fps to mniej niz jedna klatka (8,3 ms)
        SegmentInfo { file: "video-001.mov".into(), start_host_time: 400.004, end_host_time: 700.0, frame_count: 36000 },
    ];
    m.timing.first_frame_host_time = 100.0;
    assert!(m.validate().is_ok());
}

#[test]
fn validate_rejects_first_frame_outside_first_segment() {
    let mut m = sample_manifest();
    m.timing.first_frame_host_time = 999_999.0;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_malformed_intrinsics() {
    let mut m = sample_manifest();
    m.camera.intrinsic_matrix = Some(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    assert!(m.validate().is_err());
}

#[test]
fn validate_accepts_absent_intrinsics() {
    let mut m = sample_manifest();
    m.camera.intrinsic_matrix = None;
    assert!(m.validate().is_ok(), "Android zwykle nie udostepnia intrinsics");
}

#[test]
fn validate_rejects_non_finite_sync_model() {
    let mut m = sample_manifest();
    m.timing.sync_model.residual_std_ms = f64::INFINITY;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_invalid_match_setup() {
    let mut m = sample_manifest();
    m.match_setup.first_server = "p9".into();
    assert!(m.validate().is_err());
}

#[test]
fn sync_model_maps_reference_time_to_host_time() {
    let model = SyncModel::from_fit(&ClockFit {
        reference_time: 500.0,
        offset_seconds: 0.1,
        skew_ppm: 3.0,
        residual_std_ms: 0.5,
        sample_count: 12,
    });
    assert!((model.reference_host_time - 500.0).abs() < 1e-9);
    assert!((model.skew_ppm - 3.0).abs() < 1e-9);
}

#[test]
fn sync_sample_record_derives_offset_and_delay() {
    let record = SyncSampleRecord::from_sample(&ClockSample::new(100.0, 100.51, 100.512, 100.022));
    assert!((record.offset_seconds - 0.5).abs() < 1e-9);
    assert!((record.delay_seconds - 0.020).abs() < 1e-9);
    assert!((record.host_time - 100.011).abs() < 1e-9);
}

#[test]
fn event_constructors_set_the_right_fields() {
    let spike = SessionEvent::motion_spike(10.0, 0.5);
    assert_eq!(spike.event_type, "motion-spike");
    assert_eq!(spike.magnitude, Some(0.5));

    let thermal = SessionEvent::thermal(11.0, "serious");
    assert_eq!(thermal.event_type, "thermal");
    assert_eq!(thermal.state.as_deref(), Some("serious"));

    let interrupted = SessionEvent::capture_interrupted(12.0, "system-pressure");
    assert_eq!(interrupted.event_type, "capture-interrupted");
    assert_eq!(interrupted.reason.as_deref(), Some("system-pressure"));

    let mark = SessionEvent::mark(13.0, "changeover");
    assert_eq!(mark.event_type, "mark");
    assert_eq!(mark.tag.as_deref(), Some("changeover"));
}
