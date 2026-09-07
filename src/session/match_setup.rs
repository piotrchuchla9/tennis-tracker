#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum CourtEnd {
    North,
    South,
}

impl CourtEnd {
    pub fn opposite(self) -> Self {
        match self {
            CourtEnd::North => CourtEnd::South,
            CourtEnd::South => CourtEnd::North,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MatchFormat {
    SinglesAdTiebreak,
}

impl MatchFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchFormat::SinglesAdTiebreak => "singles-ad-tiebreak",
        }
    }
}

impl serde::Serialize for MatchFormat {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for MatchFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::Deserialize;
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "singles-ad-tiebreak" => Ok(MatchFormat::SinglesAdTiebreak),
            other => Err(serde::de::Error::custom(format!("nieznany format: {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: String,
    pub name: String,
    /// Dopelniacz imienia, np. "Piotra" dla "Piotr".
    ///
    /// Uzywany przez silnik oglosen w P2: "punkt dla **Piotra**". Polskiej
    /// odmiany nie da sie wyliczyc algorytmicznie w sposob pewny — "Marek"
    /// daje "Marka", z wypadnieciem e. Pole jest opcjonalne, bo oglaszanie
    /// domyslnie uzywa konstrukcji nie wymagajacych przypadkow zaleznych.
    pub genitive: Option<String>,
    pub start_end: CourtEnd,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MatchSetupError {
    #[error("expected 2 players, got {count}")]
    WrongPlayerCount { count: u32 },
    #[error("duplicate player id: {id}")]
    DuplicateId { id: String },
    #[error("first server {id} is not among the players")]
    UnknownServer { id: String },
    #[error("both players on the same court end")]
    SameEnd,
}

/// Ustawienie meczu.
///
/// Gracze sa lista, nie para pol — dzieki temu debel w P4 dokladа graczy
/// zamiast wymuszac przepisanie modelu i wszystkiego, co go uzywa.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct MatchSetup {
    pub players: Vec<Player>,
    pub first_server: String,
    pub format: MatchFormat,
}

impl MatchSetup {
    /// Anonimowe ustawienie domyslne. Imiona nie moga blokowac startu nagrania.
    pub fn default_singles() -> Self {
        Self {
            players: vec![
                Player {
                    id: "p1".into(),
                    name: "Player 1".into(),
                    genitive: None,
                    start_end: CourtEnd::North,
                },
                Player {
                    id: "p2".into(),
                    name: "Player 2".into(),
                    genitive: None,
                    start_end: CourtEnd::South,
                },
            ],
            first_server: "p1".into(),
            format: MatchFormat::SinglesAdTiebreak,
        }
    }

    pub fn player(&self, id: &str) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Koniec kortu, na ktorym stoi gracz po `completed_games` rozegranych gemach.
    ///
    /// Zmiana stron nastepuje po nieparzystych gemach: po 1., 3., 5. i tak dalej.
    /// Liczba dotychczasowych zmian to `(completed_games + 1) / 2`; parzysta
    /// liczba zmian oznacza powrot na koniec startowy.
    pub fn end_of(&self, id: &str, completed_games: u32) -> Option<CourtEnd> {
        let player = self.player(id)?;
        let swaps = (completed_games + 1) / 2;
        Some(if swaps % 2 == 0 { player.start_end } else { player.start_end.opposite() })
    }

    pub fn validate(&self) -> Result<(), MatchSetupError> {
        if self.players.len() != 2 {
            return Err(MatchSetupError::WrongPlayerCount { count: self.players.len() as u32 });
        }
        if self.players[0].id == self.players[1].id {
            return Err(MatchSetupError::DuplicateId { id: self.players[0].id.clone() });
        }
        if self.player(&self.first_server).is_none() {
            return Err(MatchSetupError::UnknownServer { id: self.first_server.clone() });
        }
        if self.players[0].start_end == self.players[1].start_end {
            return Err(MatchSetupError::SameEnd);
        }
        Ok(())
    }
}
