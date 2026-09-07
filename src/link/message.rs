use crate::clock::ClockSample;
use crate::session::{CaptureProfile, SessionRole};

/// Wiadomosci wymieniane miedzy urzadzeniami.
///
/// Kazda wiadomosc niesie wlasne znaczniki czasu i jest samoopisujaca,
/// dzieki czemu odbiorca nie zaklada nic o kolejnosci dostarczenia.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
pub enum LinkMessage {
    Hello {
        device_model: String,
        app_version: String,
        preferred_role: SessionRole,
    },
    RoleAssigned {
        role: SessionRole,
    },
    Ping {
        id: u64,
        t1: f64,
    },
    Pong {
        id: u64,
        t1: f64,
        t2: f64,
        t3: f64,
    },
    StartRecording {
        session_id: String,
        profile: CaptureProfile,
        host_time: f64,
    },
    StopRecording {
        host_time: f64,
    },
    Heartbeat {
        host_time: f64,
    },
    SyncLog {
        samples: Vec<ClockSample>,
    },
}

/// Koperta z numerem porzadkowym. Numer sluzy wylacznie do wykrywania
/// duplikatow — kolejnosc dostarczenia nie ma znaczenia dla semantyki.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct LinkEnvelope {
    pub sequence: u64,
    pub message: LinkMessage,
}
