use super::ClockFit;

/// Przelicza czas miedzy zegarem lokalnym a zegarem peera.
///
/// `peer = local + offset + skew * (local - reference)`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockModel {
    fit: ClockFit,
}

impl ClockModel {
    pub fn new(fit: ClockFit) -> Self {
        Self { fit }
    }

    pub fn fit(&self) -> ClockFit {
        self.fit
    }

    fn skew(&self) -> f64 {
        self.fit.skew_ppm * 1e-6
    }

    pub fn peer_time(&self, local_time: f64) -> f64 {
        local_time + self.fit.offset_seconds + self.skew() * (local_time - self.fit.reference_time)
    }

    /// Odwrocenie modelu:
    /// `peer = local * (1 + skew) + offset - skew * reference`
    pub fn local_time(&self, peer_time: f64) -> f64 {
        let skew = self.skew();
        (peer_time - self.fit.offset_seconds + skew * self.fit.reference_time) / (1.0 + skew)
    }
}
