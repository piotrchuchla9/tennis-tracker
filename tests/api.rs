mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::api::{capability_report, default_match_setup, estimate_base_offset, SyncEngine};
use tracker_core::device::{CapabilityProbeResult, DeviceTier, TimestampSource};

#[test]
fn sync_engine_runs_a_full_burst_and_reports_quality() {
    let transport = Arc::new(FakeTransport::new());
    let engine = SyncEngine::new(transport.clone());

    engine.start(1000.0);
    for message in transport.sent_messages() {
        if let tracker_core::link::LinkMessage::Ping { id, t1 } = message {
            let t2 = t1 + 0.005 + 0.4;
            let t3 = t2 + 0.001;
            engine.handle_pong(id, t1, t2, t3, t3 - 0.4 + 0.005);
        }
    }

    let snapshot = engine.snapshot();
    assert!((snapshot.model.offset_seconds - 0.4).abs() < 0.001);
    assert!(snapshot.model.residual_std_ms < 2.0);
    assert_eq!(snapshot.samples.len(), 50);
}

#[test]
fn sync_engine_records_gaps() {
    let transport = Arc::new(FakeTransport::new());
    let engine = SyncEngine::new(transport);

    engine.start(1000.0);
    engine.link_did_change(false, 1100.0);
    engine.link_did_change(true, 1150.0);

    let snapshot = engine.snapshot();
    assert_eq!(snapshot.gaps.len(), 1);
    assert_eq!(snapshot.gaps[0].end_host_time, Some(1150.0));
}

#[test]
fn capability_report_is_reachable_through_the_facade() {
    let report = capability_report(CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    });
    assert_eq!(report.tier, DeviceTier::Full);
}

#[test]
fn base_offset_estimate_is_reachable_through_the_facade() {
    let frames = vec![100.000, 100.008, 100.016, 100.024, 100.032, 100.040];
    let delivered = vec![102.040, 102.013, 102.021, 102.029, 102.037, 102.045];
    let estimate = estimate_base_offset(frames, delivered).expect("estymacja powinna sie udac");
    assert!((estimate.offset_ms - 2005.0).abs() < 1.0);
}

#[test]
fn base_offset_estimate_rejects_mismatched_lengths() {
    assert!(estimate_base_offset(vec![1.0, 2.0], vec![1.0]).is_none());
}

#[test]
fn default_match_setup_is_valid() {
    assert!(default_match_setup().validate().is_ok());
}
