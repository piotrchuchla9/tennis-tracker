use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use super::{decode, encode, LinkEnvelope, LinkMessage};

/// Ile ostatnich numerow porzadkowych pamietamy do wykrywania duplikatow.
const SEQUENCE_MEMORY: usize = 1024;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TransportError {
    #[error("send failed: {reason}")]
    SendFailed { reason: String },
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LinkError {
    #[error("not connected")]
    NotConnected,
    #[error("transport error: {reason}")]
    Transport { reason: String },
    #[error("codec error: {reason}")]
    Codec { reason: String },
}

/// Tryb dostarczenia pakietu.
///
/// Rozroznienie jest wymogiem, nie optymalizacja. Pomiar synchronizacji
/// zaklada, ze opoznienie pakietu odzwierciedla rzeczywista droge. TCP
/// z retransmisjami, blokowaniem czola kolejki i algorytmem Nagle'a
/// **klamie o opoznieniu** — jeden zgubiony pakiet i probka pokazuje
/// 200 ms zamiast 5. Filtr best-delay to odrzuci, ale kosztem polowy probek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Delivery {
    /// Komendy: musza dojsc, opoznienie nieistotne. TCP albo MPC `.reliable`.
    Reliable,
    /// Pingi synchronizacyjne: opoznienie musi byc prawdziwe, zgubione
    /// pakiety sa bez znaczenia. UDP albo MPC `.unreliable`.
    Unreliable,
}

/// Surowy transport pakietow. Implementacje produkcyjne opieraja sie na
/// MultipeerConnectivity albo BLE; testy podstawiaja atrape w pamieci.
pub trait PeerTransport: Send + Sync {
    fn send(&self, data: Vec<u8>, delivery: Delivery) -> Result<(), TransportError>;
    fn is_connected(&self) -> bool;
}

struct LinkState {
    next_sequence: u64,
    seen: HashSet<u64>,
    seen_order: VecDeque<u64>,
    malformed: u64,
    connected: bool,
}

/// Warstwa nad surowym transportem: numeruje wysylane koperty,
/// odrzuca duplikaty i izoluje reszte systemu od uszkodzonych pakietow.
///
/// Kolejnosc dostarczenia nie jest wymuszana. Kazda wiadomosc niesie
/// wlasne znaczniki czasu, wiec przetasowanie niczego nie psuje, a
/// wymuszanie kolejnosci kosztowaloby bufor i opoznienie.
pub struct PeerLink {
    transport: Arc<dyn PeerTransport>,
    state: Mutex<LinkState>,
}

impl PeerLink {
    pub fn new(transport: Arc<dyn PeerTransport>) -> Self {
        Self {
            transport,
            state: Mutex::new(LinkState {
                next_sequence: 0,
                seen: HashSet::new(),
                seen_order: VecDeque::new(),
                malformed: 0,
                connected: true,
            }),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state.lock().unwrap().connected
    }

    pub fn set_connected(&self, connected: bool) {
        self.state.lock().unwrap().connected = connected;
    }

    pub fn malformed_packet_count(&self) -> u64 {
        self.state.lock().unwrap().malformed
    }

    pub fn remembered_sequence_count(&self) -> usize {
        self.state.lock().unwrap().seen.len()
    }

    /// Pingi i pongi ida trybem `Unreliable`, reszta `Reliable`.
    /// Wybor nalezy do warstwy laczа, nie do wolajacego — inaczej ktos
    /// predzej czy pozniej wysle ping po TCP i zepsuje synchronizacje.
    fn delivery_for(message: &LinkMessage) -> Delivery {
        match message {
            LinkMessage::Ping { .. } | LinkMessage::Pong { .. } => Delivery::Unreliable,
            _ => Delivery::Reliable,
        }
    }

    pub fn send(&self, message: LinkMessage) -> Result<(), LinkError> {
        let delivery = Self::delivery_for(&message);
        let sequence = {
            let mut state = self.state.lock().unwrap();
            if !state.connected {
                return Err(LinkError::NotConnected);
            }
            let sequence = state.next_sequence;
            state.next_sequence += 1;
            sequence
        };

        let bytes = encode(&LinkEnvelope { sequence, message })
            .map_err(|error| LinkError::Codec { reason: error.to_string() })?;

        self.transport
            .send(bytes, delivery)
            .map_err(|error| LinkError::Transport { reason: error.to_string() })
    }

    /// Zwraca wiadomosc, jesli pakiet jest poprawny i nie jest duplikatem.
    pub fn receive(&self, data: &[u8]) -> Option<LinkMessage> {
        let envelope = match decode(data) {
            Ok(envelope) => envelope,
            Err(_) => {
                self.state.lock().unwrap().malformed += 1;
                return None;
            }
        };

        let mut state = self.state.lock().unwrap();
        if state.seen.contains(&envelope.sequence) {
            return None;
        }
        state.seen.insert(envelope.sequence);
        state.seen_order.push_back(envelope.sequence);
        if state.seen_order.len() > SEQUENCE_MEMORY {
            if let Some(evicted) = state.seen_order.pop_front() {
                state.seen.remove(&evicted);
            }
        }
        Some(envelope.message)
    }
}
