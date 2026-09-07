use std::collections::HashMap;
use std::sync::Arc;

use super::{ClockFit, ClockSample, ClockSyncEstimator};
use crate::link::{LinkMessage, PeerLink};

/// Luka w ciaglosci synchronizacji. `end_host_time` jest `None`,
/// dopoki luka trwa.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncGap {
    pub start_host_time: f64,
    pub end_host_time: Option<f64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SyncSnapshot {
    pub model: ClockFit,
    pub samples: Vec<ClockSample>,
    pub gaps: Vec<SyncGap>,
}

/// Prowadzi synchronizacje zegarow: wysyla pingi, odpowiada na cudze,
/// laczy odpowiedzi w probki i utrzymuje aktualny model.
///
/// Klasa nie odczytuje zegara samodzielnie — czas jest podawany z zewnatrz.
/// Dzieki temu testy sterują uplywem czasu bez czekania, a produkcja
/// wstrzykuje zegar monotoniczny.
pub struct SyncSession {
    /// Liczba pingow w serii startowej.
    pub burst_count: usize,
    /// Odstep miedzy pingami w stanie ustalonym.
    pub steady_period: f64,

    link: Arc<PeerLink>,
    estimator: ClockSyncEstimator,
    samples: Vec<ClockSample>,
    gaps: Vec<SyncGap>,
    current_fit: Option<ClockFit>,
    pending: HashMap<u64, f64>,
    next_ping_id: u64,
    last_ping_time: f64,
    connected: bool,
}

impl SyncSession {
    pub fn new(link: Arc<PeerLink>) -> Self {
        Self {
            burst_count: 50,
            steady_period: 1.0,
            link,
            estimator: ClockSyncEstimator::new(),
            samples: Vec::new(),
            gaps: Vec::new(),
            current_fit: None,
            pending: HashMap::new(),
            next_ping_id: 0,
            last_ping_time: f64::NEG_INFINITY,
            connected: true,
        }
    }

    /// Seria startowa. Wysylana jednym ciagiem — na lokalnej sieci
    /// zajmuje ulamek sekundy, a od razu daje uzyteczny model.
    pub fn start(&mut self, now: f64) {
        for _ in 0..self.burst_count {
            self.send_ping(now);
        }
        self.last_ping_time = now;
    }

    pub fn tick(&mut self, now: f64) {
        if !self.connected {
            return;
        }
        if now - self.last_ping_time < self.steady_period {
            return;
        }
        self.send_ping(now);
        self.last_ping_time = now;
    }

    pub fn link_did_change(&mut self, connected: bool, at: f64) {
        if connected == self.connected {
            return;
        }
        self.connected = connected;
        if connected {
            if let Some(gap) = self.gaps.last_mut() {
                if gap.end_host_time.is_none() {
                    gap.end_host_time = Some(at);
                }
            }
            self.last_ping_time = f64::NEG_INFINITY;
        } else {
            self.gaps.push(SyncGap {
                start_host_time: at,
                end_host_time: None,
                reason: "link-lost".into(),
            });
            self.pending.clear();
        }
    }

    pub fn handle(&mut self, message: &LinkMessage, received_at: f64) {
        match message {
            LinkMessage::Ping { id, t1 } => {
                // Odpowiadamy natychmiast; t2 i t3 roznia sie o czas obslugi,
                // ktory na tej sciezce jest ponizej rozdzielczosci pomiaru.
                let _ = self.link.send(LinkMessage::Pong {
                    id: *id,
                    t1: *t1,
                    t2: received_at,
                    t3: received_at,
                });
            }
            LinkMessage::Pong { id, t1, t2, t3 } => {
                if self.pending.remove(id).is_none() {
                    return;
                }
                let sample = ClockSample::new(*t1, *t2, *t3, received_at);
                self.samples.push(sample);
                self.estimator.add(sample);
                self.current_fit = self.estimator.fit();
            }
            _ => {}
        }
    }

    pub fn current_fit(&self) -> Option<ClockFit> {
        self.current_fit
    }

    pub fn samples(&self) -> &[ClockSample] {
        &self.samples
    }

    pub fn gaps(&self) -> &[SyncGap] {
        &self.gaps
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        SyncSnapshot {
            model: self.current_fit.unwrap_or_else(ClockFit::unknown),
            samples: self.samples.clone(),
            gaps: self.gaps.clone(),
        }
    }

    fn send_ping(&mut self, now: f64) {
        let id = self.next_ping_id;
        self.next_ping_id += 1;
        self.pending.insert(id, now);
        let _ = self.link.send(LinkMessage::Ping { id, t1: now });
    }
}
