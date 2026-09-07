use tracker_core::session::{MotionAnalyzer, MotionSample, MotionVector};

fn calm(host_time: f64) -> MotionSample {
    MotionSample {
        host_time,
        rotation_rate: MotionVector { x: 0.01, y: 0.005, z: 0.002 },
    }
}

fn spike(host_time: f64, magnitude: f64) -> MotionSample {
    MotionSample {
        host_time,
        rotation_rate: MotionVector { x: magnitude, y: 0.0, z: 0.0 },
    }
}

#[test]
fn calm_motion_produces_no_event() {
    let mut analyzer = MotionAnalyzer::new();
    for step in 0..100 {
        assert!(analyzer.process(&calm(step as f64 * 0.01)).is_none());
    }
}

#[test]
fn spike_produces_event_with_magnitude() {
    let mut analyzer = MotionAnalyzer::new();
    let event = analyzer.process(&spike(5.0, 1.2)).expect("oczekiwano zdarzenia");

    assert_eq!(event.event_type, "motion-spike");
    assert!((event.host_time - 5.0).abs() < 1e-9);
    assert!((event.magnitude.unwrap() - 1.2).abs() < 1e-6);
}

#[test]
fn refractory_period_suppresses_repeated_spikes() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.refractory_seconds = 1.0;

    assert!(analyzer.process(&spike(5.0, 1.2)).is_some());
    assert!(analyzer.process(&spike(5.2, 1.2)).is_none());
    assert!(analyzer.process(&spike(5.9, 1.2)).is_none());
    assert!(analyzer.process(&spike(6.1, 1.2)).is_some());
}

#[test]
fn threshold_is_configurable() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.rotation_threshold = 2.0;

    assert!(analyzer.process(&spike(1.0, 1.5)).is_none());
    assert!(analyzer.process(&spike(2.0, 2.5)).is_some());
}

#[test]
fn magnitude_uses_vector_norm() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.rotation_threshold = 0.35;

    let event = analyzer
        .process(&MotionSample {
            host_time: 1.0,
            rotation_rate: MotionVector { x: 0.3, y: 0.4, z: 0.0 },
        })
        .expect("oczekiwano zdarzenia");

    // norma (0.3, 0.4, 0) = 0.5
    assert!((event.magnitude.unwrap() - 0.5).abs() < 1e-9);
}

#[test]
fn vector_norm_is_euclidean() {
    let v = MotionVector { x: 3.0, y: 4.0, z: 12.0 };
    assert!((v.norm() - 13.0).abs() < 1e-9);
}
