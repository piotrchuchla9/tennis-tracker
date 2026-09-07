use tracker_core::device::{
    estimate_timestamp_base_offset, CapabilityProbeResult, DeviceTier, TimestampSource,
};

fn ios_probe() -> CapabilityProbeResult {
    CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
}

#[test]
fn full_tier_for_capable_device() {
    let report = ios_probe().evaluate();
    assert_eq!(report.tier, DeviceTier::Full);
    assert_eq!(report.max_fps, 120);
    assert!(report.intrinsics_available);
}

#[test]
fn missing_manual_sensor_drops_to_limited() {
    let probe = CapabilityProbeResult { manual_sensor: false, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn unknown_timestamp_source_drops_to_limited() {
    let probe = CapabilityProbeResult {
        timestamp_source: TimestampSource::Unknown,
        ..ios_probe()
    };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn missing_exposure_lock_is_rejected() {
    let probe = CapabilityProbeResult { manual_sensor: false, exposure_lock: false, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Rejected);
}

#[test]
fn too_few_frames_per_second_is_rejected() {
    let probe = CapabilityProbeResult { max_fps: 24, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Rejected);
}

#[test]
fn exposure_too_long_drops_to_limited() {
    // sensor nie schodzi ponizej 1/250 s
    let probe = CapabilityProbeResult { min_exposure_seconds: 0.004, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn tier_strings_match_spec_vocabulary() {
    assert_eq!(DeviceTier::Full.as_str(), "full");
    assert_eq!(DeviceTier::Limited.as_str(), "limited");
    assert_eq!(DeviceTier::Rejected.as_str(), "rejected");
}

#[test]
fn realtime_source_needs_no_base_offset() {
    let report = ios_probe().evaluate();
    assert_eq!(report.timestamp_base_offset_ms, None);
    assert_eq!(report.timestamp_base_uncertainty_ms, None);
}

#[test]
fn base_offset_is_estimated_from_minimum_delivery_lag() {
    // znacznik klatki lezy 2,0 s ponizej zegara monotonicznego,
    // a opoznienie dostarczenia waha sie od 5 do 40 ms
    let pairs = vec![
        (100.000, 102.040),
        (100.008, 102.013),
        (100.016, 102.021),
        (100.024, 102.029),
        (100.032, 102.037),
        (100.040, 102.045),
    ];
    let (offset_ms, uncertainty_ms) =
        estimate_timestamp_base_offset(&pairs).expect("estymacja powinna sie udac");

    // minimum roznicy to 2,005 s
    assert!((offset_ms - 2005.0).abs() < 1.0, "offset = {offset_ms}");
    assert!(uncertainty_ms > 0.0);
}

#[test]
fn base_offset_needs_enough_pairs() {
    assert!(estimate_timestamp_base_offset(&[(1.0, 2.0)]).is_none());
    assert!(estimate_timestamp_base_offset(&[]).is_none());
}
