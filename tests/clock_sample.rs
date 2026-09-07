use tracker_core::clock::ClockSample;

/// Zegar zdalny wyprzedza lokalny o 0,5 s, opoznienie w obie strony po 10 ms,
/// przetwarzanie po stronie zdalnej 2 ms.
#[test]
fn offset_and_delay_for_symmetric_path() {
    let s = ClockSample::new(100.000, 100.510, 100.512, 100.022);

    assert!((s.offset() - 0.5).abs() < 1e-9, "offset = {}", s.offset());
    assert!((s.delay() - 0.020).abs() < 1e-9, "delay = {}", s.delay());
    assert!((s.local_time() - 100.011).abs() < 1e-9);
}

/// Asymetria sciezki przenosi sie na blad offsetu rowny polowie roznicy opoznien.
#[test]
fn asymmetric_path_biases_offset_by_half_the_difference() {
    // droga tam 30 ms, droga z powrotem 10 ms, offset prawdziwy 0
    let s = ClockSample::new(0.000, 0.030, 0.031, 0.041);

    assert!((s.offset() - 0.010).abs() < 1e-9, "offset = {}", s.offset());
    assert!((s.delay() - 0.040).abs() < 1e-9);
}

#[test]
fn zero_offset_for_identical_clocks() {
    let s = ClockSample::new(10.0, 10.005, 10.006, 10.011);

    assert!(s.offset().abs() < 1e-9);
    assert!((s.delay() - 0.010).abs() < 1e-9);
}
