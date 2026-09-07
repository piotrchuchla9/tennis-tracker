mod support;

use support::{make_series, SeriesSpec};
use tracker_core::clock::{ClockSample, ClockSyncEstimator};

fn estimator_with(samples: &[ClockSample]) -> ClockSyncEstimator {
    let mut estimator = ClockSyncEstimator::new();
    for sample in samples {
        estimator.add(*sample);
    }
    estimator
}

#[test]
fn returns_none_below_minimum_sample_count() {
    let samples = make_series(&SeriesSpec {
        count: 5,
        ..Default::default()
    });
    assert!(estimator_with(&samples).fit().is_none());
}

#[test]
fn recovers_offset_with_no_skew() {
    let samples = make_series(&SeriesSpec {
        count: 60,
        offset0: 0.25,
        skew_ppm: 0.0,
        seed: 2,
        ..Default::default()
    });
    let fit = estimator_with(&samples)
        .fit()
        .expect("model powinien powstac");

    assert!(
        (fit.offset_seconds - 0.25).abs() < 0.0005,
        "offset = {}",
        fit.offset_seconds
    );
    assert!(fit.skew_ppm.abs() < 5.0, "skew = {}", fit.skew_ppm);
    assert!(
        fit.residual_std_ms < 1.0,
        "residual = {}",
        fit.residual_std_ms
    );
}

#[test]
fn recovers_skew_over_long_series() {
    // 20 ppm przez godzine to 72 ms rozjazdu
    let samples = make_series(&SeriesSpec {
        count: 3600,
        offset0: 0.10,
        skew_ppm: 20.0,
        seed: 3,
        ..Default::default()
    });
    let fit = estimator_with(&samples)
        .fit()
        .expect("model powinien powstac");

    assert!((fit.skew_ppm - 20.0).abs() < 1.0, "skew = {}", fit.skew_ppm);
}

#[test]
fn outliers_are_rejected_by_delay_filter() {
    // co dziesiata probka ma 250 ms dodatkowego opoznienia w jedna strone
    let samples = make_series(&SeriesSpec {
        count: 300,
        offset0: 0.25,
        skew_ppm: 5.0,
        seed: 4,
        outlier_every: Some(10),
        ..Default::default()
    });
    let fit = estimator_with(&samples)
        .fit()
        .expect("model powinien powstac");

    assert!(
        (fit.offset_seconds - 0.25).abs() < 0.002,
        "offset = {}",
        fit.offset_seconds
    );
    assert!((fit.skew_ppm - 5.0).abs() < 2.0, "skew = {}", fit.skew_ppm);
}

#[test]
fn survives_gap_in_series() {
    // 60 probek, przerwa 300 s, kolejne 60 probek
    let first = make_series(&SeriesSpec {
        start: 1000.0,
        count: 60,
        offset0: 0.10,
        skew_ppm: 10.0,
        seed: 5,
        ..Default::default()
    });
    let second = make_series(&SeriesSpec {
        start: 1360.0,
        count: 60,
        offset0: 0.10 + 10.0 * 1e-6 * 360.0,
        skew_ppm: 10.0,
        seed: 6,
        ..Default::default()
    });
    let all: Vec<_> = first.into_iter().chain(second).collect();
    let fit = estimator_with(&all).fit().expect("model powinien powstac");

    assert!((fit.skew_ppm - 10.0).abs() < 1.5, "skew = {}", fit.skew_ppm);
}

#[test]
fn window_drops_oldest_samples() {
    let mut estimator = ClockSyncEstimator::new();
    estimator.window_size = 50;
    for sample in make_series(&SeriesSpec {
        count: 200,
        offset0: 0.10,
        seed: 7,
        ..Default::default()
    }) {
        estimator.add(sample);
    }

    let fit = estimator.fit().expect("model powinien powstac");
    assert!(
        fit.sample_count <= 50,
        "sample_count = {}",
        fit.sample_count
    );
    // punkt odniesienia musi lezec w ostatnim oknie, nie na poczatku serii
    assert!(
        fit.reference_time > 1140.0,
        "reference = {}",
        fit.reference_time
    );
}

#[test]
fn reset_clears_samples() {
    let mut estimator = estimator_with(&make_series(&SeriesSpec::default()));
    assert!(estimator.count() > 0);
    estimator.reset();
    assert_eq!(estimator.count(), 0);
    assert!(estimator.fit().is_none());
}
