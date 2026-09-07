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
pub fn make_sample(
    t1: f64,
    offset: f64,
    forward: f64,
    backward: f64,
    processing: f64,
) -> ClockSample {
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
use tracker_core::link::{
    decode, Delivery, LinkEnvelope, LinkMessage, PeerTransport, TransportError,
};

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
        self.sent_envelopes()
            .into_iter()
            .map(|e| e.message)
            .collect()
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
            return Err(TransportError::SendFailed {
                reason: "atrapa".into(),
            });
        }
        self.sent.lock().unwrap().push((data, delivery));
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
}

use tracker_core::session::{
    CaptureControlling, CaptureError, CaptureProfile, LockedCameraSettings, SegmentInfo,
    StorageProbing, ThermalLevel, ThermalProbing, WhiteBalanceGains,
};

pub struct FakeCapture {
    state: Mutex<FakeCaptureState>,
    pub lock_should_fail: Mutex<bool>,
}

struct FakeCaptureState {
    did_lock: bool,
    did_start: bool,
    did_stop: bool,
    roll_count: usize,
    segment_index: usize,
    segment_start: f64,
}

impl FakeCapture {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeCaptureState {
                did_lock: false,
                did_start: false,
                did_stop: false,
                roll_count: 0,
                segment_index: 0,
                segment_start: 0.0,
            }),
            lock_should_fail: Mutex::new(false),
        }
    }

    pub fn did_lock(&self) -> bool {
        self.state.lock().unwrap().did_lock
    }
    pub fn did_start(&self) -> bool {
        self.state.lock().unwrap().did_start
    }
    pub fn did_stop(&self) -> bool {
        self.state.lock().unwrap().did_stop
    }
    pub fn roll_count(&self) -> usize {
        self.state.lock().unwrap().roll_count
    }
    pub fn set_lock_should_fail(&self, fail: bool) {
        *self.lock_should_fail.lock().unwrap() = fail;
    }

    fn close_segment(&self, state: &mut FakeCaptureState, now: f64) -> SegmentInfo {
        let info = SegmentInfo {
            file: format!("video-{:03}.mov", state.segment_index),
            start_host_time: state.segment_start,
            end_host_time: now,
            frame_count: ((now - state.segment_start) * 120.0) as u64,
        };
        state.segment_index += 1;
        state.segment_start = now;
        info
    }
}

impl Default for FakeCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureControlling for FakeCapture {
    fn lock_settings(&self) -> Result<LockedCameraSettings, CaptureError> {
        if *self.lock_should_fail.lock().unwrap() {
            return Err(CaptureError::NoCamera);
        }
        self.state.lock().unwrap().did_lock = true;
        Ok(LockedCameraSettings {
            exposure_duration_seconds: 0.001,
            iso: 64.0,
            focus_lens_position: 0.82,
            white_balance_gains: WhiteBalanceGains {
                r: 1.9,
                g: 1.0,
                b: 1.6,
            },
        })
    }

    fn start_recording(
        &self,
        _session_id: &str,
        _profile: CaptureProfile,
    ) -> Result<(), CaptureError> {
        let mut state = self.state.lock().unwrap();
        state.did_start = true;
        state.segment_start = 1000.05;
        Ok(())
    }

    fn roll_segment(&self, now: f64) -> Result<SegmentInfo, CaptureError> {
        let mut state = self.state.lock().unwrap();
        state.roll_count += 1;
        Ok(self.close_segment(&mut state, now))
    }

    fn stop_recording(&self, now: f64) -> Result<SegmentInfo, CaptureError> {
        let mut state = self.state.lock().unwrap();
        state.did_stop = true;
        Ok(self.close_segment(&mut state, now))
    }

    fn first_frame_host_time(&self) -> Option<f64> {
        Some(1000.05)
    }

    fn intrinsic_matrix(&self) -> Option<Vec<Vec<f64>>> {
        Some(vec![
            vec![1580.0, 0.0, 960.0],
            vec![0.0, 1580.0, 540.0],
            vec![0.0, 0.0, 1.0],
        ])
    }

    fn lens_name(&self) -> String {
        "builtInWideAngleCamera".into()
    }
}

pub struct FakeStorage {
    free: Mutex<i64>,
}

impl FakeStorage {
    pub fn new() -> Self {
        Self {
            free: Mutex::new(64_000_000_000),
        }
    }
    pub fn set_free_bytes(&self, bytes: i64) {
        *self.free.lock().unwrap() = bytes;
    }
}

impl Default for FakeStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageProbing for FakeStorage {
    fn free_bytes(&self) -> i64 {
        *self.free.lock().unwrap()
    }
}

pub struct FakeThermal {
    level: Mutex<ThermalLevel>,
}

impl FakeThermal {
    pub fn new() -> Self {
        Self {
            level: Mutex::new(ThermalLevel::Nominal),
        }
    }
    pub fn set_level(&self, level: ThermalLevel) {
        *self.level.lock().unwrap() = level;
    }
}

impl Default for FakeThermal {
    fn default() -> Self {
        Self::new()
    }
}

impl ThermalProbing for FakeThermal {
    fn thermal_level(&self) -> ThermalLevel {
        *self.level.lock().unwrap()
    }
}
