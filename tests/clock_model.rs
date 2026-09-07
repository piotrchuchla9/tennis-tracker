use tracker_core::clock::{ClockFit, ClockModel};

fn sample_fit() -> ClockFit {
    ClockFit {
        reference_time: 1000.0,
        offset_seconds: 0.25,
        skew_ppm: 20.0,
        residual_std_ms: 0.8,
        sample_count: 75,
    }
}

#[test]
fn peer_time_at_reference_is_local_plus_offset() {
    let model = ClockModel::new(sample_fit());
    assert!((model.peer_time(1000.0) - 1000.25).abs() < 1e-9);
}

#[test]
fn skew_accumulates_over_time() {
    let model = ClockModel::new(sample_fit());
    // 20 ppm przez 3600 s to 72 ms
    let expected = 4600.25 + 0.072;
    assert!((model.peer_time(4600.0) - expected).abs() < 1e-9);
}

#[test]
fn round_trip_is_exact() {
    let model = ClockModel::new(sample_fit());
    let mut local = 900.0;
    while local <= 5000.0 {
        let peer = model.peer_time(local);
        assert!(
            (model.local_time(peer) - local).abs() < 1e-6,
            "local = {}",
            local
        );
        local += 137.0;
    }
}

#[test]
fn zero_skew_behaves_as_constant_offset() {
    let flat = ClockFit {
        reference_time: 0.0,
        offset_seconds: -1.5,
        skew_ppm: 0.0,
        residual_std_ms: 0.1,
        sample_count: 20,
    };
    let model = ClockModel::new(flat);
    assert!((model.peer_time(12345.0) - 12343.5).abs() < 1e-9);
    assert!((model.local_time(12343.5) - 12345.0).abs() < 1e-9);
}
