/// Wynik dopasowania liniowego modelu zegara.
///
/// Model: `offset(t) = offset_seconds + skew_ppm * 1e-6 * (t - reference_time)`,
/// gdzie `t` jest czasem lokalnym w sekundach.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct ClockFit {
    pub reference_time: f64,
    pub offset_seconds: f64,
    pub skew_ppm: f64,
    pub residual_std_ms: f64,
    pub sample_count: u32,
}

impl ClockFit {
    /// Model zerowy, uzywany gdy synchronizacja nie zdazyla sie ustalic.
    /// Nieskonczony rezyduał jasno sygnalizuje brak wiarygodnosci.
    pub fn unknown() -> Self {
        Self {
            reference_time: 0.0,
            offset_seconds: 0.0,
            skew_ppm: 0.0,
            residual_std_ms: f64::INFINITY,
            sample_count: 0,
        }
    }
}
