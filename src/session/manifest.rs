use crate::clock::{ClockFit, ClockSample, SyncGap};
use crate::device::CapabilityReport;
use crate::session::{CaptureProfile, MatchSetup, SessionRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ios,
    Android,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub model: String,
    pub os_version: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct WhiteBalanceGains {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct LockedCameraSettings {
    pub exposure_duration_seconds: f64,
    pub iso: f64,
    pub focus_lens_position: f64,
    pub white_balance_gains: WhiteBalanceGains,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub sample_rate_hz: u32,
    pub channels: u32,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfo {
    pub lens: String,
    pub profile: CaptureProfile,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: String,
    pub target_bitrate_mbps: u32,
    pub audio: AudioInfo,
    pub locked: LockedCameraSettings,
    /// Android zwykle nie udostepnia intrinsics; wtedy `None`, a P1
    /// odzyskuje ogniskowa z geometrii kortu.
    pub intrinsic_matrix: Option<Vec<Vec<f64>>>,
}

impl CameraInfo {
    /// Szerokosc, wysokosc i fps sa wyprowadzane z profilu, zeby manifest
    /// nie mogl sam sobie zaprzeczyc.
    pub fn new(
        lens: String,
        profile: CaptureProfile,
        codec: String,
        target_bitrate_mbps: u32,
        audio: AudioInfo,
        locked: LockedCameraSettings,
        intrinsic_matrix: Option<Vec<Vec<f64>>>,
    ) -> Self {
        Self {
            lens,
            profile,
            width: profile.width(),
            height: profile.height(),
            fps: profile.fps(),
            codec,
            target_bitrate_mbps,
            audio,
            locked,
            intrinsic_matrix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncModel {
    pub reference_host_time: f64,
    pub offset_seconds: f64,
    pub skew_ppm: f64,
    pub residual_std_ms: f64,
}

impl SyncModel {
    pub fn from_fit(fit: &ClockFit) -> Self {
        Self {
            reference_host_time: fit.reference_time,
            offset_seconds: fit.offset_seconds,
            skew_ppm: fit.skew_ppm,
            residual_std_ms: fit.residual_std_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncSampleRecord {
    pub host_time: f64,
    pub offset_seconds: f64,
    pub delay_seconds: f64,
}

impl SyncSampleRecord {
    pub fn from_sample(sample: &ClockSample) -> Self {
        Self {
            host_time: sample.local_time(),
            offset_seconds: sample.offset(),
            delay_seconds: sample.delay(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct TimingInfo {
    pub clock_domain: String,
    pub first_frame_host_time: f64,
    /// Ktora sciezka transportu byla uzyta: "multipeer", "sockets", "ble".
    pub transport: String,
    pub sync_model: SyncModel,
    pub sync_samples: Vec<SyncSampleRecord>,
    pub sync_gaps: Vec<SyncGap>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInfo {
    pub file: String,
    pub start_host_time: f64,
    pub end_host_time: f64,
    pub frame_count: u64,
}

/// Zdarzenie sesji. Pola opcjonalne odpowiadaja roznym typom zdarzen:
/// `magnitude` dla `motion-spike`, `reason` dla `capture-interrupted`,
/// `state` dla `thermal`, `tag` dla `mark`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub host_time: f64,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl SessionEvent {
    fn bare(host_time: f64, event_type: &str) -> Self {
        Self {
            host_time,
            event_type: event_type.into(),
            magnitude: None,
            reason: None,
            state: None,
            tag: None,
        }
    }

    pub fn motion_spike(host_time: f64, magnitude: f64) -> Self {
        Self {
            magnitude: Some(magnitude),
            ..Self::bare(host_time, "motion-spike")
        }
    }

    pub fn thermal(host_time: f64, state: &str) -> Self {
        Self {
            state: Some(state.into()),
            ..Self::bare(host_time, "thermal")
        }
    }

    pub fn capture_interrupted(host_time: f64, reason: &str) -> Self {
        Self {
            reason: Some(reason.into()),
            ..Self::bare(host_time, "capture-interrupted")
        }
    }

    pub fn capture_resumed(host_time: f64) -> Self {
        Self::bare(host_time, "capture-resumed")
    }

    pub fn mark(host_time: f64, tag: &str) -> Self {
        Self {
            tag: Some(tag.into()),
            ..Self::bare(host_time, "mark")
        }
    }

    pub fn session_stopped(host_time: f64, reason: &str) -> Self {
        Self {
            reason: Some(reason.into()),
            ..Self::bare(host_time, "session-stopped")
        }
    }

    pub fn link_lost(host_time: f64) -> Self {
        Self::bare(host_time, "link-lost")
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ManifestError {
    #[error("unsupported schema version: {version}")]
    UnsupportedSchemaVersion { version: u32 },
    #[error("no segments")]
    NoSegments,
    #[error("segments not contiguous before index {index}")]
    SegmentsNotContiguous { index: u32 },
    #[error("first frame outside first segment")]
    FirstFrameOutsideFirstSegment,
    #[error("intrinsic matrix is not 3x3")]
    MalformedIntrinsicMatrix,
    #[error("sync model contains a non-finite value")]
    NonFiniteSyncModel,
    #[error("invalid match setup: {reason}")]
    InvalidMatchSetup { reason: String },
    #[error("serialization error: {reason}")]
    Serialization { reason: String },
}

/// Sidecar opisujacy sesje nagraniowa. To wlasciwy produkt P0a —
/// wideo bez tego pliku jest w P1 bezuzyteczne.
///
/// Wszystkie znaczniki czasu sa w sekundach, w monotonicznej domenie
/// zegara lokalnego urzadzenia (`timing.clock_domain`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SessionManifest {
    pub schema_version: u32,
    pub session_id: String,
    pub role: SessionRole,
    pub platform: Platform,
    pub peer_device_model: Option<String>,
    #[serde(rename = "match")]
    pub match_setup: MatchSetup,
    pub device: DeviceInfo,
    pub capabilities: CapabilityReport,
    pub camera: CameraInfo,
    pub timing: TimingInfo,
    pub segments: Vec<SegmentInfo>,
    pub events: Vec<SessionEvent>,
    pub motion_log: String,
}

impl SessionManifest {
    pub fn encode(&self) -> Result<String, ManifestError> {
        serde_json::to_string_pretty(self).map_err(|error| ManifestError::Serialization {
            reason: error.to_string(),
        })
    }

    pub fn decode(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|error| ManifestError::Serialization {
            reason: error.to_string(),
        })
    }

    /// Dopuszczalna przerwa miedzy segmentami: jedna klatka.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != 2 {
            return Err(ManifestError::UnsupportedSchemaVersion {
                version: self.schema_version,
            });
        }
        let first = self.segments.first().ok_or(ManifestError::NoSegments)?;

        let tolerance = 1.0 / self.camera.fps as f64;
        for index in 1..self.segments.len() {
            let previous = &self.segments[index - 1];
            let current = &self.segments[index];
            if current.start_host_time - previous.end_host_time > tolerance {
                return Err(ManifestError::SegmentsNotContiguous {
                    index: index as u32,
                });
            }
        }

        let first_frame = self.timing.first_frame_host_time;
        if first_frame < first.start_host_time - tolerance || first_frame > first.end_host_time {
            return Err(ManifestError::FirstFrameOutsideFirstSegment);
        }

        if let Some(matrix) = &self.camera.intrinsic_matrix {
            if matrix.len() != 3 || matrix.iter().any(|row| row.len() != 3) {
                return Err(ManifestError::MalformedIntrinsicMatrix);
            }
        }

        let model = &self.timing.sync_model;
        if !model.offset_seconds.is_finite()
            || !model.skew_ppm.is_finite()
            || !model.residual_std_ms.is_finite()
        {
            return Err(ManifestError::NonFiniteSyncModel);
        }

        self.match_setup
            .validate()
            .map_err(|error| ManifestError::InvalidMatchSetup {
                reason: error.to_string(),
            })?;

        Ok(())
    }
}
