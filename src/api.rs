//! Fasada FFI.
//!
//! `SyncSession`, `PeerLink` i `SessionRecorder` maja mutowalny stan
//! i wymagaja `&mut self`, czego UniFFI nie eksportuje. Ta warstwa owija
//! je w `Mutex` i wystawia metody na `&self`.
//!
//! Caly brud FFI mieszka tutaj — logika pozostaje czysta i testowalna
//! bez zadnej wiedzy o bindingach.

use std::sync::{Arc, Mutex};

use crate::clock::{SyncSession, SyncSnapshot};
use crate::device::{
    estimate_timestamp_base_offset, CapabilityProbeResult, CapabilityReport,
};
use crate::link::{LinkMessage, PeerLink, PeerTransport};
use crate::session::{MatchSetup, ManifestError, SessionManifest};

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BaseOffsetEstimate {
    pub offset_ms: f64,
    pub uncertainty_ms: f64,
}

/// Silnik synchronizacji widziany ze Swifta i Kotlina.
#[derive(uniffi::Object)]
pub struct SyncEngine {
    session: Mutex<SyncSession>,
    link: Arc<PeerLink>,
}

#[uniffi::export]
impl SyncEngine {
    #[uniffi::constructor]
    pub fn new(transport: Arc<dyn PeerTransport>) -> Arc<Self> {
        let link = Arc::new(PeerLink::new(transport));
        Arc::new(Self {
            session: Mutex::new(SyncSession::new(link.clone())),
            link,
        })
    }

    pub fn start(&self, now: f64) {
        self.session.lock().unwrap().start(now);
    }

    pub fn tick(&self, now: f64) {
        self.session.lock().unwrap().tick(now);
    }

    /// Wolane, gdy z sieci przyszedl pakiet. Zwraca `true`, jesli pakiet
    /// byl poprawny i nie byl duplikatem.
    pub fn receive(&self, data: Vec<u8>, received_at: f64) -> bool {
        match self.link.receive(&data) {
            Some(message) => {
                self.session.lock().unwrap().handle(&message, received_at);
                true
            }
            None => false,
        }
    }

    /// Sciezka dla testow i dla transportow, ktore dostarczaja juz
    /// zdekodowana odpowiedz.
    pub fn handle_pong(&self, id: u64, t1: f64, t2: f64, t3: f64, received_at: f64) {
        self.session
            .lock()
            .unwrap()
            .handle(&LinkMessage::Pong { id, t1, t2, t3 }, received_at);
    }

    pub fn link_did_change(&self, connected: bool, at: f64) {
        self.link.set_connected(connected);
        self.session.lock().unwrap().link_did_change(connected, at);
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        self.session.lock().unwrap().snapshot()
    }

    /// Jakosc synchronizacji w milisekundach. `None`, dopoki model nie powstal.
    pub fn residual_ms(&self) -> Option<f64> {
        self.session.lock().unwrap().current_fit().map(|fit| fit.residual_std_ms)
    }
}

#[uniffi::export]
pub fn capability_report(probe: CapabilityProbeResult) -> CapabilityReport {
    probe.evaluate()
}

#[uniffi::export]
pub fn estimate_base_offset(
    frame_times: Vec<f64>,
    delivery_times: Vec<f64>,
) -> Option<BaseOffsetEstimate> {
    if frame_times.len() != delivery_times.len() {
        return None;
    }
    let pairs: Vec<(f64, f64)> = frame_times.into_iter().zip(delivery_times).collect();
    estimate_timestamp_base_offset(&pairs).map(|(offset_ms, uncertainty_ms)| BaseOffsetEstimate {
        offset_ms,
        uncertainty_ms,
    })
}

#[uniffi::export]
pub fn default_match_setup() -> MatchSetup {
    MatchSetup::default_singles()
}

#[uniffi::export]
pub fn decode_manifest(json: String) -> Result<SessionManifest, ManifestError> {
    SessionManifest::decode(&json)
}
