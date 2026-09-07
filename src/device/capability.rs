/// Minimalna liczba par potrzebna do estymacji przesuniecia baz.
const MIN_TIMESTAMP_PAIRS: usize = 5;
/// Ponizej tylu klatek na sekunde nie ma sensu nagrywac.
const MIN_USABLE_FPS: u32 = 30;
/// Dluzsza ekspozycja rozmazuje pilke w kreske.
const MAX_USABLE_EXPOSURE_SECONDS: f64 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
pub enum TimestampSource {
    /// Znaczniki klatek leza na tej samej bazie co zegar monotoniczny.
    #[serde(rename = "REALTIME")]
    Realtime,
    /// Baza nieokreslona — wymaga estymacji przesuniecia.
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum DeviceTier {
    /// Wszystkie bramki spelnione; dane uzyteczne takze do stereo.
    Full,
    /// Nagrywa, ale z brakami odnotowanymi w manifescie.
    Limited,
    /// Ponizej progu uzytecznosci.
    Rejected,
}

impl DeviceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceTier::Full => "full",
            DeviceTier::Limited => "limited",
            DeviceTier::Rejected => "rejected",
        }
    }
}

/// Surowe odczyty z platformy. Wypelnia je adapter natywny.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CapabilityProbeResult {
    /// Manualna kontrola ekspozycji, ISO i czasu trwania klatki.
    pub manual_sensor: bool,
    /// Chociaz blokada automatycznej ekspozycji.
    pub exposure_lock: bool,
    pub focus_lock: bool,
    pub timestamp_source: TimestampSource,
    pub intrinsics_available: bool,
    pub max_fps: u32,
    pub min_exposure_seconds: f64,
}

/// Werdykt: co z tego urzadzenia da sie wycisnac.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    pub tier: DeviceTier,
    pub manual_sensor: bool,
    pub timestamp_source: TimestampSource,
    pub timestamp_base_offset_ms: Option<f64>,
    pub timestamp_base_uncertainty_ms: Option<f64>,
    pub intrinsics_available: bool,
    pub max_fps: u32,
    pub min_exposure_seconds: f64,
}

impl CapabilityProbeResult {
    /// Regula przyznawania poziomu.
    ///
    /// Odrzucamy tylko to, czego nie da sie uzyc do niczego: brak jakiejkolwiek
    /// blokady ekspozycji albo zbyt malo klatek. Reszta braków obniza poziom
    /// do `Limited`, bo nawet ograniczone nagranie ma wartosc — P1 potrzebuje
    /// roznorodnosci sensorow, zeby detektor nie przeuczyl sie na jeden aparat.
    pub fn evaluate(&self) -> CapabilityReport {
        let tier = if !self.exposure_lock || self.max_fps < MIN_USABLE_FPS {
            DeviceTier::Rejected
        } else if !self.manual_sensor
            || self.timestamp_source == TimestampSource::Unknown
            || self.min_exposure_seconds > MAX_USABLE_EXPOSURE_SECONDS
        {
            DeviceTier::Limited
        } else {
            DeviceTier::Full
        };

        CapabilityReport {
            tier,
            manual_sensor: self.manual_sensor,
            timestamp_source: self.timestamp_source,
            timestamp_base_offset_ms: None,
            timestamp_base_uncertainty_ms: None,
            intrinsics_available: self.intrinsics_available,
            max_fps: self.max_fps,
            min_exposure_seconds: self.min_exposure_seconds,
        }
    }
}

/// Estymuje przesuniecie baz zegarow z par (znacznik klatki, odczyt zegara
/// w chwili dostarczenia).
///
/// Ta sama sztuczka co przy synchronizacji: bierzemy **minimum** roznicy.
/// Najszybciej dostarczona klatka przeszla najkrotsza droge, wiec jej
/// roznica najlepiej przybliza przesuniecie baz. Rozrzut pozostalych
/// roznic sluzy za miare niepewnosci.
///
/// Zwraca `(przesuniecie_ms, niepewnosc_ms)`.
pub fn estimate_timestamp_base_offset(pairs: &[(f64, f64)]) -> Option<(f64, f64)> {
    if pairs.len() < MIN_TIMESTAMP_PAIRS {
        return None;
    }

    let deltas: Vec<f64> = pairs
        .iter()
        .map(|(frame, delivered)| delivered - frame)
        .collect();
    let minimum = deltas.iter().cloned().fold(f64::INFINITY, f64::min);

    let n = deltas.len() as f64;
    let mean = deltas.iter().sum::<f64>() / n;
    let variance = deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n;

    Some((minimum * 1000.0, variance.sqrt() * 1000.0))
}
