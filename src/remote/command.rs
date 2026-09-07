use crate::session::SessionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MarkTag {
    Interesting,
    Changeover,
    StrayBall,
    BadRally,
}

impl MarkTag {
    pub fn as_str(self) -> &'static str {
        match self {
            MarkTag::Interesting => "interesting",
            MarkTag::Changeover => "changeover",
            MarkTag::StrayBall => "stray-ball",
            MarkTag::BadRally => "bad-rally",
        }
    }
}

/// Komendy z urzadzenia sterujacego.
///
/// P0a nie liczy punktow, wiec zestaw jest minimalny. Komendy punktacyjne
/// dochodza w P2; enum istnieje od teraz, zeby kolejne urzadzenia sterujace
/// wpinaly sie bez ruszania logiki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RemoteCommand {
    StartRecording,
    StopRecording,
    MarkMoment { tag: MarkTag },
}

/// Informacja zwrotna na urzadzenie sterujace.
///
/// Telefony wisza 3 m nad ziemia, wiec to jedyna droga, ktora ostrzezenie
/// o koncu miejsca albo przegrzaniu dociera do gracza w trakcie meczu.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum RemoteFeedback {
    RecordingState { recording: bool, elapsed_seconds: f64 },
    SyncQuality { residual_ms: f64 },
    Storage { free_gigabytes: f64 },
    Warning { text: String },
}

/// Komendy, ktore zostawiaja slad w manifescie, zamieniaja sie w zdarzenie.
pub fn command_to_event(command: &RemoteCommand, host_time: f64) -> Option<SessionEvent> {
    match command {
        RemoteCommand::MarkMoment { tag } => Some(SessionEvent::mark(host_time, tag.as_str())),
        _ => None,
    }
}
