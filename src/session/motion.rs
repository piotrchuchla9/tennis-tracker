use super::SessionEvent;

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MotionVector {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl MotionVector {
    pub fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MotionSample {
    pub host_time: f64,
    /// Predkosc katowa w rad/s.
    pub rotation_rate: MotionVector,
}

/// Wykrywa szarpniecia telefonu zawieszonego na ogrodzeniu.
///
/// Znaczenie tych zdarzen jest w calosci po stronie P1: sa markerem
/// "tutaj homografia mogla przestac pasowac, przelicz ja od nowa".
/// Dlatego liczy sie czas i skala, a nie klasyfikacja przyczyny.
#[derive(Debug, Clone)]
pub struct MotionAnalyzer {
    /// Prog predkosci katowej w rad/s.
    pub rotation_threshold: f64,
    /// Po wykryciu skoku ignorujemy kolejne przez ten czas, zeby jedno
    /// szarpniecie nie zamienilo sie w setke zdarzen.
    pub refractory_seconds: f64,
    last_spike_time: f64,
}

impl Default for MotionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl MotionAnalyzer {
    pub fn new() -> Self {
        Self {
            rotation_threshold: 0.35,
            refractory_seconds: 1.0,
            last_spike_time: f64::NEG_INFINITY,
        }
    }

    pub fn process(&mut self, sample: &MotionSample) -> Option<SessionEvent> {
        let magnitude = sample.rotation_rate.norm();
        if magnitude < self.rotation_threshold {
            return None;
        }
        if sample.host_time - self.last_spike_time < self.refractory_seconds {
            return None;
        }
        self.last_spike_time = sample.host_time;
        Some(SessionEvent::motion_spike(sample.host_time, magnitude))
    }
}
