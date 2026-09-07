use super::{ClockFit, ClockSample};

/// Estymuje liniowy model roznicy zegarow z serii probek ping-pong.
///
/// Dwie decyzje projektowe niosa tu cala jakosc wyniku:
///
/// 1. Do dopasowania trafiaja wylacznie probki o najnizszym `delay`.
///    Pakiet, ktory przeszedl najszybciej, przeszedl najmniej zaburzony,
///    wiec jego `offset` jest najblizszy prawdy. Filtr ten zastepuje
///    odporna statystyke i sam usuwa wartosci odstajace.
/// 2. Dopasowywana jest prosta, nie stala. Kwarce dwoch urzadzen chodza
///    z rozna predkoscia i przez godzine rozjezdzaja sie o dziesiatki
///    milisekund. Bez czlonu skew model rozsypuje sie w trakcie meczu.
#[derive(Debug, Clone)]
pub struct ClockSyncEstimator {
    /// Ile ostatnich probek trzymamy.
    pub window_size: usize,
    /// Jaka czesc okna, liczac od najnizszego `delay`, trafia do dopasowania.
    pub best_fraction: f64,
    /// Ponizej tej liczby probek nie zwracamy modelu.
    pub minimum_samples: usize,
    samples: Vec<ClockSample>,
}

impl Default for ClockSyncEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockSyncEstimator {
    pub fn new() -> Self {
        Self {
            window_size: 300,
            best_fraction: 0.25,
            minimum_samples: 8,
            samples: Vec::new(),
        }
    }

    pub fn add(&mut self, sample: ClockSample) {
        self.samples.push(sample);
        if self.samples.len() > self.window_size {
            let excess = self.samples.len() - self.window_size;
            self.samples.drain(0..excess);
        }
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Probki wybrane do dopasowania, posortowane rosnaco po `local_time`.
    fn selected(&self) -> Vec<ClockSample> {
        if self.samples.len() < self.minimum_samples {
            return Vec::new();
        }
        let mut by_delay = self.samples.clone();
        by_delay.sort_by(|a, b| a.delay().partial_cmp(&b.delay()).unwrap());

        let wanted = ((by_delay.len() as f64 * self.best_fraction).ceil() as usize)
            .max(self.minimum_samples)
            .min(by_delay.len());

        let mut chosen: Vec<ClockSample> = by_delay.into_iter().take(wanted).collect();
        chosen.sort_by(|a, b| a.local_time().partial_cmp(&b.local_time()).unwrap());
        chosen
    }

    pub fn fit(&self) -> Option<ClockFit> {
        let chosen = self.selected();
        if chosen.len() < self.minimum_samples {
            return None;
        }

        let n = chosen.len() as f64;
        let times: Vec<f64> = chosen.iter().map(|s| s.local_time()).collect();
        let offsets: Vec<f64> = chosen.iter().map(|s| s.offset()).collect();

        // Punkt odniesienia w srodku ciezkosci probek. Dzieki temu suma
        // odchylek x jest zerowa, a wyraz wolny rowna sie sredniej offsetow.
        let reference = times.iter().sum::<f64>() / n;
        let mean_offset = offsets.iter().sum::<f64>() / n;

        let mut sxx = 0.0;
        let mut sxy = 0.0;
        for (time, offset) in times.iter().zip(offsets.iter()) {
            let x = time - reference;
            sxx += x * x;
            sxy += x * (offset - mean_offset);
        }
        let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };

        let mut sum_squares = 0.0;
        for (time, offset) in times.iter().zip(offsets.iter()) {
            let predicted = mean_offset + slope * (time - reference);
            let residual = offset - predicted;
            sum_squares += residual * residual;
        }
        // Dwa stopnie swobody zjada dopasowana prosta.
        let variance = if chosen.len() > 2 {
            sum_squares / (n - 2.0)
        } else {
            0.0
        };

        Some(ClockFit {
            reference_time: reference,
            offset_seconds: mean_offset,
            skew_ppm: slope * 1e6,
            residual_std_ms: variance.sqrt() * 1000.0,
            sample_count: chosen.len() as u32,
        })
    }
}
