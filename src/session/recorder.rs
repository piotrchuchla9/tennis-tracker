use std::sync::Arc;

use crate::clock::SyncSnapshot;
use crate::device::CapabilityReport;
use crate::session::{
    AudioInfo, CameraInfo, CaptureControlling, CaptureProfile, DeviceInfo, LockedCameraSettings,
    ManifestError, MatchSetup, Platform, SegmentInfo, SessionEvent, SessionManifest, SessionRole,
    StorageProbing, SyncModel, SyncSampleRecord, ThermalLevel, ThermalProbing, TimingInfo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StopReason {
    UserRequested,
    StorageExhausted,
    ThermalCritical,
    PeerSilence,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::UserRequested => "user-requested",
            StopReason::StorageExhausted => "storage-exhausted",
            StopReason::ThermalCritical => "thermal-critical",
            StopReason::PeerSilence => "peer-silence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RecorderState {
    Idle,
    Armed,
    Recording,
    Finalizing,
    Stopped { reason: StopReason },
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct RecorderConfig {
    pub segment_seconds: f64,
    /// Ponizej tej rezerwy konczymy sesje w kontrolowany sposob.
    pub minimum_free_bytes: i64,
    /// Cisza peera dluzsza niz to konczy sesje po stronie slave'a.
    pub peer_silence_timeout: f64,
    pub target_bitrate_mbps: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            segment_seconds: 300.0,
            minimum_free_bytes: 500_000_000,
            peer_silence_timeout: 120.0,
            target_bitrate_mbps: 40,
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum RecorderError {
    #[error("wrong state")]
    WrongState,
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("session not started")]
    NotStarted,
    #[error("camera error: {reason}")]
    Capture { reason: String },
    #[error("manifest error: {reason}")]
    Manifest { reason: String },
}

/// Prowadzi sesje nagraniowa i buduje manifest.
///
/// Nie odczytuje zegara ani nie uruchamia timerow — czas przychodzi
/// z zewnatrz przez `tick`. Cala obsluga awarii ze specyfikacji sprowadza
/// sie dzieki temu do testow jednostkowych.
///
/// Zasada nadrzedna: **utrata lacznosci nigdy nie zatrzymuje nagrywania.**
/// Zdarzenie `link-lost` trafia do manifestu, ale nie zmienia stanu.
pub struct SessionRecorder {
    role: SessionRole,
    platform: Platform,
    device: DeviceInfo,
    peer_device_model: Option<String>,
    match_setup: MatchSetup,
    capabilities: CapabilityReport,
    transport: String,
    capture: Arc<dyn CaptureControlling>,
    storage: Arc<dyn StorageProbing>,
    thermal: Arc<dyn ThermalProbing>,
    config: RecorderConfig,

    state: RecorderState,
    session_id: Option<String>,
    profile: Option<CaptureProfile>,
    locked: Option<LockedCameraSettings>,
    segments: Vec<SegmentInfo>,
    events: Vec<SessionEvent>,
    segment_started: f64,
    last_peer_heartbeat: Option<f64>,
    last_thermal: ThermalLevel,
}

impl SessionRecorder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: SessionRole,
        platform: Platform,
        device: DeviceInfo,
        peer_device_model: Option<String>,
        match_setup: MatchSetup,
        capabilities: CapabilityReport,
        transport: String,
        capture: Arc<dyn CaptureControlling>,
        storage: Arc<dyn StorageProbing>,
        thermal: Arc<dyn ThermalProbing>,
        config: RecorderConfig,
    ) -> Self {
        Self {
            role,
            platform,
            device,
            peer_device_model,
            match_setup,
            capabilities,
            transport,
            capture,
            storage,
            thermal,
            config,
            state: RecorderState::Idle,
            session_id: None,
            profile: None,
            locked: None,
            segments: Vec::new(),
            events: Vec::new(),
            segment_started: 0.0,
            last_peer_heartbeat: None,
            last_thermal: ThermalLevel::Nominal,
        }
    }

    pub fn state(&self) -> RecorderState {
        self.state
    }

    /// Sprawdza zapas miejsca i blokuje parametry kamery.
    pub fn arm(&mut self) -> Result<(), RecorderError> {
        if self.state != RecorderState::Idle {
            return Err(RecorderError::WrongState);
        }
        if self.storage.free_bytes() < self.config.minimum_free_bytes {
            return Err(RecorderError::InsufficientStorage);
        }
        let locked = self
            .capture
            .lock_settings()
            .map_err(|error| RecorderError::Capture { reason: error.to_string() })?;
        self.locked = Some(locked);
        self.state = RecorderState::Armed;
        Ok(())
    }

    pub fn start(
        &mut self,
        session_id: &str,
        profile: CaptureProfile,
        now: f64,
    ) -> Result<(), RecorderError> {
        if self.state != RecorderState::Armed {
            return Err(RecorderError::WrongState);
        }
        self.capture
            .start_recording(session_id, profile)
            .map_err(|error| RecorderError::Capture { reason: error.to_string() })?;
        self.session_id = Some(session_id.to_string());
        self.profile = Some(profile);
        self.segment_started = now;
        self.state = RecorderState::Recording;
        Ok(())
    }

    /// Wolane co sekunde. Oddaje sterowanie czasem na zewnatrz.
    pub fn tick(&mut self, now: f64) {
        if self.state != RecorderState::Recording {
            return;
        }

        let level = self.thermal.thermal_level();
        if level != self.last_thermal {
            self.last_thermal = level;
            self.events.push(SessionEvent::thermal(now, level.as_str()));
        }
        if level == ThermalLevel::Critical {
            self.finish(now, StopReason::ThermalCritical);
            return;
        }

        if self.storage.free_bytes() < self.config.minimum_free_bytes {
            self.finish(now, StopReason::StorageExhausted);
            return;
        }

        if self.role == SessionRole::Slave {
            if let Some(last) = self.last_peer_heartbeat {
                if now - last > self.config.peer_silence_timeout {
                    self.finish(now, StopReason::PeerSilence);
                    return;
                }
            }
        }

        if now - self.segment_started >= self.config.segment_seconds {
            if let Ok(segment) = self.capture.roll_segment(now) {
                self.segments.push(segment);
                self.segment_started = now;
            }
        }
    }

    pub fn note_event(&mut self, event: SessionEvent) {
        self.events.push(event);
    }

    pub fn note_peer_heartbeat(&mut self, at: f64) {
        self.last_peer_heartbeat = Some(at);
    }

    /// Zatrzymanie na zadanie uzytkownika. Jesli sesja zostala juz
    /// zakonczona automatycznie, zwraca manifest bez ponownego dotykania kamery.
    pub fn stop(
        &mut self,
        now: f64,
        snapshot: &SyncSnapshot,
    ) -> Result<SessionManifest, RecorderError> {
        match self.state {
            RecorderState::Recording => self.finish(now, StopReason::UserRequested),
            RecorderState::Stopped { .. } => {}
            _ => return Err(RecorderError::WrongState),
        }
        let manifest = self.build_manifest(now, snapshot)?;

        // Walidacja tylko tutaj: `build_manifest` bywa wolane w trakcie
        // nagrywania, gdy lista segmentow jest jeszcze pusta. Manifest,
        // ktory sam siebie nie sprawdza przy zamknieciu sesji, jest gorszy
        // niz jego brak — w P1 wyszloby to dopiero po meczu.
        manifest
            .validate()
            .map_err(|error: ManifestError| RecorderError::Manifest { reason: error.to_string() })?;

        Ok(manifest)
    }

    fn finish(&mut self, now: f64, reason: StopReason) {
        self.state = RecorderState::Finalizing;
        if let Ok(segment) = self.capture.stop_recording(now) {
            self.segments.push(segment);
        }
        self.events.push(SessionEvent::session_stopped(now, reason.as_str()));
        self.state = RecorderState::Stopped { reason };
    }

    pub fn build_manifest(
        &self,
        now: f64,
        snapshot: &SyncSnapshot,
    ) -> Result<SessionManifest, RecorderError> {
        let session_id = self.session_id.clone().ok_or(RecorderError::NotStarted)?;
        let profile = self.profile.ok_or(RecorderError::NotStarted)?;
        let locked = self.locked.ok_or(RecorderError::NotStarted)?;

        let first_frame = self
            .capture
            .first_frame_host_time()
            .or_else(|| self.segments.first().map(|s| s.start_host_time))
            .unwrap_or(now);

        let manifest = SessionManifest {
            schema_version: 2,
            session_id,
            role: self.role,
            platform: self.platform,
            peer_device_model: self.peer_device_model.clone(),
            match_setup: self.match_setup.clone(),
            device: self.device.clone(),
            capabilities: self.capabilities.clone(),
            camera: CameraInfo::new(
                self.capture.lens_name(),
                profile,
                "hevc".into(),
                self.config.target_bitrate_mbps,
                AudioInfo { sample_rate_hz: 48000, channels: 1, codec: "pcm".into() },
                locked,
                self.capture.intrinsic_matrix(),
            ),
            timing: TimingInfo {
                clock_domain: "mach_absolute_time".into(),
                first_frame_host_time: first_frame,
                transport: self.transport.clone(),
                sync_model: SyncModel::from_fit(&snapshot.model),
                sync_samples: snapshot.samples.iter().map(SyncSampleRecord::from_sample).collect(),
                sync_gaps: snapshot.gaps.clone(),
            },
            segments: self.segments.clone(),
            events: self.events.clone(),
            motion_log: "motion.jsonl".into(),
        };

        Ok(manifest)
    }
}
