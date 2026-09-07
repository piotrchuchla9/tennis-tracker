#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum SessionRole {
    Master,
    Slave,
}

impl SessionRole {
    pub fn opposite(self) -> Self {
        match self {
            SessionRole::Master => SessionRole::Slave,
            SessionRole::Slave => SessionRole::Master,
        }
    }
}

/// Profil nagrywania. Wybor "rozdzielczosc kontra liczba klatek" jest
/// rozstrzygany empirycznie w P1, dlatego profil jest parametrem sesji
/// i trafia do manifestu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CaptureProfile {
    P1080p120,
    P4k60,
    P1080p60,
    P1080p30,
}

impl CaptureProfile {
    pub fn width(self) -> u32 {
        match self {
            CaptureProfile::P4k60 => 3840,
            _ => 1920,
        }
    }

    pub fn height(self) -> u32 {
        match self {
            CaptureProfile::P4k60 => 2160,
            _ => 1080,
        }
    }

    pub fn fps(self) -> u32 {
        match self {
            CaptureProfile::P1080p120 => 120,
            CaptureProfile::P4k60 | CaptureProfile::P1080p60 => 60,
            CaptureProfile::P1080p30 => 30,
        }
    }

    /// Reprezentacja tekstowa uzywana w manifescie.
    pub fn as_str(self) -> &'static str {
        match self {
            CaptureProfile::P1080p120 => "1080p120",
            CaptureProfile::P4k60 => "4K60",
            CaptureProfile::P1080p60 => "1080p60",
            CaptureProfile::P1080p30 => "1080p30",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "1080p120" => Some(CaptureProfile::P1080p120),
            "4K60" => Some(CaptureProfile::P4k60),
            "1080p60" => Some(CaptureProfile::P1080p60),
            "1080p30" => Some(CaptureProfile::P1080p30),
            _ => None,
        }
    }
}

// Serde uzywa reprezentacji tekstowej, zeby JSON manifestu i protokolu
// mowily tym samym slownikiem co specyfikacja.
impl serde::Serialize for CaptureProfile {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for CaptureProfile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        CaptureProfile::from_str(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("nieznany profil: {raw}")))
    }
}
