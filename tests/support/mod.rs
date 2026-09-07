#![allow(dead_code)]

use tracker_core::clock::ClockSample;

/// Deterministyczny generator liniowy kongruentny. Testy nie moga byc losowe.
pub struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// Wartosc z przedzialu [0, upper).
    pub fn uniform(&mut self, upper: f64) -> f64 {
        (self.next_u64() % 1_000_000) as f64 / 1_000_000.0 * upper
    }
}

/// Buduje probke o zadanych, znanych parametrach sciezki.
/// Odwrotnosc arytmetyki z `ClockSample`, dzieki czemu test wie,
/// jaka wartosc estymator powinien odzyskac.
pub fn make_sample(t1: f64, offset: f64, forward: f64, backward: f64, processing: f64) -> ClockSample {
    let t2 = t1 + forward + offset;
    let t3 = t2 + processing;
    let t4 = t3 - offset + backward;
    ClockSample::new(t1, t2, t3, t4)
}

pub struct SeriesSpec {
    pub start: f64,
    pub count: usize,
    pub period: f64,
    pub offset0: f64,
    pub skew_ppm: f64,
    pub base_one_way: f64,
    pub jitter: f64,
    pub seed: u64,
    /// Co ile probek wstawic pakiet o ogromnym opoznieniu. `None` = brak.
    pub outlier_every: Option<usize>,
}

impl Default for SeriesSpec {
    fn default() -> Self {
        Self {
            start: 1000.0,
            count: 60,
            period: 1.0,
            offset0: 0.25,
            skew_ppm: 0.0,
            base_one_way: 0.005,
            jitter: 0.0004,
            seed: 1,
            outlier_every: None,
        }
    }
}

/// Seria probek z zadanym offsetem poczatkowym, dryfem i jitterem.
pub fn make_series(spec: &SeriesSpec) -> Vec<ClockSample> {
    let mut rng = SeededRandom::new(spec.seed);
    (0..spec.count)
        .map(|index| {
            let t1 = spec.start + index as f64 * spec.period;
            let true_offset = spec.offset0 + spec.skew_ppm * 1e-6 * (t1 - spec.start);

            let mut forward = spec.base_one_way + rng.uniform(spec.jitter);
            let mut backward = spec.base_one_way + rng.uniform(spec.jitter);

            if let Some(every) = spec.outlier_every {
                if every > 0 && index % every == 0 {
                    forward += 0.250;
                    backward += 0.050;
                }
            }

            make_sample(t1, true_offset, forward, backward, 0.001)
        })
        .collect()
}

use std::sync::Mutex;
use tracker_core::link::{decode, Delivery, LinkEnvelope, LinkMessage, PeerTransport, TransportError};

/// Transport w pamieci. Pozwala testom podac dowolna sekwencje pakietow,
/// wlacznie z duplikatami i kolejnoscia odwrocona.
pub struct FakeTransport {
    sent: Mutex<Vec<(Vec<u8>, Delivery)>>,
    connected: Mutex<bool>,
    fail_send: Mutex<bool>,
}

impl FakeTransport {
    pub fn new() -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            connected: Mutex::new(true),
            fail_send: Mutex::new(false),
        }
    }

    pub fn sent_envelopes(&self) -> Vec<LinkEnvelope> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .map(|(bytes, _)| decode(bytes).expect("wyslany pakiet musi byc poprawny"))
            .collect()
    }

    /// Tryby dostarczenia w kolejnosci wysylki — do sprawdzenia,
    /// ze pingi nie ida po niezawodnym kanale.
    pub fn sent_deliveries(&self) -> Vec<Delivery> {
        self.sent.lock().unwrap().iter().map(|(_, d)| *d).collect()
    }

    pub fn sent_messages(&self) -> Vec<LinkMessage> {
        self.sent_envelopes().into_iter().map(|e| e.message).collect()
    }

    pub fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }

    pub fn clear_sent(&self) {
        self.sent.lock().unwrap().clear();
    }

    pub fn set_connected(&self, connected: bool) {
        *self.connected.lock().unwrap() = connected;
    }

    pub fn set_fail_send(&self, fail: bool) {
        *self.fail_send.lock().unwrap() = fail;
    }
}

impl Default for FakeTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerTransport for FakeTransport {
    fn send(&self, data: Vec<u8>, delivery: Delivery) -> Result<(), TransportError> {
        if *self.fail_send.lock().unwrap() {
            return Err(TransportError::SendFailed { reason: "atrapa".into() });
        }
        self.sent.lock().unwrap().push((data, delivery));
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
}
