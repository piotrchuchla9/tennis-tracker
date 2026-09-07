/// Pojedyncza wymiana ping-pong w stylu NTP.
///
/// Wszystkie czasy sa w sekundach, w monotonicznej domenie zegara
/// urzadzenia, ktore je zmierzylo: `t1` i `t4` na zegarze lokalnym,
/// `t2` i `t3` na zegarze zdalnym.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct ClockSample {
    /// Wyslanie pingu, zegar lokalny.
    pub t1: f64,
    /// Odbior pingu, zegar zdalny.
    pub t2: f64,
    /// Wyslanie ponga, zegar zdalny.
    pub t3: f64,
    /// Odbior ponga, zegar lokalny.
    pub t4: f64,
}

impl ClockSample {
    pub fn new(t1: f64, t2: f64, t3: f64, t4: f64) -> Self {
        Self { t1, t2, t3, t4 }
    }

    /// Przesuniecie zegara zdalnego wzgledem lokalnego: `zdalny = lokalny + offset`.
    pub fn offset(&self) -> f64 {
        ((self.t2 - self.t1) + (self.t3 - self.t4)) / 2.0
    }

    /// Czas obiegu pomniejszony o czas przetwarzania po stronie zdalnej.
    /// Nizsza wartosc oznacza mniej zaburzona probke.
    pub fn delay(&self) -> f64 {
        (self.t4 - self.t1) - (self.t3 - self.t2)
    }

    /// Moment na zegarze lokalnym, do ktorego odnosi sie ta probka.
    pub fn local_time(&self) -> f64 {
        (self.t1 + self.t4) / 2.0
    }
}
