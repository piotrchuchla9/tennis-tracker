use super::{CaptureProfile, LockedCameraSettings, SegmentInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, uniffi::Enum)]
pub enum ThermalLevel {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            ThermalLevel::Nominal => "nominal",
            ThermalLevel::Fair => "fair",
            ThermalLevel::Serious => "serious",
            ThermalLevel::Critical => "critical",
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CaptureError {
    #[error("camera unavailable")]
    NoCamera,
    #[error("profile unavailable: {profile}")]
    ProfileUnavailable { profile: String },
    #[error("not recording")]
    NotRecording,
    #[error("camera error: {reason}")]
    Other { reason: String },
}

/// Sterowanie kamera. Implementacje produkcyjne opakowuja AVFoundation
/// albo Camera2.
///
/// Ksztalt tego traitu jest podyktowany **ograniczeniami Camera2**, nie
/// mozliwosciami AVFoundation: `intrinsic_matrix` jest opcjonalne, bo
/// Android zwykle go nie udostepnia, a `lock_settings` moze zwrocic
/// wartosci pochodzace z samej blokady AE, gdy manualna kontrola sensora
/// jest niedostepna.
pub trait CaptureControlling: Send + Sync {
    fn lock_settings(&self) -> Result<LockedCameraSettings, CaptureError>;
    fn start_recording(&self, session_id: &str, profile: CaptureProfile) -> Result<(), CaptureError>;
    /// Zamyka biezacy segment i otwiera nastepny. Zwraca zamkniety segment.
    fn roll_segment(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    /// Zamyka biezacy segment i konczy nagrywanie.
    fn stop_recording(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    fn first_frame_host_time(&self) -> Option<f64>;
    fn intrinsic_matrix(&self) -> Option<Vec<Vec<f64>>>;
    fn lens_name(&self) -> String;
}

pub trait StorageProbing: Send + Sync {
    fn free_bytes(&self) -> i64;
}

pub trait ThermalProbing: Send + Sync {
    fn thermal_level(&self) -> ThermalLevel;
}
