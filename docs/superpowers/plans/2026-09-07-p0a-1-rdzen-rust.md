# P0a część 1 — rdzeń Rust: plan implementacji

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Crate `tracker-core` w Ruście zawierający całą logikę bezsprzętową rigu — synchronizację zegarów, protokół łącza, manifest sesji, ustawienie meczu, maszynę stanów nagrywania i raport zdolności urządzenia — wraz z bindingami UniFFI dla Swifta i Kotlina.

**Architecture:** Wszystko w tym planie jest czystą logiką bez zależności od sprzętu. Zależności platformowe wyrażone są traitami, które w części 2 zaimplementują adaptery natywne. Cały plan weryfikuje się przez `cargo test` na macOS — bez symulatora, bez Xcode, bez telefonu, pętla TDD w sekundach.

**Tech Stack:** Rust 2021, UniFFI 0.28 (tryb proc-macro), serde + serde_json, XCTest niepotrzebny.

**Spec:** `docs/superpowers/specs/2026-09-07-tennis-tracker-p0a-rdzen-ios-design.md`

**Część 2** (aplikacje iOS i watchOS, narzędzia Python) powstanie osobno, po zazielenieniu tego planu. Kolejność jest celowa: kształt wygenerowanego API jest znany dopiero, gdy rdzeń istnieje.

## Global Constraints

- Edycja Rusta: **2021**. Nazwa crate'u: `tracker-core`, nazwa biblioteki: `tracker_core`.
- **Wyłącznie zegar monotoniczny.** Rdzeń **nigdy** nie odczytuje czasu samodzielnie — czas przychodzi z zewnątrz jako parametr. Dzięki temu testy sterują upływem czasu bez czekania, a produkcja wstrzykuje `CACurrentMediaTime()`.
- Jednostka czasu: **sekundy**, typ `f64`. Nazwa domeny w manifeście: `"mach_absolute_time"`.
- Cel dokładności synchronizacji: **poniżej 2 ms** (`residual_std_ms < 2.0`).
- Model synchronizacji jest **liniowy: offset ORAZ skew**. Sam offset jest niewystarczający.
- `schemaVersion` manifestu: **2**.
- Segmenty po **300 sekund**. Timeout ciszy peera: **120 sekund**.
- Audio: PCM, **48000 Hz, mono**.
- Profile iOS: `1080p120` (domyślny), `4K60`, `1080p60`. Profile Android: `1080p60`, `1080p30`.
- Poziomy zdolności: `full`, `limited`, `rejected`.
- **Utrata łączności nigdy nie zatrzymuje nagrywania.**
- Klucze JSON w manifeście są w `camelCase`, pola Rusta w `snake_case` — mapowanie przez `#[serde(rename_all = "camelCase")]`.
- Komentarze w kodzie: polski, bez znaków diakrytycznych (unikamy problemów z kodowaniem w narzędziach). Nazwy symboli: angielski.
- **Komunikaty `Display` typów błędów: angielski.** Przechodzą przez granicę FFI i są przeznaczone dla logów, nie dla użytkownika — interfejs mapuje wariant enuma na zlokalizowany tekst i nigdy nie wyświetla pola `reason`.
- Każdy typ danych przekraczający granicę FFI dostaje `#[derive(uniffi::Record)]` albo `#[derive(uniffi::Enum)]` już w zadaniu, które go tworzy. Typy stanowe (z mutowalnym stanem wewnętrznym) eksportowane są jako `uniffi::Object` dopiero w zadaniu 13.

---

## Struktura plików

```
Cargo.toml                          manifest crate'u
.gitignore
rust-toolchain.toml                 przypięta wersja toolchainu
src/lib.rs                          moduły + uniffi::setup_scaffolding!()
src/clock/mod.rs
src/clock/sample.rs                 ClockSample: offset, delay
src/clock/fit.rs                    ClockFit: wynik dopasowania
src/clock/estimator.rs              ClockSyncEstimator: best-delay + regresja
src/clock/model.rs                  ClockModel: konwersja czasu
src/clock/session.rs                SyncSession: harmonogram, luki
src/link/mod.rs
src/link/message.rs                 LinkMessage, LinkEnvelope
src/link/codec.rs                   serializacja
src/link/peer.rs                    PeerTransport, PeerLink
src/session/mod.rs
src/session/profile.rs              SessionRole, CaptureProfile
src/session/match_setup.rs          MatchSetup, Player, CourtEnd
src/session/manifest.rs             SessionManifest i typy zagniezdzone
src/session/motion.rs               MotionAnalyzer
src/session/ports.rs                traity: kamera, dysk, termika, sync
src/session/recorder.rs             SessionRecorder: maszyna stanow
src/device/mod.rs
src/device/capability.rs            CapabilityReport, DeviceTier
src/remote/mod.rs
src/remote/command.rs               RemoteCommand, RemoteFeedback
tests/support/mod.rs                generator deterministyczny, fabryki probek
tests/clock_sample.rs
tests/clock_estimator.rs
tests/clock_model.rs
tests/link_codec.rs
tests/peer_link.rs
tests/sync_session.rs
tests/capability.rs
tests/match_setup.rs
tests/manifest.rs
tests/motion.rs
tests/recorder.rs
tests/bindings.rs                   weryfikacja generacji bindingow
```

Testy leżą w `tests/` (testy integracyjne), nie w modułach — konsumują crate przez jego publiczne API, dokładnie tak jak zrobią to bindingi. Gdyby coś nie dało się przetestować z zewnątrz, znaczyłoby to, że nie da się tego wywołać ze Swifta.

---

### Task 1: Szkielet crate'u i model próbki zegara

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `rust-toolchain.toml`
- Create: `src/lib.rs`, `src/clock/mod.rs`, `src/clock/sample.rs`
- Test: `tests/clock_sample.rs`

**Interfaces:**
- Consumes: nic
- Produces: `ClockSample` z `ClockSample::new(t1: f64, t2: f64, t3: f64, t4: f64)` oraz metodami `offset() -> f64`, `delay() -> f64`, `local_time() -> f64`

- [ ] **Step 1: Utwórz `Cargo.toml`**

```toml
[package]
name = "tracker-core"
version = "0.1.0"
edition = "2021"

[lib]
name = "tracker_core"
crate-type = ["lib", "staticlib", "cdylib"]

[dependencies]
uniffi = { version = "0.28", features = ["cli"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
uniffi = { version = "0.28", features = ["build"] }

[[bin]]
name = "uniffi-bindgen"
path = "src/bin/uniffi-bindgen.rs"
required-features = ["cli"]
```

- [ ] **Step 2: Utwórz `rust-toolchain.toml` i `.gitignore`**

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
targets = [
  "aarch64-apple-ios",
  "aarch64-apple-ios-sim",
  "aarch64-linux-android",
]
```

`.gitignore`:

```
target/
Cargo.lock
.DS_Store
DerivedData/
xcuserdata/
*.xcuserstate
bindings/
tools/__pycache__/
tools/out/
```

`Cargo.lock` jest ignorowany, bo `tracker-core` jest biblioteką, nie binarką aplikacyjną.

- [ ] **Step 3: Utwórz `src/lib.rs` i `src/clock/mod.rs`**

`src/lib.rs`:

```rust
//! Rdzen rigu Tennis Tracker.
//!
//! Cala logika bezsprzetowa: synchronizacja zegarow, protokol lacza,
//! manifest sesji, maszyna stanow nagrywania.
//!
//! Zasada nadrzedna: ten crate NIGDY nie odczytuje zegara samodzielnie.
//! Czas przychodzi z zewnatrz jako parametr, dzieki czemu testy sterują
//! jego uplywem bez czekania.

pub mod clock;

uniffi::setup_scaffolding!();
```

`src/clock/mod.rs`:

```rust
mod sample;

pub use sample::ClockSample;
```

- [ ] **Step 4: Napisz test, który ma nie przejść**

`tests/clock_sample.rs`:

```rust
use tracker_core::clock::ClockSample;

/// Zegar zdalny wyprzedza lokalny o 0,5 s, opoznienie w obie strony po 10 ms,
/// przetwarzanie po stronie zdalnej 2 ms.
#[test]
fn offset_and_delay_for_symmetric_path() {
    let s = ClockSample::new(100.000, 100.510, 100.512, 100.022);

    assert!((s.offset() - 0.5).abs() < 1e-9, "offset = {}", s.offset());
    assert!((s.delay() - 0.020).abs() < 1e-9, "delay = {}", s.delay());
    assert!((s.local_time() - 100.011).abs() < 1e-9);
}

/// Asymetria sciezki przenosi sie na blad offsetu rowny polowie roznicy opoznien.
#[test]
fn asymmetric_path_biases_offset_by_half_the_difference() {
    // droga tam 30 ms, droga z powrotem 10 ms, offset prawdziwy 0
    let s = ClockSample::new(0.000, 0.030, 0.031, 0.041);

    assert!((s.offset() - 0.010).abs() < 1e-9, "offset = {}", s.offset());
    assert!((s.delay() - 0.040).abs() < 1e-9);
}

#[test]
fn zero_offset_for_identical_clocks() {
    let s = ClockSample::new(10.0, 10.005, 10.006, 10.011);

    assert!(s.offset().abs() < 1e-9);
    assert!((s.delay() - 0.010).abs() < 1e-9);
}
```

- [ ] **Step 5: Uruchom test i potwierdź, że nie przechodzi**

Run: `cargo test --test clock_sample`
Expected: FAIL, kompilacja nie przechodzi — `unresolved import tracker_core::clock::ClockSample` albo `no function or associated item named new`

- [ ] **Step 6: Napisz implementację**

`src/clock/sample.rs`:

```rust
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
```

- [ ] **Step 7: Utwórz zaślepkę binarki bindgen**

`src/bin/uniffi-bindgen.rs`:

```rust
fn main() {
    uniffi::uniffi_bindgen_main()
}
```

Binarka jest potrzebna dopiero w zadaniu 13, ale `Cargo.toml` już się do niej odwołuje, więc musi istnieć od teraz.

- [ ] **Step 8: Uruchom test i potwierdź, że przechodzi**

Run: `cargo test --test clock_sample`
Expected: PASS, 3 testy

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore src tests
git commit -m "feat(clock): szkielet crate tracker-core i model probki NTP"
```

---

### Task 2: Estymator synchronizacji

Najważniejszy moduł całego P0a. Reszta projektu stoi na tym, że wspólna oś czasu jest wiarygodna.

**Files:**
- Create: `src/clock/fit.rs`, `src/clock/estimator.rs`
- Modify: `src/clock/mod.rs`
- Test: `tests/support/mod.rs`, `tests/clock_estimator.rs`

**Interfaces:**
- Consumes: `ClockSample` z zadania 1
- Produces: `ClockFit { reference_time, offset_seconds, skew_ppm, residual_std_ms, sample_count }` oraz `ClockSyncEstimator` z `new()`, `add(&mut self, ClockSample)`, `fit(&self) -> Option<ClockFit>`, `count(&self) -> usize`, `reset(&mut self)`, polami publicznymi `window_size: usize`, `best_fraction: f64`, `minimum_samples: usize`

- [ ] **Step 1: Napisz pomocniki testowe**

`tests/support/mod.rs`:

```rust
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
```

- [ ] **Step 2: Napisz testy, które mają nie przejść**

`tests/clock_estimator.rs`:

```rust
mod support;

use support::{make_series, SeriesSpec};
use tracker_core::clock::{ClockSample, ClockSyncEstimator};

fn estimator_with(samples: &[ClockSample]) -> ClockSyncEstimator {
    let mut estimator = ClockSyncEstimator::new();
    for sample in samples {
        estimator.add(*sample);
    }
    estimator
}

#[test]
fn returns_none_below_minimum_sample_count() {
    let samples = make_series(&SeriesSpec { count: 5, ..Default::default() });
    assert!(estimator_with(&samples).fit().is_none());
}

#[test]
fn recovers_offset_with_no_skew() {
    let samples = make_series(&SeriesSpec {
        count: 60,
        offset0: 0.25,
        skew_ppm: 0.0,
        seed: 2,
        ..Default::default()
    });
    let fit = estimator_with(&samples).fit().expect("model powinien powstac");

    assert!((fit.offset_seconds - 0.25).abs() < 0.0005, "offset = {}", fit.offset_seconds);
    assert!(fit.skew_ppm.abs() < 5.0, "skew = {}", fit.skew_ppm);
    assert!(fit.residual_std_ms < 1.0, "residual = {}", fit.residual_std_ms);
}

#[test]
fn recovers_skew_over_long_series() {
    // 20 ppm przez godzine to 72 ms rozjazdu
    let samples = make_series(&SeriesSpec {
        count: 3600,
        offset0: 0.10,
        skew_ppm: 20.0,
        seed: 3,
        ..Default::default()
    });
    let fit = estimator_with(&samples).fit().expect("model powinien powstac");

    assert!((fit.skew_ppm - 20.0).abs() < 1.0, "skew = {}", fit.skew_ppm);
}

#[test]
fn outliers_are_rejected_by_delay_filter() {
    // co dziesiata probka ma 250 ms dodatkowego opoznienia w jedna strone
    let samples = make_series(&SeriesSpec {
        count: 300,
        offset0: 0.25,
        skew_ppm: 5.0,
        seed: 4,
        outlier_every: Some(10),
        ..Default::default()
    });
    let fit = estimator_with(&samples).fit().expect("model powinien powstac");

    assert!((fit.offset_seconds - 0.25).abs() < 0.002, "offset = {}", fit.offset_seconds);
    assert!((fit.skew_ppm - 5.0).abs() < 2.0, "skew = {}", fit.skew_ppm);
}

#[test]
fn survives_gap_in_series() {
    // 60 probek, przerwa 300 s, kolejne 60 probek
    let first = make_series(&SeriesSpec {
        start: 1000.0, count: 60, offset0: 0.10, skew_ppm: 10.0, seed: 5, ..Default::default()
    });
    let second = make_series(&SeriesSpec {
        start: 1360.0,
        count: 60,
        offset0: 0.10 + 10.0 * 1e-6 * 360.0,
        skew_ppm: 10.0,
        seed: 6,
        ..Default::default()
    });
    let all: Vec<_> = first.into_iter().chain(second).collect();
    let fit = estimator_with(&all).fit().expect("model powinien powstac");

    assert!((fit.skew_ppm - 10.0).abs() < 1.5, "skew = {}", fit.skew_ppm);
}

#[test]
fn window_drops_oldest_samples() {
    let mut estimator = ClockSyncEstimator::new();
    estimator.window_size = 50;
    for sample in make_series(&SeriesSpec { count: 200, offset0: 0.10, seed: 7, ..Default::default() }) {
        estimator.add(sample);
    }

    let fit = estimator.fit().expect("model powinien powstac");
    assert!(fit.sample_count <= 50, "sample_count = {}", fit.sample_count);
    // punkt odniesienia musi lezec w ostatnim oknie, nie na poczatku serii
    assert!(fit.reference_time > 1140.0, "reference = {}", fit.reference_time);
}

#[test]
fn reset_clears_samples() {
    let mut estimator = estimator_with(&make_series(&SeriesSpec::default()));
    assert!(estimator.count() > 0);
    estimator.reset();
    assert_eq!(estimator.count(), 0);
    assert!(estimator.fit().is_none());
}
```

- [ ] **Step 3: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test clock_estimator`
Expected: FAIL — `unresolved import tracker_core::clock::ClockSyncEstimator`

- [ ] **Step 4: Napisz `ClockFit`**

`src/clock/fit.rs`:

```rust
/// Wynik dopasowania liniowego modelu zegara.
///
/// Model: `offset(t) = offset_seconds + skew_ppm * 1e-6 * (t - reference_time)`,
/// gdzie `t` jest czasem lokalnym w sekundach.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct ClockFit {
    pub reference_time: f64,
    pub offset_seconds: f64,
    pub skew_ppm: f64,
    pub residual_std_ms: f64,
    pub sample_count: u32,
}

impl ClockFit {
    /// Model zerowy, uzywany gdy synchronizacja nie zdazyla sie ustalic.
    /// Nieskonczony rezyduał jasno sygnalizuje brak wiarygodnosci.
    pub fn unknown() -> Self {
        Self {
            reference_time: 0.0,
            offset_seconds: 0.0,
            skew_ppm: 0.0,
            residual_std_ms: f64::INFINITY,
            sample_count: 0,
        }
    }
}
```

- [ ] **Step 5: Napisz `ClockSyncEstimator`**

`src/clock/estimator.rs`:

```rust
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
        let variance = if chosen.len() > 2 { sum_squares / (n - 2.0) } else { 0.0 };

        Some(ClockFit {
            reference_time: reference,
            offset_seconds: mean_offset,
            skew_ppm: slope * 1e6,
            residual_std_ms: variance.sqrt() * 1000.0,
            sample_count: chosen.len() as u32,
        })
    }
}
```

- [ ] **Step 6: Podepnij moduły**

`src/clock/mod.rs`:

```rust
mod estimator;
mod fit;
mod sample;

pub use estimator::ClockSyncEstimator;
pub use fit::ClockFit;
pub use sample::ClockSample;
```

- [ ] **Step 7: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test clock_estimator`
Expected: PASS, 7 testów

- [ ] **Step 8: Commit**

```bash
git add src/clock tests
git commit -m "feat(clock): estymator offsetu i skew z filtrem best-delay"
```

---

### Task 3: Konwersja czasu między urządzeniami

**Files:**
- Create: `src/clock/model.rs`
- Modify: `src/clock/mod.rs`
- Test: `tests/clock_model.rs`

**Interfaces:**
- Consumes: `ClockFit` z zadania 2
- Produces: `ClockModel` z `ClockModel::new(fit: ClockFit)`, `peer_time(&self, local_time: f64) -> f64`, `local_time(&self, peer_time: f64) -> f64`, `fit(&self) -> ClockFit`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/clock_model.rs`:

```rust
use tracker_core::clock::{ClockFit, ClockModel};

fn sample_fit() -> ClockFit {
    ClockFit {
        reference_time: 1000.0,
        offset_seconds: 0.25,
        skew_ppm: 20.0,
        residual_std_ms: 0.8,
        sample_count: 75,
    }
}

#[test]
fn peer_time_at_reference_is_local_plus_offset() {
    let model = ClockModel::new(sample_fit());
    assert!((model.peer_time(1000.0) - 1000.25).abs() < 1e-9);
}

#[test]
fn skew_accumulates_over_time() {
    let model = ClockModel::new(sample_fit());
    // 20 ppm przez 3600 s to 72 ms
    let expected = 4600.25 + 0.072;
    assert!((model.peer_time(4600.0) - expected).abs() < 1e-9);
}

#[test]
fn round_trip_is_exact() {
    let model = ClockModel::new(sample_fit());
    let mut local = 900.0;
    while local <= 5000.0 {
        let peer = model.peer_time(local);
        assert!((model.local_time(peer) - local).abs() < 1e-6, "local = {}", local);
        local += 137.0;
    }
}

#[test]
fn zero_skew_behaves_as_constant_offset() {
    let flat = ClockFit {
        reference_time: 0.0,
        offset_seconds: -1.5,
        skew_ppm: 0.0,
        residual_std_ms: 0.1,
        sample_count: 20,
    };
    let model = ClockModel::new(flat);
    assert!((model.peer_time(12345.0) - 12343.5).abs() < 1e-9);
    assert!((model.local_time(12343.5) - 12345.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test clock_model`
Expected: FAIL — `unresolved import tracker_core::clock::ClockModel`

- [ ] **Step 3: Napisz implementację**

`src/clock/model.rs`:

```rust
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
```

- [ ] **Step 4: Podepnij moduł**

W `src/clock/mod.rs` dodaj `mod model;` oraz `pub use model::ClockModel;`.

- [ ] **Step 5: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test clock_model`
Expected: PASS, 4 testy

- [ ] **Step 6: Commit**

```bash
git add src/clock tests/clock_model.rs
git commit -m "feat(clock): konwersja czasu miedzy urzadzeniami"
```

---

### Task 4: Typy sesji i protokół wiadomości

**Files:**
- Create: `src/session/mod.rs`, `src/session/profile.rs`
- Create: `src/link/mod.rs`, `src/link/message.rs`, `src/link/codec.rs`
- Modify: `src/lib.rs`, `Cargo.toml`
- Test: `tests/link_codec.rs`

**Interfaces:**
- Consumes: `ClockSample` z zadania 1
- Produces: `SessionRole` (`Master`, `Slave`, z `opposite()`), `CaptureProfile` (`P1080p120`, `P4k60`, `P1080p60`, `P1080p30`, z `width()`, `height()`, `fps()`, `as_str()`, `from_str()`), `LinkMessage` (enum), `LinkEnvelope { sequence: u64, message: LinkMessage }`, `encode(&LinkEnvelope) -> Result<Vec<u8>, CodecError>`, `decode(&[u8]) -> Result<LinkEnvelope, CodecError>`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/link_codec.rs`:

```rust
use tracker_core::clock::ClockSample;
use tracker_core::link::{decode, encode, LinkEnvelope, LinkMessage};
use tracker_core::session::{CaptureProfile, SessionRole};

fn round_trip(message: LinkMessage) {
    let envelope = LinkEnvelope { sequence: 42, message };
    let bytes = encode(&envelope).expect("kodowanie powinno sie udac");
    let decoded = decode(&bytes).expect("dekodowanie powinno sie udac");
    assert_eq!(decoded, envelope);
}

#[test]
fn round_trips_every_message_case() {
    round_trip(LinkMessage::Hello {
        device_model: "iPhone17,2".into(),
        app_version: "0.1.0".into(),
        preferred_role: SessionRole::Master,
    });
    round_trip(LinkMessage::RoleAssigned { role: SessionRole::Slave });
    round_trip(LinkMessage::Ping { id: 7, t1: 1000.5 });
    round_trip(LinkMessage::Pong { id: 7, t1: 1000.5, t2: 1000.75, t3: 1000.751 });
    round_trip(LinkMessage::StartRecording {
        session_id: "s-1".into(),
        profile: CaptureProfile::P1080p120,
        host_time: 12.5,
    });
    round_trip(LinkMessage::StopRecording { host_time: 900.0 });
    round_trip(LinkMessage::Heartbeat { host_time: 33.0 });
    round_trip(LinkMessage::SyncLog {
        samples: vec![ClockSample::new(1.0, 2.0, 3.0, 4.0)],
    });
}

#[test]
fn decoding_garbage_fails() {
    assert!(decode(&[0x00, 0x01, 0x02]).is_err());
}

#[test]
fn capture_profile_geometry() {
    assert_eq!(CaptureProfile::P1080p120.width(), 1920);
    assert_eq!(CaptureProfile::P1080p120.height(), 1080);
    assert_eq!(CaptureProfile::P1080p120.fps(), 120);

    assert_eq!(CaptureProfile::P4k60.width(), 3840);
    assert_eq!(CaptureProfile::P4k60.height(), 2160);
    assert_eq!(CaptureProfile::P4k60.fps(), 60);

    assert_eq!(CaptureProfile::P1080p60.width(), 1920);
    assert_eq!(CaptureProfile::P1080p60.fps(), 60);

    assert_eq!(CaptureProfile::P1080p30.width(), 1920);
    assert_eq!(CaptureProfile::P1080p30.fps(), 30);
}

#[test]
fn profile_strings_match_manifest_vocabulary() {
    assert_eq!(CaptureProfile::P1080p120.as_str(), "1080p120");
    assert_eq!(CaptureProfile::P4k60.as_str(), "4K60");
    assert_eq!(CaptureProfile::P1080p60.as_str(), "1080p60");
    assert_eq!(CaptureProfile::P1080p30.as_str(), "1080p30");

    assert_eq!(CaptureProfile::from_str("1080p120"), Some(CaptureProfile::P1080p120));
    assert_eq!(CaptureProfile::from_str("4K60"), Some(CaptureProfile::P4k60));
    assert_eq!(CaptureProfile::from_str("bzdura"), None);
}

#[test]
fn role_has_an_opposite() {
    assert_eq!(SessionRole::Master.opposite(), SessionRole::Slave);
    assert_eq!(SessionRole::Slave.opposite(), SessionRole::Master);
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test link_codec`
Expected: FAIL — `unresolved import tracker_core::link`

- [ ] **Step 3: Dodaj `thiserror` do zależności**

W `Cargo.toml`, w sekcji `[dependencies]`, dopisz:

```toml
thiserror = "1"
```

- [ ] **Step 4: Napisz typy sesji**

`src/session/profile.rs`:

```rust
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
        use serde::Deserialize;
        let raw = String::deserialize(deserializer)?;
        CaptureProfile::from_str(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("nieznany profil: {raw}")))
    }
}
```

`src/session/mod.rs`:

```rust
mod profile;

pub use profile::{CaptureProfile, SessionRole};
```

- [ ] **Step 5: Napisz protokół i kodek**

`src/link/message.rs`:

```rust
use crate::clock::ClockSample;
use crate::session::{CaptureProfile, SessionRole};

/// Wiadomosci wymieniane miedzy urzadzeniami.
///
/// Kazda wiadomosc niesie wlasne znaczniki czasu i jest samoopisujaca,
/// dzieki czemu odbiorca nie zaklada nic o kolejnosci dostarczenia.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
pub enum LinkMessage {
    Hello { device_model: String, app_version: String, preferred_role: SessionRole },
    RoleAssigned { role: SessionRole },
    Ping { id: u64, t1: f64 },
    Pong { id: u64, t1: f64, t2: f64, t3: f64 },
    StartRecording { session_id: String, profile: CaptureProfile, host_time: f64 },
    StopRecording { host_time: f64 },
    Heartbeat { host_time: f64 },
    SyncLog { samples: Vec<ClockSample> },
}

/// Koperta z numerem porzadkowym. Numer sluzy wylacznie do wykrywania
/// duplikatow — kolejnosc dostarczenia nie ma znaczenia dla semantyki.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct LinkEnvelope {
    pub sequence: u64,
    pub message: LinkMessage,
}
```

`src/link/codec.rs`:

```rust
use super::LinkEnvelope;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CodecError {
    #[error("serialization error: {reason}")]
    Serialization { reason: String },
}

pub fn encode(envelope: &LinkEnvelope) -> Result<Vec<u8>, CodecError> {
    serde_json::to_vec(envelope).map_err(|error| CodecError::Serialization {
        reason: error.to_string(),
    })
}

pub fn decode(bytes: &[u8]) -> Result<LinkEnvelope, CodecError> {
    serde_json::from_slice(bytes).map_err(|error| CodecError::Serialization {
        reason: error.to_string(),
    })
}
```

`src/link/mod.rs`:

```rust
mod codec;
mod message;

pub use codec::{decode, encode, CodecError};
pub use message::{LinkEnvelope, LinkMessage};
```

W `src/lib.rs` dodaj `pub mod link;` oraz `pub mod session;`.

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test link_codec`
Expected: PASS, 5 testów

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/link src/session src/lib.rs tests/link_codec.rs
git commit -m "feat(link): typy sesji, protokol wiadomosci i kodek"
```

---

### Task 5: Warstwa łącza

**Files:**
- Create: `src/link/peer.rs`
- Modify: `src/link/mod.rs`, `tests/support/mod.rs`
- Test: `tests/peer_link.rs`

**Interfaces:**
- Consumes: `LinkEnvelope`, `LinkMessage`, `encode`, `decode` z zadania 4
- Produces: `Delivery` (`Reliable`, `Unreliable`), trait `PeerTransport` z `send(&self, data: Vec<u8>, delivery: Delivery) -> Result<(), TransportError>` i `is_connected(&self) -> bool`; `PeerLink` z `new(transport: Arc<dyn PeerTransport>)`, `send(&self, LinkMessage) -> Result<(), LinkError>`, `receive(&self, data: &[u8]) -> Option<LinkMessage>`, `set_connected(&self, bool)`, `is_connected(&self) -> bool`, `malformed_packet_count(&self) -> u64`, `remembered_sequence_count(&self) -> usize`; błędy `TransportError::SendFailed { reason }` i `LinkError::{NotConnected, Transport, Codec}`

Decyzja projektowa: **`PeerLink` nie ma callbacków.** Warstwa natywna woła `receive()` i sama decyduje, co zrobić ze zwróconą wiadomością. Callbacki przez granicę FFI są kosztowne, trudne do testowania i wymuszają uważność przy czasie życia obiektów; zwracanie wartości jest prostsze i w pełni testowalne.

- [ ] **Step 1: Dopisz atrapę transportu do pomocników testowych**

Na końcu `tests/support/mod.rs`:

```rust
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
```

- [ ] **Step 2: Napisz testy, które mają nie przejść**

`tests/peer_link.rs`:

```rust
mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::link::{encode, Delivery, LinkEnvelope, LinkMessage, PeerLink};

fn setup() -> (Arc<FakeTransport>, PeerLink) {
    let transport = Arc::new(FakeTransport::new());
    let link = PeerLink::new(transport.clone());
    (transport, link)
}

fn heartbeat(host_time: f64) -> LinkMessage {
    LinkMessage::Heartbeat { host_time }
}

fn packet(sequence: u64, host_time: f64) -> Vec<u8> {
    encode(&LinkEnvelope { sequence, message: heartbeat(host_time) }).unwrap()
}

#[test]
fn assigns_increasing_sequence_numbers() {
    let (transport, link) = setup();
    link.send(heartbeat(1.0)).unwrap();
    link.send(heartbeat(2.0)).unwrap();
    link.send(heartbeat(3.0)).unwrap();

    let sequences: Vec<u64> = transport.sent_envelopes().iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![0, 1, 2]);
}

#[test]
fn delivers_received_message() {
    let (_transport, link) = setup();
    assert_eq!(link.receive(&packet(0, 5.0)), Some(heartbeat(5.0)));
}

#[test]
fn duplicate_sequence_is_dropped() {
    let (_transport, link) = setup();
    let data = packet(9, 5.0);
    assert!(link.receive(&data).is_some());
    assert!(link.receive(&data).is_none());
}

#[test]
fn out_of_order_messages_are_all_delivered() {
    let (_transport, link) = setup();
    assert_eq!(link.receive(&packet(2, 2.0)), Some(heartbeat(2.0)));
    assert_eq!(link.receive(&packet(1, 1.0)), Some(heartbeat(1.0)));
}

#[test]
fn malformed_packet_is_ignored_and_counted() {
    let (_transport, link) = setup();
    assert!(link.receive(&[0xFF, 0xFE]).is_none());
    assert_eq!(link.malformed_packet_count(), 1);
}

#[test]
fn send_while_disconnected_fails() {
    let (_transport, link) = setup();
    link.set_connected(false);
    assert!(link.send(heartbeat(1.0)).is_err());
}

#[test]
fn transport_failure_is_reported() {
    let (transport, link) = setup();
    transport.set_fail_send(true);
    assert!(link.send(heartbeat(1.0)).is_err());
}

#[test]
fn connection_state_is_tracked() {
    let (_transport, link) = setup();
    assert!(link.is_connected());
    link.set_connected(false);
    assert!(!link.is_connected());
    link.set_connected(true);
    assert!(link.is_connected());
}

/// Pingi po niezawodnym kanale zepsulyby pomiar opoznienia,
/// wiec trasowanie trybu jest czescia kontraktu warstwy lacza.
#[test]
fn pings_go_unreliable_and_commands_go_reliable() {
    let (transport, link) = setup();
    link.send(LinkMessage::Ping { id: 1, t1: 10.0 }).unwrap();
    link.send(LinkMessage::Pong { id: 1, t1: 10.0, t2: 10.1, t3: 10.2 }).unwrap();
    link.send(heartbeat(11.0)).unwrap();
    link.send(LinkMessage::StopRecording { host_time: 12.0 }).unwrap();

    assert_eq!(
        transport.sent_deliveries(),
        vec![Delivery::Unreliable, Delivery::Unreliable, Delivery::Reliable, Delivery::Reliable]
    );
}

#[test]
fn sequence_memory_is_bounded() {
    let (_transport, link) = setup();
    for sequence in 0..5000u64 {
        assert!(link.receive(&packet(sequence, sequence as f64)).is_some());
    }
    assert!(link.remembered_sequence_count() <= 1024);
}
```

- [ ] **Step 3: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test peer_link`
Expected: FAIL — `unresolved import tracker_core::link::PeerLink`

- [ ] **Step 4: Napisz implementację**

`src/link/peer.rs`:

```rust
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use super::{decode, encode, LinkEnvelope, LinkMessage};

/// Ile ostatnich numerow porzadkowych pamietamy do wykrywania duplikatow.
const SEQUENCE_MEMORY: usize = 1024;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TransportError {
    #[error("send failed: {reason}")]
    SendFailed { reason: String },
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LinkError {
    #[error("not connected")]
    NotConnected,
    #[error("transport error: {reason}")]
    Transport { reason: String },
    #[error("codec error: {reason}")]
    Codec { reason: String },
}

/// Tryb dostarczenia pakietu.
///
/// Rozroznienie jest wymogiem, nie optymalizacja. Pomiar synchronizacji
/// zaklada, ze opoznienie pakietu odzwierciedla rzeczywista droge. TCP
/// z retransmisjami, blokowaniem czola kolejki i algorytmem Nagle'a
/// **klamie o opoznieniu** — jeden zgubiony pakiet i probka pokazuje
/// 200 ms zamiast 5. Filtr best-delay to odrzuci, ale kosztem polowy probek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Delivery {
    /// Komendy: musza dojsc, opoznienie nieistotne. TCP albo MPC `.reliable`.
    Reliable,
    /// Pingi synchronizacyjne: opoznienie musi byc prawdziwe, zgubione
    /// pakiety sa bez znaczenia. UDP albo MPC `.unreliable`.
    Unreliable,
}

/// Surowy transport pakietow. Implementacje produkcyjne opieraja sie na
/// MultipeerConnectivity albo BLE; testy podstawiaja atrape w pamieci.
pub trait PeerTransport: Send + Sync {
    fn send(&self, data: Vec<u8>, delivery: Delivery) -> Result<(), TransportError>;
    fn is_connected(&self) -> bool;
}

struct LinkState {
    next_sequence: u64,
    seen: HashSet<u64>,
    seen_order: VecDeque<u64>,
    malformed: u64,
    connected: bool,
}

/// Warstwa nad surowym transportem: numeruje wysylane koperty,
/// odrzuca duplikaty i izoluje reszte systemu od uszkodzonych pakietow.
///
/// Kolejnosc dostarczenia nie jest wymuszana. Kazda wiadomosc niesie
/// wlasne znaczniki czasu, wiec przetasowanie niczego nie psuje, a
/// wymuszanie kolejnosci kosztowaloby bufor i opoznienie.
pub struct PeerLink {
    transport: Arc<dyn PeerTransport>,
    state: Mutex<LinkState>,
}

impl PeerLink {
    pub fn new(transport: Arc<dyn PeerTransport>) -> Self {
        Self {
            transport,
            state: Mutex::new(LinkState {
                next_sequence: 0,
                seen: HashSet::new(),
                seen_order: VecDeque::new(),
                malformed: 0,
                connected: true,
            }),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state.lock().unwrap().connected
    }

    pub fn set_connected(&self, connected: bool) {
        self.state.lock().unwrap().connected = connected;
    }

    pub fn malformed_packet_count(&self) -> u64 {
        self.state.lock().unwrap().malformed
    }

    pub fn remembered_sequence_count(&self) -> usize {
        self.state.lock().unwrap().seen.len()
    }

    /// Pingi i pongi ida trybem `Unreliable`, reszta `Reliable`.
    /// Wybor nalezy do warstwy laczа, nie do wolajacego — inaczej ktos
    /// predzej czy pozniej wysle ping po TCP i zepsuje synchronizacje.
    fn delivery_for(message: &LinkMessage) -> Delivery {
        match message {
            LinkMessage::Ping { .. } | LinkMessage::Pong { .. } => Delivery::Unreliable,
            _ => Delivery::Reliable,
        }
    }

    pub fn send(&self, message: LinkMessage) -> Result<(), LinkError> {
        let delivery = Self::delivery_for(&message);
        let sequence = {
            let mut state = self.state.lock().unwrap();
            if !state.connected {
                return Err(LinkError::NotConnected);
            }
            let sequence = state.next_sequence;
            state.next_sequence += 1;
            sequence
        };

        let bytes = encode(&LinkEnvelope { sequence, message })
            .map_err(|error| LinkError::Codec { reason: error.to_string() })?;

        self.transport
            .send(bytes, delivery)
            .map_err(|error| LinkError::Transport { reason: error.to_string() })
    }

    /// Zwraca wiadomosc, jesli pakiet jest poprawny i nie jest duplikatem.
    pub fn receive(&self, data: &[u8]) -> Option<LinkMessage> {
        let envelope = match decode(data) {
            Ok(envelope) => envelope,
            Err(_) => {
                self.state.lock().unwrap().malformed += 1;
                return None;
            }
        };

        let mut state = self.state.lock().unwrap();
        if state.seen.contains(&envelope.sequence) {
            return None;
        }
        state.seen.insert(envelope.sequence);
        state.seen_order.push_back(envelope.sequence);
        if state.seen_order.len() > SEQUENCE_MEMORY {
            if let Some(evicted) = state.seen_order.pop_front() {
                state.seen.remove(&evicted);
            }
        }
        Some(envelope.message)
    }
}
```

W `src/link/mod.rs` dodaj `mod peer;` oraz `pub use peer::{Delivery, LinkError, PeerLink, PeerTransport, TransportError};`.

- [ ] **Step 5: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test peer_link`
Expected: PASS, 10 testów

- [ ] **Step 6: Commit**

```bash
git add src/link tests
git commit -m "feat(link): warstwa laczna z deduplikacja i stanem polaczenia"
```

---

### Task 6: Sesja synchronizacji

**Files:**
- Create: `src/clock/session.rs`
- Modify: `src/clock/mod.rs`
- Test: `tests/sync_session.rs`

**Interfaces:**
- Consumes: `PeerLink` (zadanie 5), `ClockSyncEstimator`, `ClockFit`, `ClockSample` (zadania 1-2), `LinkMessage` (zadanie 4)
- Produces: `SyncGap { start_host_time: f64, end_host_time: Option<f64>, reason: String }`, `SyncSnapshot { model: ClockFit, samples: Vec<ClockSample>, gaps: Vec<SyncGap> }`, `SyncSession` z `new(link: Arc<PeerLink>)`, `start(&mut self, now: f64)`, `tick(&mut self, now: f64)`, `handle(&mut self, message: &LinkMessage, received_at: f64)`, `link_did_change(&mut self, connected: bool, at: f64)`, `current_fit(&self) -> Option<ClockFit>`, `samples(&self) -> &[ClockSample]`, `gaps(&self) -> &[SyncGap]`, `snapshot(&self) -> SyncSnapshot`, polami `burst_count: usize`, `steady_period: f64`

`SyncSession` nie mierzy czasu samodzielnie — czas podaje wołający przez `now` i `received_at`. Dzięki temu testy sterują jego upływem bez czekania.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/sync_session.rs`:

```rust
mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::clock::SyncSession;
use tracker_core::link::{LinkMessage, PeerLink};

fn setup() -> (Arc<FakeTransport>, Arc<PeerLink>, SyncSession) {
    let transport = Arc::new(FakeTransport::new());
    let link = Arc::new(PeerLink::new(transport.clone()));
    let session = SyncSession::new(link.clone());
    (transport, link, session)
}

fn ping_count(transport: &FakeTransport) -> usize {
    transport
        .sent_messages()
        .iter()
        .filter(|m| matches!(m, LinkMessage::Ping { .. }))
        .count()
}

/// Odpowiada na wszystkie wyslane pingi tak, jak zrobilby to peer
/// o zadanym offsecie i symetrycznej sciezce.
fn reply_to_pings(transport: &FakeTransport, session: &mut SyncSession, offset: f64) {
    let one_way = 0.005;
    for message in transport.sent_messages() {
        if let LinkMessage::Ping { id, t1 } = message {
            let t2 = t1 + one_way + offset;
            let t3 = t2 + 0.001;
            let t4 = t3 - offset + one_way;
            session.handle(&LinkMessage::Pong { id, t1, t2, t3 }, t4);
        }
    }
}

#[test]
fn burst_sends_fifty_pings_on_start() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    assert_eq!(ping_count(&transport), 50);
}

#[test]
fn produces_fit_after_burst_is_answered() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    reply_to_pings(&transport, &mut session, 0.4);

    let fit = session.current_fit().expect("model powinien powstac");
    assert!((fit.offset_seconds - 0.4).abs() < 0.001, "offset = {}", fit.offset_seconds);
}

#[test]
fn steady_state_sends_one_ping_per_second() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    transport.clear_sent();

    session.tick(1000.5);
    assert_eq!(transport.sent_count(), 0);

    session.tick(1001.0);
    assert_eq!(transport.sent_count(), 1);

    session.tick(1002.0);
    assert_eq!(transport.sent_count(), 2);
}

#[test]
fn unmatched_pong_is_ignored() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.handle(&LinkMessage::Pong { id: 99_999, t1: 1.0, t2: 2.0, t3: 3.0 }, 4.0);
    assert_eq!(session.samples().len(), 0);
}

#[test]
fn ping_is_answered_with_pong() {
    let (transport, _link, mut session) = setup();
    session.handle(&LinkMessage::Ping { id: 5, t1: 700.0 }, 700.31);

    let last = transport.sent_messages().pop().expect("oczekiwano odpowiedzi");
    match last {
        LinkMessage::Pong { id, t1, t2, t3 } => {
            assert_eq!(id, 5);
            assert!((t1 - 700.0).abs() < 1e-9);
            assert!((t2 - 700.31).abs() < 1e-9);
            assert!(t3 >= t2);
        }
        other => panic!("oczekiwano ponga, otrzymano {other:?}"),
    }
}

#[test]
fn gap_is_recorded_when_link_drops() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1100.0);
    session.link_did_change(true, 1160.0);

    assert_eq!(session.gaps().len(), 1);
    let gap = &session.gaps()[0];
    assert!((gap.start_host_time - 1100.0).abs() < 1e-9);
    assert_eq!(gap.end_host_time, Some(1160.0));
    assert_eq!(gap.reason, "link-lost");
}

#[test]
fn gap_stays_open_while_disconnected() {
    let (_transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1100.0);

    assert_eq!(session.gaps().len(), 1);
    assert_eq!(session.gaps()[0].end_host_time, None);
}

#[test]
fn no_pings_are_sent_while_disconnected() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    session.link_did_change(false, 1000.0);
    transport.clear_sent();

    session.tick(1005.0);

    assert_eq!(transport.sent_count(), 0);
}

#[test]
fn snapshot_carries_model_samples_and_gaps() {
    let (transport, _link, mut session) = setup();
    session.start(1000.0);
    reply_to_pings(&transport, &mut session, 0.4);
    session.link_did_change(false, 1100.0);
    session.link_did_change(true, 1120.0);

    let snapshot = session.snapshot();
    assert!((snapshot.model.offset_seconds - 0.4).abs() < 0.001);
    assert_eq!(snapshot.samples.len(), 50);
    assert_eq!(snapshot.gaps.len(), 1);
}

#[test]
fn snapshot_without_fit_reports_infinite_residual() {
    let (_transport, _link, session) = setup();
    let snapshot = session.snapshot();
    assert!(snapshot.model.residual_std_ms.is_infinite());
    assert_eq!(snapshot.model.sample_count, 0);
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test sync_session`
Expected: FAIL — `unresolved import tracker_core::clock::SyncSession`

- [ ] **Step 3: Napisz implementację**

`src/clock/session.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use super::{ClockFit, ClockSample, ClockSyncEstimator};
use crate::link::{LinkMessage, PeerLink};

/// Luka w ciaglosci synchronizacji. `end_host_time` jest `None`,
/// dopoki luka trwa.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncGap {
    pub start_host_time: f64,
    pub end_host_time: Option<f64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SyncSnapshot {
    pub model: ClockFit,
    pub samples: Vec<ClockSample>,
    pub gaps: Vec<SyncGap>,
}

/// Prowadzi synchronizacje zegarow: wysyla pingi, odpowiada na cudze,
/// laczy odpowiedzi w probki i utrzymuje aktualny model.
///
/// Klasa nie odczytuje zegara samodzielnie — czas jest podawany z zewnatrz.
/// Dzieki temu testy sterują uplywem czasu bez czekania, a produkcja
/// wstrzykuje zegar monotoniczny.
pub struct SyncSession {
    /// Liczba pingow w serii startowej.
    pub burst_count: usize,
    /// Odstep miedzy pingami w stanie ustalonym.
    pub steady_period: f64,

    link: Arc<PeerLink>,
    estimator: ClockSyncEstimator,
    samples: Vec<ClockSample>,
    gaps: Vec<SyncGap>,
    current_fit: Option<ClockFit>,
    pending: HashMap<u64, f64>,
    next_ping_id: u64,
    last_ping_time: f64,
    connected: bool,
}

impl SyncSession {
    pub fn new(link: Arc<PeerLink>) -> Self {
        Self {
            burst_count: 50,
            steady_period: 1.0,
            link,
            estimator: ClockSyncEstimator::new(),
            samples: Vec::new(),
            gaps: Vec::new(),
            current_fit: None,
            pending: HashMap::new(),
            next_ping_id: 0,
            last_ping_time: f64::NEG_INFINITY,
            connected: true,
        }
    }

    /// Seria startowa. Wysylana jednym ciagiem — na lokalnej sieci
    /// zajmuje ulamek sekundy, a od razu daje uzyteczny model.
    pub fn start(&mut self, now: f64) {
        for _ in 0..self.burst_count {
            self.send_ping(now);
        }
        self.last_ping_time = now;
    }

    pub fn tick(&mut self, now: f64) {
        if !self.connected {
            return;
        }
        if now - self.last_ping_time < self.steady_period {
            return;
        }
        self.send_ping(now);
        self.last_ping_time = now;
    }

    pub fn link_did_change(&mut self, connected: bool, at: f64) {
        if connected == self.connected {
            return;
        }
        self.connected = connected;
        if connected {
            if let Some(gap) = self.gaps.last_mut() {
                if gap.end_host_time.is_none() {
                    gap.end_host_time = Some(at);
                }
            }
            self.last_ping_time = f64::NEG_INFINITY;
        } else {
            self.gaps.push(SyncGap {
                start_host_time: at,
                end_host_time: None,
                reason: "link-lost".into(),
            });
            self.pending.clear();
        }
    }

    pub fn handle(&mut self, message: &LinkMessage, received_at: f64) {
        match message {
            LinkMessage::Ping { id, t1 } => {
                // Odpowiadamy natychmiast; t2 i t3 roznia sie o czas obslugi,
                // ktory na tej sciezce jest ponizej rozdzielczosci pomiaru.
                let _ = self.link.send(LinkMessage::Pong {
                    id: *id,
                    t1: *t1,
                    t2: received_at,
                    t3: received_at,
                });
            }
            LinkMessage::Pong { id, t1, t2, t3 } => {
                if self.pending.remove(id).is_none() {
                    return;
                }
                let sample = ClockSample::new(*t1, *t2, *t3, received_at);
                self.samples.push(sample);
                self.estimator.add(sample);
                self.current_fit = self.estimator.fit();
            }
            _ => {}
        }
    }

    pub fn current_fit(&self) -> Option<ClockFit> {
        self.current_fit
    }

    pub fn samples(&self) -> &[ClockSample] {
        &self.samples
    }

    pub fn gaps(&self) -> &[SyncGap] {
        &self.gaps
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        SyncSnapshot {
            model: self.current_fit.unwrap_or_else(ClockFit::unknown),
            samples: self.samples.clone(),
            gaps: self.gaps.clone(),
        }
    }

    fn send_ping(&mut self, now: f64) {
        let id = self.next_ping_id;
        self.next_ping_id += 1;
        self.pending.insert(id, now);
        let _ = self.link.send(LinkMessage::Ping { id, t1: now });
    }
}
```

W `src/clock/mod.rs` dodaj `mod session;` oraz `pub use session::{SyncGap, SyncSession, SyncSnapshot};`.

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test sync_session`
Expected: PASS, 10 testów

- [ ] **Step 5: Commit**

```bash
git add src/clock tests/sync_session.rs
git commit -m "feat(clock): sesja synchronizacji z harmonogramem pingow i lukami"
```

---

### Task 7: Raport zdolności urządzenia

**Files:**
- Create: `src/device/mod.rs`, `src/device/capability.rs`
- Modify: `src/lib.rs`
- Test: `tests/capability.rs`

**Interfaces:**
- Consumes: nic
- Produces: `TimestampSource` (`Realtime`, `Unknown`), `DeviceTier` (`Full`, `Limited`, `Rejected`, z `as_str()`), `CapabilityReport { tier, manual_sensor, timestamp_source, timestamp_base_offset_ms: Option<f64>, timestamp_base_uncertainty_ms: Option<f64>, intrinsics_available, max_fps: u32, min_exposure_seconds: f64 }`, `CapabilityProbeResult` (surowe odczyty) z `evaluate() -> CapabilityReport`, oraz `estimate_timestamp_base_offset(pairs: &[(f64, f64)]) -> Option<(f64, f64)>`

Uzasadnienie podziału: `CapabilityProbeResult` to **surowe odczyty** z platformy, `CapabilityReport` to **werdykt**. Reguła przyznawania poziomu jest logiką, więc mieszka w rdzeniu i jest testowalna; odczyt wartości jest platformowy i mieszka w adapterze.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/capability.rs`:

```rust
use tracker_core::device::{
    estimate_timestamp_base_offset, CapabilityProbeResult, DeviceTier, TimestampSource,
};

fn ios_probe() -> CapabilityProbeResult {
    CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
}

#[test]
fn full_tier_for_capable_device() {
    let report = ios_probe().evaluate();
    assert_eq!(report.tier, DeviceTier::Full);
    assert_eq!(report.max_fps, 120);
    assert!(report.intrinsics_available);
}

#[test]
fn missing_manual_sensor_drops_to_limited() {
    let probe = CapabilityProbeResult { manual_sensor: false, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn unknown_timestamp_source_drops_to_limited() {
    let probe = CapabilityProbeResult {
        timestamp_source: TimestampSource::Unknown,
        ..ios_probe()
    };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn missing_exposure_lock_is_rejected() {
    let probe = CapabilityProbeResult { manual_sensor: false, exposure_lock: false, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Rejected);
}

#[test]
fn too_few_frames_per_second_is_rejected() {
    let probe = CapabilityProbeResult { max_fps: 24, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Rejected);
}

#[test]
fn exposure_too_long_drops_to_limited() {
    // sensor nie schodzi ponizej 1/250 s
    let probe = CapabilityProbeResult { min_exposure_seconds: 0.004, ..ios_probe() };
    assert_eq!(probe.evaluate().tier, DeviceTier::Limited);
}

#[test]
fn tier_strings_match_spec_vocabulary() {
    assert_eq!(DeviceTier::Full.as_str(), "full");
    assert_eq!(DeviceTier::Limited.as_str(), "limited");
    assert_eq!(DeviceTier::Rejected.as_str(), "rejected");
}

#[test]
fn realtime_source_needs_no_base_offset() {
    let report = ios_probe().evaluate();
    assert_eq!(report.timestamp_base_offset_ms, None);
    assert_eq!(report.timestamp_base_uncertainty_ms, None);
}

#[test]
fn base_offset_is_estimated_from_minimum_delivery_lag() {
    // znacznik klatki lezy 2,0 s ponizej zegara monotonicznego,
    // a opoznienie dostarczenia waha sie od 5 do 40 ms
    let pairs = vec![
        (100.000, 102.040),
        (100.008, 102.013),
        (100.016, 102.021),
        (100.024, 102.029),
        (100.032, 102.037),
        (100.040, 102.045),
    ];
    let (offset_ms, uncertainty_ms) =
        estimate_timestamp_base_offset(&pairs).expect("estymacja powinna sie udac");

    // minimum roznicy to 2,005 s
    assert!((offset_ms - 2005.0).abs() < 1.0, "offset = {offset_ms}");
    assert!(uncertainty_ms > 0.0);
}

#[test]
fn base_offset_needs_enough_pairs() {
    assert!(estimate_timestamp_base_offset(&[(1.0, 2.0)]).is_none());
    assert!(estimate_timestamp_base_offset(&[]).is_none());
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test capability`
Expected: FAIL — `unresolved import tracker_core::device`

- [ ] **Step 3: Napisz implementację**

`src/device/capability.rs`:

```rust
/// Minimalna liczba par potrzebna do estymacji przesuniecia baz.
const MIN_TIMESTAMP_PAIRS: usize = 5;
/// Ponizej tylu klatek na sekunde nie ma sensu nagrywac.
const MIN_USABLE_FPS: u32 = 30;
/// Dluzsza ekspozycja rozmazuje pilke w kreske.
const MAX_USABLE_EXPOSURE_SECONDS: f64 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
pub enum TimestampSource {
    /// Znaczniki klatek leza na tej samej bazie co zegar monotoniczny.
    #[serde(rename = "REALTIME")]
    Realtime,
    /// Baza nieokreslona — wymaga estymacji przesuniecia.
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum DeviceTier {
    /// Wszystkie bramki spelnione; dane uzyteczne takze do stereo.
    Full,
    /// Nagrywa, ale z brakami odnotowanymi w manifescie.
    Limited,
    /// Ponizej progu uzytecznosci.
    Rejected,
}

impl DeviceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceTier::Full => "full",
            DeviceTier::Limited => "limited",
            DeviceTier::Rejected => "rejected",
        }
    }
}

/// Surowe odczyty z platformy. Wypelnia je adapter natywny.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CapabilityProbeResult {
    /// Manualna kontrola ekspozycji, ISO i czasu trwania klatki.
    pub manual_sensor: bool,
    /// Chociaz blokada automatycznej ekspozycji.
    pub exposure_lock: bool,
    pub focus_lock: bool,
    pub timestamp_source: TimestampSource,
    pub intrinsics_available: bool,
    pub max_fps: u32,
    pub min_exposure_seconds: f64,
}

/// Werdykt: co z tego urzadzenia da sie wycisnac.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    pub tier: DeviceTier,
    pub manual_sensor: bool,
    pub timestamp_source: TimestampSource,
    pub timestamp_base_offset_ms: Option<f64>,
    pub timestamp_base_uncertainty_ms: Option<f64>,
    pub intrinsics_available: bool,
    pub max_fps: u32,
    pub min_exposure_seconds: f64,
}

impl CapabilityProbeResult {
    /// Regula przyznawania poziomu.
    ///
    /// Odrzucamy tylko to, czego nie da sie uzyc do niczego: brak jakiejkolwiek
    /// blokady ekspozycji albo zbyt malo klatek. Reszta braków obniza poziom
    /// do `Limited`, bo nawet ograniczone nagranie ma wartosc — P1 potrzebuje
    /// roznorodnosci sensorow, zeby detektor nie przeuczyl sie na jeden aparat.
    pub fn evaluate(&self) -> CapabilityReport {
        let tier = if !self.exposure_lock || self.max_fps < MIN_USABLE_FPS {
            DeviceTier::Rejected
        } else if !self.manual_sensor
            || self.timestamp_source == TimestampSource::Unknown
            || self.min_exposure_seconds > MAX_USABLE_EXPOSURE_SECONDS
        {
            DeviceTier::Limited
        } else {
            DeviceTier::Full
        };

        CapabilityReport {
            tier,
            manual_sensor: self.manual_sensor,
            timestamp_source: self.timestamp_source,
            timestamp_base_offset_ms: None,
            timestamp_base_uncertainty_ms: None,
            intrinsics_available: self.intrinsics_available,
            max_fps: self.max_fps,
            min_exposure_seconds: self.min_exposure_seconds,
        }
    }
}

/// Estymuje przesuniecie baz zegarow z par (znacznik klatki, odczyt zegara
/// w chwili dostarczenia).
///
/// Ta sama sztuczka co przy synchronizacji: bierzemy **minimum** roznicy.
/// Najszybciej dostarczona klatka przeszla najkrotsza droga, wiec jej
/// roznica najlepiej przybliza przesuniecie baz. Rozrzut pozostalych
/// roznic sluzy za miare niepewnosci.
///
/// Zwraca `(przesuniecie_ms, niepewnosc_ms)`.
pub fn estimate_timestamp_base_offset(pairs: &[(f64, f64)]) -> Option<(f64, f64)> {
    if pairs.len() < MIN_TIMESTAMP_PAIRS {
        return None;
    }

    let deltas: Vec<f64> = pairs.iter().map(|(frame, delivered)| delivered - frame).collect();
    let minimum = deltas.iter().cloned().fold(f64::INFINITY, f64::min);

    let n = deltas.len() as f64;
    let mean = deltas.iter().sum::<f64>() / n;
    let variance = deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n;

    Some((minimum * 1000.0, variance.sqrt() * 1000.0))
}
```

`src/device/mod.rs`:

```rust
mod capability;

pub use capability::{
    estimate_timestamp_base_offset, CapabilityProbeResult, CapabilityReport, DeviceTier,
    TimestampSource,
};
```

W `src/lib.rs` dodaj `pub mod device;`.

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test capability`
Expected: PASS, 10 testów

- [ ] **Step 5: Commit**

```bash
git add src/device src/lib.rs tests/capability.rs
git commit -m "feat(device): raport zdolnosci i estymacja przesuniecia baz zegarow"
```

---

### Task 8: Ustawienie meczu

**Files:**
- Create: `src/session/match_setup.rs`
- Modify: `src/session/mod.rs`
- Test: `tests/match_setup.rs`

**Interfaces:**
- Consumes: nic
- Produces: `CourtEnd` (`North`, `South`, z `opposite()`), `Player { id: String, name: String, genitive: Option<String>, start_end: CourtEnd }`, `MatchFormat` (`SinglesAdTiebreak`, z `as_str()`), `MatchSetup { players: Vec<Player>, first_server: String, format: MatchFormat }` z `default_singles()`, `validate() -> Result<(), MatchSetupError>`, `player(&self, id: &str) -> Option<&Player>`, `end_of(&self, id: &str, completed_games: u32) -> Option<CourtEnd>`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/match_setup.rs`:

```rust
use tracker_core::session::{CourtEnd, MatchFormat, MatchSetup, Player};

fn setup() -> MatchSetup {
    MatchSetup {
        players: vec![
            Player {
                id: "p1".into(),
                name: "Piotr".into(),
                genitive: Some("Piotra".into()),
                start_end: CourtEnd::North,
            },
            Player {
                id: "p2".into(),
                name: "Marek".into(),
                genitive: Some("Marka".into()),
                start_end: CourtEnd::South,
            },
        ],
        first_server: "p1".into(),
        format: MatchFormat::SinglesAdTiebreak,
    }
}

#[test]
fn default_singles_is_valid_and_anonymous() {
    let default = MatchSetup::default_singles();
    assert!(default.validate().is_ok());
    assert_eq!(default.players.len(), 2);
    assert_eq!(default.players[0].name, "Player 1");
    assert_eq!(default.players[1].name, "Player 2");
    // Nazwy domyslne sa w jezyku bazowym (angielskim); UI podmienia je
    // na zlokalizowane przy wyswietlaniu, nie w rdzeniu.
    assert!(default.players.iter().all(|p| p.genitive.is_none()));
    assert_ne!(default.players[0].start_end, default.players[1].start_end);
}

#[test]
fn valid_setup_passes() {
    assert!(setup().validate().is_ok());
}

#[test]
fn rejects_duplicate_player_ids() {
    let mut s = setup();
    s.players[1].id = "p1".into();
    s.first_server = "p1".into();
    assert!(s.validate().is_err());
}

#[test]
fn rejects_unknown_first_server() {
    let mut s = setup();
    s.first_server = "p9".into();
    assert!(s.validate().is_err());
}

#[test]
fn rejects_both_players_on_the_same_end() {
    let mut s = setup();
    s.players[1].start_end = CourtEnd::North;
    assert!(s.validate().is_err());
}

#[test]
fn rejects_wrong_player_count_for_singles() {
    let mut s = setup();
    s.players.pop();
    assert!(s.validate().is_err());
}

#[test]
fn player_lookup_by_id() {
    let s = setup();
    assert_eq!(s.player("p2").map(|p| p.name.as_str()), Some("Marek"));
    assert!(s.player("p9").is_none());
}

/// Zmiana stron nastepuje po nieparzystych gemach: po 1., 3., 5. …
#[test]
fn ends_swap_after_odd_games() {
    let s = setup();
    assert_eq!(s.end_of("p1", 0), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 1), Some(CourtEnd::South));
    assert_eq!(s.end_of("p1", 2), Some(CourtEnd::South));
    assert_eq!(s.end_of("p1", 3), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 4), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 5), Some(CourtEnd::South));
}

#[test]
fn both_players_always_face_each_other() {
    let s = setup();
    for games in 0..12u32 {
        let a = s.end_of("p1", games).unwrap();
        let b = s.end_of("p2", games).unwrap();
        assert_ne!(a, b, "po {games} gemach gracze staneli po tej samej stronie");
    }
}

/// Dopelniacz jest opcjonalny — jego brak nie moze blokowac niczego,
/// bo oglaszanie domyslnie uzywa konstrukcji w mianowniku.
#[test]
fn genitive_is_optional_and_does_not_affect_validation() {
    let mut s = setup();
    assert_eq!(s.player("p1").and_then(|p| p.genitive.as_deref()), Some("Piotra"));

    s.players[0].genitive = None;
    assert!(s.validate().is_ok());
}

#[test]
fn court_end_has_an_opposite() {
    assert_eq!(CourtEnd::North.opposite(), CourtEnd::South);
    assert_eq!(CourtEnd::South.opposite(), CourtEnd::North);
}

#[test]
fn format_string_matches_manifest_vocabulary() {
    assert_eq!(MatchFormat::SinglesAdTiebreak.as_str(), "singles-ad-tiebreak");
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test match_setup`
Expected: FAIL — `unresolved import tracker_core::session::MatchSetup`

- [ ] **Step 3: Napisz implementację**

`src/session/match_setup.rs`:

```rust
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
```

W `src/session/mod.rs` dodaj `mod match_setup;` oraz `pub use match_setup::{CourtEnd, MatchFormat, MatchSetup, MatchSetupError, Player};`.

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test match_setup`
Expected: PASS, 12 testów

- [ ] **Step 5: Commit**

```bash
git add src/session tests/match_setup.rs
git commit -m "feat(session): ustawienie meczu z wiazaniem graczy do stron kortu"
```

---

### Task 9: Manifest sesji

Właściwy produkt P0a. Wideo bez tego pliku jest w P1 bezużyteczne.

**Files:**
- Create: `src/session/manifest.rs`
- Modify: `src/session/mod.rs`
- Test: `tests/manifest.rs`

**Interfaces:**
- Consumes: `SessionRole`, `CaptureProfile` (zadanie 4), `MatchSetup` (zadanie 8), `CapabilityReport` (zadanie 7), `ClockFit`, `ClockSample`, `SyncGap` (zadania 2 i 6)
- Produces: `Platform` (`Ios`, `Android`), `DeviceInfo`, `AudioInfo`, `WhiteBalanceGains`, `LockedCameraSettings`, `CameraInfo::new(...)`, `SyncModel::from_fit(&ClockFit)`, `SyncSampleRecord::from_sample(&ClockSample)`, `TimingInfo`, `SegmentInfo`, `SessionEvent` z konstruktorami `motion_spike`, `thermal`, `capture_interrupted`, `mark`, `session_stopped`; `SessionManifest` z `encode(&self) -> Result<String, ManifestError>`, `decode(&str) -> Result<SessionManifest, ManifestError>`, `validate(&self) -> Result<(), ManifestError>`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/manifest.rs`:

```rust
use tracker_core::clock::{ClockFit, ClockSample, SyncGap};
use tracker_core::device::{CapabilityProbeResult, TimestampSource};
use tracker_core::session::{
    AudioInfo, CameraInfo, CaptureProfile, DeviceInfo, LockedCameraSettings, MatchSetup, Platform,
    SegmentInfo, SessionEvent, SessionManifest, SessionRole, SyncModel, SyncSampleRecord,
    TimingInfo, WhiteBalanceGains,
};

fn capabilities() -> tracker_core::device::CapabilityReport {
    CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
    .evaluate()
}

fn sample_manifest() -> SessionManifest {
    SessionManifest {
        schema_version: 2,
        session_id: "2026-09-14T17:32:10Z-a3f9".into(),
        role: SessionRole::Master,
        platform: Platform::Ios,
        peer_device_model: Some("iPhone15,4".into()),
        match_setup: MatchSetup::default_singles(),
        device: DeviceInfo {
            model: "iPhone17,2".into(),
            os_version: "26.0".into(),
            app_version: "0.1.0".into(),
        },
        capabilities: capabilities(),
        camera: CameraInfo::new(
            "builtInWideAngleCamera".into(),
            CaptureProfile::P1080p120,
            "hevc".into(),
            40,
            AudioInfo { sample_rate_hz: 48000, channels: 1, codec: "pcm".into() },
            LockedCameraSettings {
                exposure_duration_seconds: 0.001,
                iso: 64.0,
                focus_lens_position: 0.82,
                white_balance_gains: WhiteBalanceGains { r: 1.9, g: 1.0, b: 1.6 },
            },
            Some(vec![
                vec![1580.0, 0.0, 960.0],
                vec![0.0, 1580.0, 540.0],
                vec![0.0, 0.0, 1.0],
            ]),
        ),
        timing: TimingInfo {
            clock_domain: "mach_absolute_time".into(),
            first_frame_host_time: 123456.789012,
            transport: "multipeer".into(),
            sync_model: SyncModel::from_fit(&ClockFit {
                reference_time: 123456.0,
                offset_seconds: 0.0142,
                skew_ppm: 7.3,
                residual_std_ms: 0.9,
                sample_count: 60,
            }),
            sync_samples: vec![SyncSampleRecord::from_sample(&ClockSample::new(
                123456.0, 123456.019, 123456.020, 123456.006,
            ))],
            sync_gaps: vec![SyncGap {
                start_host_time: 124000.0,
                end_host_time: Some(124035.0),
                reason: "link-lost".into(),
            }],
        },
        segments: vec![SegmentInfo {
            file: "video-000.mov".into(),
            start_host_time: 123456.789,
            end_host_time: 123756.789,
            frame_count: 36000,
        }],
        events: vec![SessionEvent::motion_spike(123500.1, 0.42)],
        motion_log: "motion.jsonl".into(),
    }
}

#[test]
fn json_round_trip() {
    let original = sample_manifest();
    let decoded = SessionManifest::decode(&original.encode().unwrap()).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn player_genitive_survives_round_trip() {
    let mut m = sample_manifest();
    m.match_setup.players[0].genitive = Some("Piotra".into());

    let decoded = SessionManifest::decode(&m.encode().unwrap()).unwrap();
    assert_eq!(decoded.match_setup.players[0].genitive.as_deref(), Some("Piotra"));
}

#[test]
fn encoded_json_uses_spec_keys() {
    let json = sample_manifest().encode().unwrap();
    for key in [
        "schemaVersion", "sessionId", "platform", "peerDeviceModel", "match",
        "capabilities", "timestampSource", "intrinsicMatrix", "firstFrameHostTime",
        "transport", "syncModel", "referenceHostTime", "skewPpm", "residualStdMs",
        "syncGaps", "targetBitrateMbps", "motionLog", "firstServer",
    ] {
        assert!(json.contains(&format!("\"{key}\"")), "brak klucza {key}");
    }
}

#[test]
fn profile_and_tier_are_encoded_as_spec_strings() {
    let json = sample_manifest().encode().unwrap();
    assert!(json.contains("\"1080p120\""));
    assert!(json.contains("\"full\""));
    assert!(json.contains("\"REALTIME\""));
    assert!(json.contains("\"singles-ad-tiebreak\""));
}

#[test]
fn camera_geometry_is_derived_from_profile() {
    let manifest = sample_manifest();
    assert_eq!(manifest.camera.width, 1920);
    assert_eq!(manifest.camera.height, 1080);
    assert_eq!(manifest.camera.fps, 120);
}

#[test]
fn validate_accepts_well_formed_manifest() {
    assert!(sample_manifest().validate().is_ok());
}

#[test]
fn validate_rejects_wrong_schema_version() {
    let mut m = sample_manifest();
    m.schema_version = 1;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_empty_segments() {
    let mut m = sample_manifest();
    m.segments.clear();
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_non_contiguous_segments() {
    let mut m = sample_manifest();
    m.segments = vec![
        SegmentInfo { file: "video-000.mov".into(), start_host_time: 100.0, end_host_time: 400.0, frame_count: 36000 },
        SegmentInfo { file: "video-001.mov".into(), start_host_time: 420.0, end_host_time: 720.0, frame_count: 36000 },
    ];
    m.timing.first_frame_host_time = 100.0;
    assert!(m.validate().is_err());
}

#[test]
fn validate_accepts_segments_within_one_frame() {
    let mut m = sample_manifest();
    m.segments = vec![
        SegmentInfo { file: "video-000.mov".into(), start_host_time: 100.0, end_host_time: 400.0, frame_count: 36000 },
        // 4 ms przerwy przy 120 fps to mniej niz jedna klatka (8,3 ms)
        SegmentInfo { file: "video-001.mov".into(), start_host_time: 400.004, end_host_time: 700.0, frame_count: 36000 },
    ];
    m.timing.first_frame_host_time = 100.0;
    assert!(m.validate().is_ok());
}

#[test]
fn validate_rejects_first_frame_outside_first_segment() {
    let mut m = sample_manifest();
    m.timing.first_frame_host_time = 999_999.0;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_malformed_intrinsics() {
    let mut m = sample_manifest();
    m.camera.intrinsic_matrix = Some(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    assert!(m.validate().is_err());
}

#[test]
fn validate_accepts_absent_intrinsics() {
    let mut m = sample_manifest();
    m.camera.intrinsic_matrix = None;
    assert!(m.validate().is_ok(), "Android zwykle nie udostepnia intrinsics");
}

#[test]
fn validate_rejects_non_finite_sync_model() {
    let mut m = sample_manifest();
    m.timing.sync_model.residual_std_ms = f64::INFINITY;
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_invalid_match_setup() {
    let mut m = sample_manifest();
    m.match_setup.first_server = "p9".into();
    assert!(m.validate().is_err());
}

#[test]
fn sync_model_maps_reference_time_to_host_time() {
    let model = SyncModel::from_fit(&ClockFit {
        reference_time: 500.0,
        offset_seconds: 0.1,
        skew_ppm: 3.0,
        residual_std_ms: 0.5,
        sample_count: 12,
    });
    assert!((model.reference_host_time - 500.0).abs() < 1e-9);
    assert!((model.skew_ppm - 3.0).abs() < 1e-9);
}

#[test]
fn sync_sample_record_derives_offset_and_delay() {
    let record = SyncSampleRecord::from_sample(&ClockSample::new(100.0, 100.51, 100.512, 100.022));
    assert!((record.offset_seconds - 0.5).abs() < 1e-9);
    assert!((record.delay_seconds - 0.020).abs() < 1e-9);
    assert!((record.host_time - 100.011).abs() < 1e-9);
}

#[test]
fn event_constructors_set_the_right_fields() {
    let spike = SessionEvent::motion_spike(10.0, 0.5);
    assert_eq!(spike.event_type, "motion-spike");
    assert_eq!(spike.magnitude, Some(0.5));

    let thermal = SessionEvent::thermal(11.0, "serious");
    assert_eq!(thermal.event_type, "thermal");
    assert_eq!(thermal.state.as_deref(), Some("serious"));

    let interrupted = SessionEvent::capture_interrupted(12.0, "system-pressure");
    assert_eq!(interrupted.event_type, "capture-interrupted");
    assert_eq!(interrupted.reason.as_deref(), Some("system-pressure"));

    let mark = SessionEvent::mark(13.0, "changeover");
    assert_eq!(mark.event_type, "mark");
    assert_eq!(mark.tag.as_deref(), Some("changeover"));
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test manifest`
Expected: FAIL — `unresolved import tracker_core::session::SessionManifest`

- [ ] **Step 3: Napisz implementację**

`src/session/manifest.rs`:

```rust
use crate::clock::{ClockFit, ClockSample, SyncGap};
use crate::device::CapabilityReport;
use crate::session::{CaptureProfile, MatchSetup, SessionRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ios,
    Android,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub model: String,
    pub os_version: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct WhiteBalanceGains {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct LockedCameraSettings {
    pub exposure_duration_seconds: f64,
    pub iso: f64,
    pub focus_lens_position: f64,
    pub white_balance_gains: WhiteBalanceGains,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub sample_rate_hz: u32,
    pub channels: u32,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfo {
    pub lens: String,
    pub profile: CaptureProfile,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: String,
    pub target_bitrate_mbps: u32,
    pub audio: AudioInfo,
    pub locked: LockedCameraSettings,
    /// Android zwykle nie udostepnia intrinsics; wtedy `None`, a P1
    /// odzyskuje ogniskowa z geometrii kortu.
    pub intrinsic_matrix: Option<Vec<Vec<f64>>>,
}

impl CameraInfo {
    /// Szerokosc, wysokosc i fps sa wyprowadzane z profilu, zeby manifest
    /// nie mogl sam sobie zaprzeczyc.
    pub fn new(
        lens: String,
        profile: CaptureProfile,
        codec: String,
        target_bitrate_mbps: u32,
        audio: AudioInfo,
        locked: LockedCameraSettings,
        intrinsic_matrix: Option<Vec<Vec<f64>>>,
    ) -> Self {
        Self {
            lens,
            profile,
            width: profile.width(),
            height: profile.height(),
            fps: profile.fps(),
            codec,
            target_bitrate_mbps,
            audio,
            locked,
            intrinsic_matrix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncModel {
    pub reference_host_time: f64,
    pub offset_seconds: f64,
    pub skew_ppm: f64,
    pub residual_std_ms: f64,
}

impl SyncModel {
    pub fn from_fit(fit: &ClockFit) -> Self {
        Self {
            reference_host_time: fit.reference_time,
            offset_seconds: fit.offset_seconds,
            skew_ppm: fit.skew_ppm,
            residual_std_ms: fit.residual_std_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SyncSampleRecord {
    pub host_time: f64,
    pub offset_seconds: f64,
    pub delay_seconds: f64,
}

impl SyncSampleRecord {
    pub fn from_sample(sample: &ClockSample) -> Self {
        Self {
            host_time: sample.local_time(),
            offset_seconds: sample.offset(),
            delay_seconds: sample.delay(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct TimingInfo {
    pub clock_domain: String,
    pub first_frame_host_time: f64,
    /// Ktora sciezka transportu byla uzyta: "multipeer", "sockets", "ble".
    pub transport: String,
    pub sync_model: SyncModel,
    pub sync_samples: Vec<SyncSampleRecord>,
    pub sync_gaps: Vec<SyncGap>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInfo {
    pub file: String,
    pub start_host_time: f64,
    pub end_host_time: f64,
    pub frame_count: u64,
}

/// Zdarzenie sesji. Pola opcjonalne odpowiadaja roznym typom zdarzen:
/// `magnitude` dla `motion-spike`, `reason` dla `capture-interrupted`,
/// `state` dla `thermal`, `tag` dla `mark`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub host_time: f64,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl SessionEvent {
    fn bare(host_time: f64, event_type: &str) -> Self {
        Self {
            host_time,
            event_type: event_type.into(),
            magnitude: None,
            reason: None,
            state: None,
            tag: None,
        }
    }

    pub fn motion_spike(host_time: f64, magnitude: f64) -> Self {
        Self { magnitude: Some(magnitude), ..Self::bare(host_time, "motion-spike") }
    }

    pub fn thermal(host_time: f64, state: &str) -> Self {
        Self { state: Some(state.into()), ..Self::bare(host_time, "thermal") }
    }

    pub fn capture_interrupted(host_time: f64, reason: &str) -> Self {
        Self { reason: Some(reason.into()), ..Self::bare(host_time, "capture-interrupted") }
    }

    pub fn capture_resumed(host_time: f64) -> Self {
        Self::bare(host_time, "capture-resumed")
    }

    pub fn mark(host_time: f64, tag: &str) -> Self {
        Self { tag: Some(tag.into()), ..Self::bare(host_time, "mark") }
    }

    pub fn session_stopped(host_time: f64, reason: &str) -> Self {
        Self { reason: Some(reason.into()), ..Self::bare(host_time, "session-stopped") }
    }

    pub fn link_lost(host_time: f64) -> Self {
        Self::bare(host_time, "link-lost")
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ManifestError {
    #[error("unsupported schema version: {version}")]
    UnsupportedSchemaVersion { version: u32 },
    #[error("no segments")]
    NoSegments,
    #[error("segments not contiguous before index {index}")]
    SegmentsNotContiguous { index: u32 },
    #[error("first frame outside first segment")]
    FirstFrameOutsideFirstSegment,
    #[error("intrinsic matrix is not 3x3")]
    MalformedIntrinsicMatrix,
    #[error("sync model contains a non-finite value")]
    NonFiniteSyncModel,
    #[error("invalid match setup: {reason}")]
    InvalidMatchSetup { reason: String },
    #[error("serialization error: {reason}")]
    Serialization { reason: String },
}

/// Sidecar opisujacy sesje nagraniowa. To wlasciwy produkt P0a —
/// wideo bez tego pliku jest w P1 bezuzyteczne.
///
/// Wszystkie znaczniki czasu sa w sekundach, w monotonicznej domenie
/// zegara lokalnego urzadzenia (`timing.clock_domain`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct SessionManifest {
    pub schema_version: u32,
    pub session_id: String,
    pub role: SessionRole,
    pub platform: Platform,
    pub peer_device_model: Option<String>,
    #[serde(rename = "match")]
    pub match_setup: MatchSetup,
    pub device: DeviceInfo,
    pub capabilities: CapabilityReport,
    pub camera: CameraInfo,
    pub timing: TimingInfo,
    pub segments: Vec<SegmentInfo>,
    pub events: Vec<SessionEvent>,
    pub motion_log: String,
}

impl SessionManifest {
    pub fn encode(&self) -> Result<String, ManifestError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| ManifestError::Serialization { reason: error.to_string() })
    }

    pub fn decode(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json)
            .map_err(|error| ManifestError::Serialization { reason: error.to_string() })
    }

    /// Dopuszczalna przerwa miedzy segmentami: jedna klatka.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != 2 {
            return Err(ManifestError::UnsupportedSchemaVersion { version: self.schema_version });
        }
        let first = self.segments.first().ok_or(ManifestError::NoSegments)?;

        let tolerance = 1.0 / self.camera.fps as f64;
        for index in 1..self.segments.len() {
            let previous = &self.segments[index - 1];
            let current = &self.segments[index];
            if current.start_host_time - previous.end_host_time > tolerance {
                return Err(ManifestError::SegmentsNotContiguous { index: index as u32 });
            }
        }

        let first_frame = self.timing.first_frame_host_time;
        if first_frame < first.start_host_time - tolerance || first_frame > first.end_host_time {
            return Err(ManifestError::FirstFrameOutsideFirstSegment);
        }

        if let Some(matrix) = &self.camera.intrinsic_matrix {
            if matrix.len() != 3 || matrix.iter().any(|row| row.len() != 3) {
                return Err(ManifestError::MalformedIntrinsicMatrix);
            }
        }

        let model = &self.timing.sync_model;
        if !model.offset_seconds.is_finite()
            || !model.skew_ppm.is_finite()
            || !model.residual_std_ms.is_finite()
        {
            return Err(ManifestError::NonFiniteSyncModel);
        }

        self.match_setup
            .validate()
            .map_err(|error| ManifestError::InvalidMatchSetup { reason: error.to_string() })?;

        Ok(())
    }
}
```

W `src/session/mod.rs` dodaj `mod manifest;` oraz reeksport:

```rust
pub use manifest::{
    AudioInfo, CameraInfo, DeviceInfo, LockedCameraSettings, ManifestError, Platform, SegmentInfo,
    SessionEvent, SessionManifest, SyncModel, SyncSampleRecord, TimingInfo, WhiteBalanceGains,
};
```

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test manifest`
Expected: PASS, 18 testów

- [ ] **Step 5: Commit**

```bash
git add src/session tests/manifest.rs
git commit -m "feat(session): manifest sesji z walidacja"
```

---

### Task 10: Detekcja szarpnięć kamery

**Files:**
- Create: `src/session/motion.rs`
- Modify: `src/session/mod.rs`
- Test: `tests/motion.rs`

**Interfaces:**
- Consumes: `SessionEvent` z zadania 9
- Produces: `MotionVector { x, y, z }` z `norm()`, `MotionSample { host_time, rotation_rate }`, `MotionAnalyzer` z `new()`, `process(&mut self, &MotionSample) -> Option<SessionEvent>`, polami `rotation_threshold: f64`, `refractory_seconds: f64`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/motion.rs`:

```rust
use tracker_core::session::{MotionAnalyzer, MotionSample, MotionVector};

fn calm(host_time: f64) -> MotionSample {
    MotionSample {
        host_time,
        rotation_rate: MotionVector { x: 0.01, y: 0.005, z: 0.002 },
    }
}

fn spike(host_time: f64, magnitude: f64) -> MotionSample {
    MotionSample {
        host_time,
        rotation_rate: MotionVector { x: magnitude, y: 0.0, z: 0.0 },
    }
}

#[test]
fn calm_motion_produces_no_event() {
    let mut analyzer = MotionAnalyzer::new();
    for step in 0..100 {
        assert!(analyzer.process(&calm(step as f64 * 0.01)).is_none());
    }
}

#[test]
fn spike_produces_event_with_magnitude() {
    let mut analyzer = MotionAnalyzer::new();
    let event = analyzer.process(&spike(5.0, 1.2)).expect("oczekiwano zdarzenia");

    assert_eq!(event.event_type, "motion-spike");
    assert!((event.host_time - 5.0).abs() < 1e-9);
    assert!((event.magnitude.unwrap() - 1.2).abs() < 1e-6);
}

#[test]
fn refractory_period_suppresses_repeated_spikes() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.refractory_seconds = 1.0;

    assert!(analyzer.process(&spike(5.0, 1.2)).is_some());
    assert!(analyzer.process(&spike(5.2, 1.2)).is_none());
    assert!(analyzer.process(&spike(5.9, 1.2)).is_none());
    assert!(analyzer.process(&spike(6.1, 1.2)).is_some());
}

#[test]
fn threshold_is_configurable() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.rotation_threshold = 2.0;

    assert!(analyzer.process(&spike(1.0, 1.5)).is_none());
    assert!(analyzer.process(&spike(2.0, 2.5)).is_some());
}

#[test]
fn magnitude_uses_vector_norm() {
    let mut analyzer = MotionAnalyzer::new();
    analyzer.rotation_threshold = 0.35;

    let event = analyzer
        .process(&MotionSample {
            host_time: 1.0,
            rotation_rate: MotionVector { x: 0.3, y: 0.4, z: 0.0 },
        })
        .expect("oczekiwano zdarzenia");

    // norma (0.3, 0.4, 0) = 0.5
    assert!((event.magnitude.unwrap() - 0.5).abs() < 1e-9);
}

#[test]
fn vector_norm_is_euclidean() {
    let v = MotionVector { x: 3.0, y: 4.0, z: 12.0 };
    assert!((v.norm() - 13.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test motion`
Expected: FAIL — `unresolved import tracker_core::session::MotionAnalyzer`

- [ ] **Step 3: Napisz implementację**

`src/session/motion.rs`:

```rust
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
```

W `src/session/mod.rs` dodaj `mod motion;` oraz `pub use motion::{MotionAnalyzer, MotionSample, MotionVector};`.

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test motion`
Expected: PASS, 6 testów

- [ ] **Step 5: Commit**

```bash
git add src/session tests/motion.rs
git commit -m "feat(session): detekcja szarpniec kamery z okresem refrakcji"
```

---

### Task 11: Porty sprzętowe i maszyna stanów sesji

Serce obsługi awarii z sekcji 11 specyfikacji.

**Files:**
- Create: `src/session/ports.rs`, `src/session/recorder.rs`
- Modify: `src/session/mod.rs`, `tests/support/mod.rs`
- Test: `tests/recorder.rs`

**Interfaces:**
- Consumes: wszystko z zadań 4, 7, 8, 9
- Produces: `ThermalLevel` (`Nominal`, `Fair`, `Serious`, `Critical`, z `as_str()`), traity `CaptureControlling`, `StorageProbing`, `ThermalProbing`; `RecorderConfig`, `RecorderState` (`Idle`, `Armed`, `Recording`, `Finalizing`, `Stopped { reason }`), `StopReason` (`UserRequested`, `StorageExhausted`, `ThermalCritical`, `PeerSilence`, z `as_str()`), `RecorderError`; `SessionRecorder` z `new(...)`, `arm()`, `start(session_id, profile, now)`, `tick(now)`, `note_event(event)`, `note_peer_heartbeat(at)`, `stop(now, snapshot) -> Result<SessionManifest, RecorderError>`, `build_manifest(now, snapshot)`, `state()`

Uproszczenie względem szkicu w specyfikacji: **nie ma traitu dostarczającego snapshot synchronizacji.** Snapshot jest parametrem `stop()` i `build_manifest()`. Jeden trait mniej, a testy podają go wprost.

- [ ] **Step 1: Napisz porty**

`src/session/ports.rs`:

```rust
use super::{CaptureProfile, LockedCameraSettings, SegmentInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, uniffi::Enum)]
pub enum ThermalLevel {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            ThermalLevel::Nominal => "nominal",
            ThermalLevel::Fair => "fair",
            ThermalLevel::Serious => "serious",
            ThermalLevel::Critical => "critical",
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CaptureError {
    #[error("camera unavailable")]
    NoCamera,
    #[error("profile unavailable: {profile}")]
    ProfileUnavailable { profile: String },
    #[error("not recording")]
    NotRecording,
    #[error("camera error: {reason}")]
    Other { reason: String },
}

/// Sterowanie kamera. Implementacje produkcyjne opakowuja AVFoundation
/// albo Camera2.
///
/// Ksztalt tego traitu jest podyktowany **ograniczeniami Camera2**, nie
/// mozliwosciami AVFoundation: `intrinsic_matrix` jest opcjonalne, bo
/// Android zwykle go nie udostepnia, a `lock_settings` moze zwrocic
/// wartosci pochodzace z samej blokady AE, gdy manualna kontrola sensora
/// jest niedostepna.
pub trait CaptureControlling: Send + Sync {
    fn lock_settings(&self) -> Result<LockedCameraSettings, CaptureError>;
    fn start_recording(&self, session_id: &str, profile: CaptureProfile) -> Result<(), CaptureError>;
    /// Zamyka biezacy segment i otwiera nastepny. Zwraca zamkniety segment.
    fn roll_segment(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    /// Zamyka biezacy segment i konczy nagrywanie.
    fn stop_recording(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    fn first_frame_host_time(&self) -> Option<f64>;
    fn intrinsic_matrix(&self) -> Option<Vec<Vec<f64>>>;
    fn lens_name(&self) -> String;
}

pub trait StorageProbing: Send + Sync {
    fn free_bytes(&self) -> i64;
}

pub trait ThermalProbing: Send + Sync {
    fn thermal_level(&self) -> ThermalLevel;
}
```

- [ ] **Step 2: Napisz atrapy portów**

Na końcu `tests/support/mod.rs`:

```rust
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

    pub fn did_lock(&self) -> bool { self.state.lock().unwrap().did_lock }
    pub fn did_start(&self) -> bool { self.state.lock().unwrap().did_start }
    pub fn did_stop(&self) -> bool { self.state.lock().unwrap().did_stop }
    pub fn roll_count(&self) -> usize { self.state.lock().unwrap().roll_count }
    pub fn set_lock_should_fail(&self, fail: bool) { *self.lock_should_fail.lock().unwrap() = fail; }

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
    fn default() -> Self { Self::new() }
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
            white_balance_gains: WhiteBalanceGains { r: 1.9, g: 1.0, b: 1.6 },
        })
    }

    fn start_recording(&self, _session_id: &str, _profile: CaptureProfile) -> Result<(), CaptureError> {
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

    fn first_frame_host_time(&self) -> Option<f64> { Some(1000.05) }

    fn intrinsic_matrix(&self) -> Option<Vec<Vec<f64>>> {
        Some(vec![
            vec![1580.0, 0.0, 960.0],
            vec![0.0, 1580.0, 540.0],
            vec![0.0, 0.0, 1.0],
        ])
    }

    fn lens_name(&self) -> String { "builtInWideAngleCamera".into() }
}

pub struct FakeStorage {
    free: Mutex<i64>,
}

impl FakeStorage {
    pub fn new() -> Self { Self { free: Mutex::new(64_000_000_000) } }
    pub fn set_free_bytes(&self, bytes: i64) { *self.free.lock().unwrap() = bytes; }
}

impl Default for FakeStorage {
    fn default() -> Self { Self::new() }
}

impl StorageProbing for FakeStorage {
    fn free_bytes(&self) -> i64 { *self.free.lock().unwrap() }
}

pub struct FakeThermal {
    level: Mutex<ThermalLevel>,
}

impl FakeThermal {
    pub fn new() -> Self { Self { level: Mutex::new(ThermalLevel::Nominal) } }
    pub fn set_level(&self, level: ThermalLevel) { *self.level.lock().unwrap() = level; }
}

impl Default for FakeThermal {
    fn default() -> Self { Self::new() }
}

impl ThermalProbing for FakeThermal {
    fn thermal_level(&self) -> ThermalLevel { *self.level.lock().unwrap() }
}
```

- [ ] **Step 3: Napisz testy, które mają nie przejść**

`tests/recorder.rs`:

```rust
mod support;

use std::sync::Arc;
use support::{FakeCapture, FakeStorage, FakeThermal};
use tracker_core::clock::{ClockFit, SyncGap, SyncSnapshot};
use tracker_core::device::{CapabilityProbeResult, TimestampSource};
use tracker_core::session::{
    CaptureProfile, DeviceInfo, MatchSetup, Platform, RecorderConfig, RecorderState, SessionEvent,
    SessionRecorder, SessionRole, StopReason,
};

struct Harness {
    capture: Arc<FakeCapture>,
    storage: Arc<FakeStorage>,
    thermal: Arc<FakeThermal>,
    recorder: SessionRecorder,
}

fn snapshot() -> SyncSnapshot {
    SyncSnapshot {
        model: ClockFit {
            reference_time: 1000.0,
            offset_seconds: 0.014,
            skew_ppm: 7.3,
            residual_std_ms: 0.9,
            sample_count: 60,
        },
        samples: vec![],
        gaps: vec![],
    }
}

fn harness(role: SessionRole) -> Harness {
    let capture = Arc::new(FakeCapture::new());
    let storage = Arc::new(FakeStorage::new());
    let thermal = Arc::new(FakeThermal::new());
    let capabilities = CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    }
    .evaluate();

    let recorder = SessionRecorder::new(
        role,
        Platform::Ios,
        DeviceInfo {
            model: "iPhone17,2".into(),
            os_version: "26.0".into(),
            app_version: "0.1.0".into(),
        },
        Some("iPhone15,4".into()),
        MatchSetup::default_singles(),
        capabilities,
        "multipeer".into(),
        capture.clone(),
        storage.clone(),
        thermal.clone(),
        RecorderConfig::default(),
    );

    Harness { capture, storage, thermal, recorder }
}

fn started(role: SessionRole, now: f64) -> Harness {
    let mut h = harness(role);
    h.recorder.arm().unwrap();
    h.recorder.start("s-1", CaptureProfile::P1080p120, now).unwrap();
    h
}

// --- przejscia stanow ---

#[test]
fn starts_idle() {
    assert_eq!(harness(SessionRole::Master).recorder.state(), RecorderState::Idle);
}

#[test]
fn arm_locks_camera_settings() {
    let mut h = harness(SessionRole::Master);
    h.recorder.arm().unwrap();
    assert_eq!(h.recorder.state(), RecorderState::Armed);
    assert!(h.capture.did_lock());
}

#[test]
fn start_requires_armed_state() {
    let mut h = harness(SessionRole::Master);
    assert!(h.recorder.start("s-1", CaptureProfile::P1080p120, 1000.0).is_err());
}

#[test]
fn lock_failure_leaves_recorder_idle() {
    let mut h = harness(SessionRole::Master);
    h.capture.set_lock_should_fail(true);
    assert!(h.recorder.arm().is_err());
    assert_eq!(h.recorder.state(), RecorderState::Idle);
}

#[test]
fn start_moves_to_recording() {
    let h = started(SessionRole::Master, 1000.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
    assert!(h.capture.did_start());
}

// --- segmentacja ---

#[test]
fn segment_rolls_after_configured_duration() {
    let mut h = started(SessionRole::Master, 1000.0);

    h.recorder.tick(1200.0);
    assert_eq!(h.capture.roll_count(), 0);

    h.recorder.tick(1300.1);
    assert_eq!(h.capture.roll_count(), 1);

    h.recorder.tick(1600.2);
    assert_eq!(h.capture.roll_count(), 2);
}

// --- awarie ---

#[test]
fn critical_thermal_stops_session() {
    let mut h = started(SessionRole::Master, 1000.0);

    h.thermal.set_level(tracker_core::session::ThermalLevel::Serious);
    h.recorder.tick(1010.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);

    h.thermal.set_level(tracker_core::session::ThermalLevel::Critical);
    h.recorder.tick(1020.0);
    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::ThermalCritical });
    assert!(h.capture.did_stop());
}

#[test]
fn serious_thermal_is_recorded_as_event() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.thermal.set_level(tracker_core::session::ThermalLevel::Serious);
    h.recorder.tick(1010.0);

    let manifest = h.recorder.build_manifest(1010.0, &snapshot()).unwrap();
    assert!(manifest
        .events
        .iter()
        .any(|e| e.event_type == "thermal" && e.state.as_deref() == Some("serious")));
}

#[test]
fn low_storage_stops_session() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.storage.set_free_bytes(100_000_000);
    h.recorder.tick(1010.0);

    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::StorageExhausted });
}

#[test]
fn arm_rejects_insufficient_storage() {
    let mut h = harness(SessionRole::Master);
    h.storage.set_free_bytes(10_000_000);
    assert!(h.recorder.arm().is_err());
}

#[test]
fn slave_stops_after_peer_silence_timeout() {
    let mut h = started(SessionRole::Slave, 1000.0);

    h.recorder.note_peer_heartbeat(1050.0);
    h.recorder.tick(1150.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);

    h.recorder.tick(1180.0);
    assert_eq!(h.recorder.state(), RecorderState::Stopped { reason: StopReason::PeerSilence });
}

#[test]
fn master_ignores_peer_silence() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_peer_heartbeat(1000.0);
    h.recorder.tick(2000.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
}

#[test]
fn link_loss_does_not_stop_recording() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_event(SessionEvent::link_lost(1100.0));
    h.recorder.tick(1110.0);
    assert_eq!(h.recorder.state(), RecorderState::Recording);
}

// --- manifest ---

#[test]
fn stop_produces_valid_manifest() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.tick(1300.1);
    let manifest = h.recorder.stop(1450.0, &snapshot()).unwrap();

    assert!(manifest.validate().is_ok(), "manifest niepoprawny: {:?}", manifest.validate());
    assert_eq!(manifest.session_id, "s-1");
    assert_eq!(manifest.role, SessionRole::Master);
    assert_eq!(manifest.platform, Platform::Ios);
    assert_eq!(manifest.camera.profile, CaptureProfile::P1080p120);
    assert_eq!(manifest.camera.fps, 120);
    assert_eq!(manifest.segments.len(), 2);
    assert_eq!(manifest.timing.clock_domain, "mach_absolute_time");
    assert_eq!(manifest.timing.transport, "multipeer");
    assert_eq!(manifest.motion_log, "motion.jsonl");
    assert_eq!(manifest.schema_version, 2);
}

#[test]
fn manifest_carries_sync_model_and_gaps() {
    let mut h = started(SessionRole::Master, 1000.0);
    let mut snap = snapshot();
    snap.gaps.push(SyncGap {
        start_host_time: 1100.0,
        end_host_time: Some(1160.0),
        reason: "link-lost".into(),
    });

    let manifest = h.recorder.stop(1200.0, &snap).unwrap();
    assert!((manifest.timing.sync_model.skew_ppm - 7.3).abs() < 1e-9);
    assert_eq!(manifest.timing.sync_gaps.len(), 1);
    assert_eq!(manifest.timing.sync_gaps[0].reason, "link-lost");
}

#[test]
fn stop_is_idempotent_after_automatic_stop() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.thermal.set_level(tracker_core::session::ThermalLevel::Critical);
    h.recorder.tick(1010.0);

    let manifest = h.recorder.stop(1020.0, &snapshot()).unwrap();
    assert!(manifest.validate().is_ok());
    assert_eq!(h.capture.roll_count(), 0);
}

#[test]
fn marks_from_the_watch_reach_the_manifest() {
    let mut h = started(SessionRole::Master, 1000.0);
    h.recorder.note_event(SessionEvent::mark(1100.0, "interesting"));
    h.recorder.note_event(SessionEvent::mark(1200.0, "changeover"));

    let manifest = h.recorder.stop(1300.0, &snapshot()).unwrap();
    let tags: Vec<&str> = manifest
        .events
        .iter()
        .filter(|e| e.event_type == "mark")
        .filter_map(|e| e.tag.as_deref())
        .collect();
    assert_eq!(tags, vec!["interesting", "changeover"]);
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test recorder`
Expected: FAIL — `unresolved import tracker_core::session::SessionRecorder`

- [ ] **Step 5: Napisz implementację**

`src/session/recorder.rs`:

```rust
use std::sync::Arc;

use crate::clock::SyncSnapshot;
use crate::device::CapabilityReport;
use crate::session::{
    AudioInfo, CameraInfo, CaptureControlling, CaptureProfile, DeviceInfo, LockedCameraSettings,
    ManifestError, MatchSetup, Platform, SegmentInfo, SessionEvent, SessionManifest, SessionRole,
    StorageProbing, SyncModel, SyncSampleRecord, ThermalLevel, ThermalProbing, TimingInfo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StopReason {
    UserRequested,
    StorageExhausted,
    ThermalCritical,
    PeerSilence,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::UserRequested => "user-requested",
            StopReason::StorageExhausted => "storage-exhausted",
            StopReason::ThermalCritical => "thermal-critical",
            StopReason::PeerSilence => "peer-silence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RecorderState {
    Idle,
    Armed,
    Recording,
    Finalizing,
    Stopped { reason: StopReason },
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct RecorderConfig {
    pub segment_seconds: f64,
    /// Ponizej tej rezerwy konczymy sesje w kontrolowany sposob.
    pub minimum_free_bytes: i64,
    /// Cisza peera dluzsza niz to konczy sesje po stronie slave'a.
    pub peer_silence_timeout: f64,
    pub target_bitrate_mbps: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            segment_seconds: 300.0,
            minimum_free_bytes: 500_000_000,
            peer_silence_timeout: 120.0,
            target_bitrate_mbps: 40,
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum RecorderError {
    #[error("wrong state")]
    WrongState,
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("session not started")]
    NotStarted,
    #[error("camera error: {reason}")]
    Capture { reason: String },
    #[error("manifest error: {reason}")]
    Manifest { reason: String },
}

/// Prowadzi sesje nagraniowa i buduje manifest.
///
/// Nie odczytuje zegara ani nie uruchamia timerow — czas przychodzi
/// z zewnatrz przez `tick`. Cala obsluga awarii ze specyfikacji sprowadza
/// sie dzieki temu do testow jednostkowych.
///
/// Zasada nadrzedna: **utrata lacznosci nigdy nie zatrzymuje nagrywania.**
/// Zdarzenie `link-lost` trafia do manifestu, ale nie zmienia stanu.
pub struct SessionRecorder {
    role: SessionRole,
    platform: Platform,
    device: DeviceInfo,
    peer_device_model: Option<String>,
    match_setup: MatchSetup,
    capabilities: CapabilityReport,
    transport: String,
    capture: Arc<dyn CaptureControlling>,
    storage: Arc<dyn StorageProbing>,
    thermal: Arc<dyn ThermalProbing>,
    config: RecorderConfig,

    state: RecorderState,
    session_id: Option<String>,
    profile: Option<CaptureProfile>,
    locked: Option<LockedCameraSettings>,
    segments: Vec<SegmentInfo>,
    events: Vec<SessionEvent>,
    segment_started: f64,
    last_peer_heartbeat: Option<f64>,
    last_thermal: ThermalLevel,
}

impl SessionRecorder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: SessionRole,
        platform: Platform,
        device: DeviceInfo,
        peer_device_model: Option<String>,
        match_setup: MatchSetup,
        capabilities: CapabilityReport,
        transport: String,
        capture: Arc<dyn CaptureControlling>,
        storage: Arc<dyn StorageProbing>,
        thermal: Arc<dyn ThermalProbing>,
        config: RecorderConfig,
    ) -> Self {
        Self {
            role,
            platform,
            device,
            peer_device_model,
            match_setup,
            capabilities,
            transport,
            capture,
            storage,
            thermal,
            config,
            state: RecorderState::Idle,
            session_id: None,
            profile: None,
            locked: None,
            segments: Vec::new(),
            events: Vec::new(),
            segment_started: 0.0,
            last_peer_heartbeat: None,
            last_thermal: ThermalLevel::Nominal,
        }
    }

    pub fn state(&self) -> RecorderState {
        self.state
    }

    /// Sprawdza zapas miejsca i blokuje parametry kamery.
    pub fn arm(&mut self) -> Result<(), RecorderError> {
        if self.state != RecorderState::Idle {
            return Err(RecorderError::WrongState);
        }
        if self.storage.free_bytes() < self.config.minimum_free_bytes {
            return Err(RecorderError::InsufficientStorage);
        }
        let locked = self
            .capture
            .lock_settings()
            .map_err(|error| RecorderError::Capture { reason: error.to_string() })?;
        self.locked = Some(locked);
        self.state = RecorderState::Armed;
        Ok(())
    }

    pub fn start(
        &mut self,
        session_id: &str,
        profile: CaptureProfile,
        now: f64,
    ) -> Result<(), RecorderError> {
        if self.state != RecorderState::Armed {
            return Err(RecorderError::WrongState);
        }
        self.capture
            .start_recording(session_id, profile)
            .map_err(|error| RecorderError::Capture { reason: error.to_string() })?;
        self.session_id = Some(session_id.to_string());
        self.profile = Some(profile);
        self.segment_started = now;
        self.state = RecorderState::Recording;
        Ok(())
    }

    /// Wolane co sekunde. Oddaje sterowanie czasem na zewnatrz.
    pub fn tick(&mut self, now: f64) {
        if self.state != RecorderState::Recording {
            return;
        }

        let level = self.thermal.thermal_level();
        if level != self.last_thermal {
            self.last_thermal = level;
            self.events.push(SessionEvent::thermal(now, level.as_str()));
        }
        if level == ThermalLevel::Critical {
            self.finish(now, StopReason::ThermalCritical);
            return;
        }

        if self.storage.free_bytes() < self.config.minimum_free_bytes {
            self.finish(now, StopReason::StorageExhausted);
            return;
        }

        if self.role == SessionRole::Slave {
            if let Some(last) = self.last_peer_heartbeat {
                if now - last > self.config.peer_silence_timeout {
                    self.finish(now, StopReason::PeerSilence);
                    return;
                }
            }
        }

        if now - self.segment_started >= self.config.segment_seconds {
            if let Ok(segment) = self.capture.roll_segment(now) {
                self.segments.push(segment);
                self.segment_started = now;
            }
        }
    }

    pub fn note_event(&mut self, event: SessionEvent) {
        self.events.push(event);
    }

    pub fn note_peer_heartbeat(&mut self, at: f64) {
        self.last_peer_heartbeat = Some(at);
    }

    /// Zatrzymanie na zadanie uzytkownika. Jesli sesja zostala juz
    /// zakonczona automatycznie, zwraca manifest bez ponownego dotykania kamery.
    pub fn stop(
        &mut self,
        now: f64,
        snapshot: &SyncSnapshot,
    ) -> Result<SessionManifest, RecorderError> {
        match self.state {
            RecorderState::Recording => self.finish(now, StopReason::UserRequested),
            RecorderState::Stopped { .. } => {}
            _ => return Err(RecorderError::WrongState),
        }
        let manifest = self.build_manifest(now, snapshot)?;

        // Walidacja tylko tutaj: `build_manifest` bywa wolane w trakcie
        // nagrywania, gdy lista segmentow jest jeszcze pusta. Manifest,
        // ktory sam siebie nie sprawdza przy zamknieciu sesji, jest gorszy
        // niz jego brak — w P1 wyszloby to dopiero po meczu.
        manifest
            .validate()
            .map_err(|error: ManifestError| RecorderError::Manifest { reason: error.to_string() })?;

        Ok(manifest)
    }

    fn finish(&mut self, now: f64, reason: StopReason) {
        self.state = RecorderState::Finalizing;
        if let Ok(segment) = self.capture.stop_recording(now) {
            self.segments.push(segment);
        }
        self.events.push(SessionEvent::session_stopped(now, reason.as_str()));
        self.state = RecorderState::Stopped { reason };
    }

    pub fn build_manifest(
        &self,
        now: f64,
        snapshot: &SyncSnapshot,
    ) -> Result<SessionManifest, RecorderError> {
        let session_id = self.session_id.clone().ok_or(RecorderError::NotStarted)?;
        let profile = self.profile.ok_or(RecorderError::NotStarted)?;
        let locked = self.locked.ok_or(RecorderError::NotStarted)?;

        let first_frame = self
            .capture
            .first_frame_host_time()
            .or_else(|| self.segments.first().map(|s| s.start_host_time))
            .unwrap_or(now);

        let manifest = SessionManifest {
            schema_version: 2,
            session_id,
            role: self.role,
            platform: self.platform,
            peer_device_model: self.peer_device_model.clone(),
            match_setup: self.match_setup.clone(),
            device: self.device.clone(),
            capabilities: self.capabilities.clone(),
            camera: CameraInfo::new(
                self.capture.lens_name(),
                profile,
                "hevc".into(),
                self.config.target_bitrate_mbps,
                AudioInfo { sample_rate_hz: 48000, channels: 1, codec: "pcm".into() },
                locked,
                self.capture.intrinsic_matrix(),
            ),
            timing: TimingInfo {
                clock_domain: "mach_absolute_time".into(),
                first_frame_host_time: first_frame,
                transport: self.transport.clone(),
                sync_model: SyncModel::from_fit(&snapshot.model),
                sync_samples: snapshot.samples.iter().map(SyncSampleRecord::from_sample).collect(),
                sync_gaps: snapshot.gaps.clone(),
            },
            segments: self.segments.clone(),
            events: self.events.clone(),
            motion_log: "motion.jsonl".into(),
        };

        Ok(manifest)
    }
}
```

W `src/session/mod.rs` dodaj `mod ports;`, `mod recorder;` oraz:

```rust
pub use ports::{
    CaptureControlling, CaptureError, StorageProbing, ThermalLevel, ThermalProbing,
};
pub use recorder::{RecorderConfig, RecorderError, RecorderState, SessionRecorder, StopReason};
```

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test recorder`
Expected: PASS, 17 testów

- [ ] **Step 7: Uruchom cały pakiet**

Run: `cargo test`
Expected: PASS, wszystkie testy z zadań 1-11

- [ ] **Step 8: Commit**

```bash
git add src/session tests
git commit -m "feat(session): porty sprzetowe i maszyna stanow sesji"
```

---

### Task 12: Zestaw komend pilota

**Files:**
- Create: `src/remote/mod.rs`, `src/remote/command.rs`
- Modify: `src/lib.rs`
- Test: `tests/remote.rs`

**Interfaces:**
- Consumes: `SessionEvent` (zadanie 9), `ClockFit` (zadanie 2)
- Produces: `MarkTag` (`Interesting`, `Changeover`, `StrayBall`, `BadRally`, z `as_str()`), `RemoteCommand` (`StartRecording`, `StopRecording`, `MarkMoment { tag }`), `RemoteFeedback` (`RecordingState { recording, elapsed_seconds }`, `SyncQuality { residual_ms }`, `Storage { free_gigabytes }`, `Warning { text }`), `command_to_event(&RemoteCommand, host_time) -> Option<SessionEvent>`

Zestaw jest celowo minimalny — P0a nie liczy punktów. Komendy punktacyjne dochodzą w P2, ale enum i kanał zwrotny istnieją od teraz, żeby Garmin, przycisk BLE czy cokolwiek innego wpinało się później bez ruszania logiki.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/remote.rs`:

```rust
use tracker_core::remote::{command_to_event, MarkTag, RemoteCommand, RemoteFeedback};

#[test]
fn mark_command_becomes_a_manifest_event() {
    let event = command_to_event(&RemoteCommand::MarkMoment { tag: MarkTag::Changeover }, 1234.5)
        .expect("oczekiwano zdarzenia");

    assert_eq!(event.event_type, "mark");
    assert_eq!(event.tag.as_deref(), Some("changeover"));
    assert!((event.host_time - 1234.5).abs() < 1e-9);
}

#[test]
fn transport_commands_produce_no_event() {
    assert!(command_to_event(&RemoteCommand::StartRecording, 1.0).is_none());
    assert!(command_to_event(&RemoteCommand::StopRecording, 1.0).is_none());
}

#[test]
fn mark_tags_match_manifest_vocabulary() {
    assert_eq!(MarkTag::Interesting.as_str(), "interesting");
    assert_eq!(MarkTag::Changeover.as_str(), "changeover");
    assert_eq!(MarkTag::StrayBall.as_str(), "stray-ball");
    assert_eq!(MarkTag::BadRally.as_str(), "bad-rally");
}

#[test]
fn feedback_variants_carry_their_payload() {
    match (RemoteFeedback::SyncQuality { residual_ms: 1.2 }) {
        RemoteFeedback::SyncQuality { residual_ms } => assert!((residual_ms - 1.2).abs() < 1e-9),
        other => panic!("nieoczekiwany wariant: {other:?}"),
    }
    match (RemoteFeedback::RecordingState { recording: true, elapsed_seconds: 42.0 }) {
        RemoteFeedback::RecordingState { recording, elapsed_seconds } => {
            assert!(recording);
            assert!((elapsed_seconds - 42.0).abs() < 1e-9);
        }
        other => panic!("nieoczekiwany wariant: {other:?}"),
    }
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test remote`
Expected: FAIL — `unresolved import tracker_core::remote`

- [ ] **Step 3: Napisz implementację**

`src/remote/command.rs`:

```rust
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
```

`src/remote/mod.rs`:

```rust
mod command;

pub use command::{command_to_event, MarkTag, RemoteCommand, RemoteFeedback};
```

W `src/lib.rs` dodaj `pub mod remote;`.

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test remote`
Expected: PASS, 4 testy

- [ ] **Step 5: Commit**

```bash
git add src/remote src/lib.rs tests/remote.rs
git commit -m "feat(remote): zestaw komend pilota i kanal zwrotny"
```

---

### Task 13: Eksport UniFFI i generacja bindingów

Ostatnie zadanie planu. Do tej pory typy danych miały derive'y UniFFI, ale nic nie zostało wyeksportowane jako wywoływalne API. Tutaj powstaje fasada, którą zobaczy Swift.

**Files:**
- Create: `src/api.rs`
- Modify: `src/lib.rs`, `Cargo.toml`
- Create: `scripts/build-bindings.sh`
- Test: `tests/api.rs`

**Interfaces:**
- Consumes: wszystko z zadań 1-12
- Produces: obiekty UniFFI `SyncEngine` i `Recorder`; funkcje `capability_report(probe) -> CapabilityReport`, `estimate_base_offset(frame_times, delivery_times) -> Option<BaseOffsetEstimate>`, `default_match_setup() -> MatchSetup`, `decode_manifest(json) -> Result<SessionManifest>`

Powód istnienia fasady: `SyncSession`, `PeerLink` i `SessionRecorder` mają mutowalny stan i wymagają `&mut self`, czego UniFFI nie eksportuje. Fasada owija je w `Mutex` i wystawia metody na `&self`. Trzymanie tej warstwy osobno oznacza, że logika pozostaje czysta, a cały brud FFI mieszka w jednym pliku.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

`tests/api.rs`:

```rust
mod support;

use std::sync::Arc;
use support::FakeTransport;
use tracker_core::api::{capability_report, default_match_setup, estimate_base_offset, SyncEngine};
use tracker_core::device::{CapabilityProbeResult, DeviceTier, TimestampSource};

#[test]
fn sync_engine_runs_a_full_burst_and_reports_quality() {
    let transport = Arc::new(FakeTransport::new());
    let engine = SyncEngine::new(transport.clone());

    engine.start(1000.0);
    for message in transport.sent_messages() {
        if let tracker_core::link::LinkMessage::Ping { id, t1 } = message {
            let t2 = t1 + 0.005 + 0.4;
            let t3 = t2 + 0.001;
            engine.handle_pong(id, t1, t2, t3, t3 - 0.4 + 0.005);
        }
    }

    let snapshot = engine.snapshot();
    assert!((snapshot.model.offset_seconds - 0.4).abs() < 0.001);
    assert!(snapshot.model.residual_std_ms < 2.0);
    assert_eq!(snapshot.samples.len(), 50);
}

#[test]
fn sync_engine_records_gaps() {
    let transport = Arc::new(FakeTransport::new());
    let engine = SyncEngine::new(transport);

    engine.start(1000.0);
    engine.link_did_change(false, 1100.0);
    engine.link_did_change(true, 1150.0);

    let snapshot = engine.snapshot();
    assert_eq!(snapshot.gaps.len(), 1);
    assert_eq!(snapshot.gaps[0].end_host_time, Some(1150.0));
}

#[test]
fn capability_report_is_reachable_through_the_facade() {
    let report = capability_report(CapabilityProbeResult {
        manual_sensor: true,
        exposure_lock: true,
        focus_lock: true,
        timestamp_source: TimestampSource::Realtime,
        intrinsics_available: true,
        max_fps: 120,
        min_exposure_seconds: 0.000125,
    });
    assert_eq!(report.tier, DeviceTier::Full);
}

#[test]
fn base_offset_estimate_is_reachable_through_the_facade() {
    let frames = vec![100.000, 100.008, 100.016, 100.024, 100.032, 100.040];
    let delivered = vec![102.040, 102.013, 102.021, 102.029, 102.037, 102.045];
    let estimate = estimate_base_offset(frames, delivered).expect("estymacja powinna sie udac");
    assert!((estimate.offset_ms - 2005.0).abs() < 1.0);
}

#[test]
fn base_offset_estimate_rejects_mismatched_lengths() {
    assert!(estimate_base_offset(vec![1.0, 2.0], vec![1.0]).is_none());
}

#[test]
fn default_match_setup_is_valid() {
    assert!(default_match_setup().validate().is_ok());
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test api`
Expected: FAIL — `unresolved import tracker_core::api`

- [ ] **Step 3: Napisz fasadę**

`src/api.rs`:

```rust
//! Fasada FFI.
//!
//! `SyncSession`, `PeerLink` i `SessionRecorder` maja mutowalny stan
//! i wymagaja `&mut self`, czego UniFFI nie eksportuje. Ta warstwa owija
//! je w `Mutex` i wystawia metody na `&self`.
//!
//! Caly brud FFI mieszka tutaj — logika pozostaje czysta i testowalna
//! bez zadnej wiedzy o bindingach.

use std::sync::{Arc, Mutex};

use crate::clock::{SyncSession, SyncSnapshot};
use crate::device::{
    estimate_timestamp_base_offset, CapabilityProbeResult, CapabilityReport,
};
use crate::link::{LinkMessage, PeerLink, PeerTransport};
use crate::session::{MatchSetup, ManifestError, SessionManifest};

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BaseOffsetEstimate {
    pub offset_ms: f64,
    pub uncertainty_ms: f64,
}

/// Silnik synchronizacji widziany ze Swifta i Kotlina.
#[derive(uniffi::Object)]
pub struct SyncEngine {
    session: Mutex<SyncSession>,
    link: Arc<PeerLink>,
}

#[uniffi::export]
impl SyncEngine {
    #[uniffi::constructor]
    pub fn new(transport: Arc<dyn PeerTransport>) -> Arc<Self> {
        let link = Arc::new(PeerLink::new(transport));
        Arc::new(Self {
            session: Mutex::new(SyncSession::new(link.clone())),
            link,
        })
    }

    pub fn start(&self, now: f64) {
        self.session.lock().unwrap().start(now);
    }

    pub fn tick(&self, now: f64) {
        self.session.lock().unwrap().tick(now);
    }

    /// Wolane, gdy z sieci przyszedl pakiet. Zwraca `true`, jesli pakiet
    /// byl poprawny i nie byl duplikatem.
    pub fn receive(&self, data: Vec<u8>, received_at: f64) -> bool {
        match self.link.receive(&data) {
            Some(message) => {
                self.session.lock().unwrap().handle(&message, received_at);
                true
            }
            None => false,
        }
    }

    /// Sciezka dla testow i dla transportow, ktore dostarczaja juz
    /// zdekodowana odpowiedz.
    pub fn handle_pong(&self, id: u64, t1: f64, t2: f64, t3: f64, received_at: f64) {
        self.session
            .lock()
            .unwrap()
            .handle(&LinkMessage::Pong { id, t1, t2, t3 }, received_at);
    }

    pub fn link_did_change(&self, connected: bool, at: f64) {
        self.link.set_connected(connected);
        self.session.lock().unwrap().link_did_change(connected, at);
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        self.session.lock().unwrap().snapshot()
    }

    /// Jakosc synchronizacji w milisekundach. `None`, dopoki model nie powstal.
    pub fn residual_ms(&self) -> Option<f64> {
        self.session.lock().unwrap().current_fit().map(|fit| fit.residual_std_ms)
    }
}

#[uniffi::export]
pub fn capability_report(probe: CapabilityProbeResult) -> CapabilityReport {
    probe.evaluate()
}

#[uniffi::export]
pub fn estimate_base_offset(
    frame_times: Vec<f64>,
    delivery_times: Vec<f64>,
) -> Option<BaseOffsetEstimate> {
    if frame_times.len() != delivery_times.len() {
        return None;
    }
    let pairs: Vec<(f64, f64)> = frame_times.into_iter().zip(delivery_times).collect();
    estimate_timestamp_base_offset(&pairs).map(|(offset_ms, uncertainty_ms)| BaseOffsetEstimate {
        offset_ms,
        uncertainty_ms,
    })
}

#[uniffi::export]
pub fn default_match_setup() -> MatchSetup {
    MatchSetup::default_singles()
}

#[uniffi::export]
pub fn decode_manifest(json: String) -> Result<SessionManifest, ManifestError> {
    SessionManifest::decode(&json)
}
```

W `src/lib.rs` dodaj `pub mod api;`.

- [ ] **Step 4: Wyeksportuj trait transportu jako interfejs callbacku**

W `src/link/peer.rs` dodaj atrybut nad deklaracją traitu:

```rust
#[uniffi::export(with_foreign)]
pub trait PeerTransport: Send + Sync {
```

`with_foreign` oznacza, że implementację może dostarczyć strona natywna — dokładnie tego chcemy, bo MultipeerConnectivity i BLE żyją w Swifcie.

- [ ] **Step 5: Uruchom testy i potwierdź, że przechodzą**

Run: `cargo test --test api`
Expected: PASS, 6 testów

- [ ] **Step 6: Napisz skrypt budujący bindingi**

`scripts/build-bindings.sh`:

```bash
#!/usr/bin/env bash
# Generuje bindingi Swift i Kotlin z crate'u tracker-core.
#
# Uzycie: ./scripts/build-bindings.sh
# Wynik:  bindings/swift/, bindings/kotlin/
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> budowanie biblioteki"
cargo build --release

LIB="target/release/libtracker_core.dylib"
if [ ! -f "$LIB" ]; then
    LIB="target/release/libtracker_core.so"
fi

echo "==> generowanie bindingow Swift"
rm -rf bindings/swift
cargo run --features cli --bin uniffi-bindgen -- \
    generate --library "$LIB" --language swift --out-dir bindings/swift

echo "==> generowanie bindingow Kotlin"
rm -rf bindings/kotlin
cargo run --features cli --bin uniffi-bindgen -- \
    generate --library "$LIB" --language kotlin --out-dir bindings/kotlin

echo "==> gotowe"
ls -la bindings/swift bindings/kotlin
```

Nadaj prawa wykonywania: `chmod +x scripts/build-bindings.sh`

- [ ] **Step 7: Wygeneruj bindingi i sprawdź, co powstało**

Run: `./scripts/build-bindings.sh`
Expected: powstają `bindings/swift/tracker_core.swift`, `bindings/swift/tracker_coreFFI.h`, `bindings/swift/tracker_coreFFI.modulemap` oraz `bindings/kotlin/.../tracker_core.kt`

Sprawdź, że wygenerowany Swift zawiera to, czego użyje aplikacja:

```bash
grep -c "class SyncEngine" bindings/swift/tracker_core.swift
grep -c "protocol PeerTransport" bindings/swift/tracker_core.swift
grep -c "struct SessionManifest" bindings/swift/tracker_core.swift
grep -c "enum DeviceTier" bindings/swift/tracker_core.swift
```

Każde polecenie musi zwrócić wartość większą od zera. Jeśli któreś zwróci 0, brakujący typ nie przekracza granicy FFI — sprawdź, czy ma odpowiedni `derive` albo `#[uniffi::export]`.

- [ ] **Step 8: Commit**

```bash
git add src/api.rs src/lib.rs src/link/peer.rs scripts tests/api.rs
git commit -m "feat(api): fasada UniFFI i generacja bindingow Swift oraz Kotlin"
```

---

## Kryteria ukończenia planu

1. `cargo test` przechodzi w całości — wszystkie 13 zadań.
2. `cargo clippy -- -D warnings` nie zgłasza uwag.
3. `cargo fmt --check` przechodzi.
4. `./scripts/build-bindings.sh` generuje bindingi Swift i Kotlin bez błędów.
5. Wygenerowany Swift zawiera `SyncEngine`, `PeerTransport`, `SessionManifest`, `DeviceTier`, `MatchSetup`, `CapabilityReport`.
6. Żaden moduł poza `api.rs` nie odwołuje się do czasu systemowego — sprawdzenie: `grep -rn "SystemTime\|Instant::now" src/` nie zwraca nic.

Punkt 6 jest twardy. Odczyt zegara wewnątrz rdzenia unieważniłby całą strategię testowania i wprowadziłby niedeterminizm do modułu, na którym stoi reszta projektu.

## Co dalej

Po zazielenieniu tego planu powstaje **P0a część 2**: aplikacja iOS (AVFoundation, MultipeerConnectivity, BLE, CoreMotion), aplikacja watchOS oraz narzędzia Python weryfikujące sesje. Część 2 konsumuje `bindings/swift/` wygenerowane w zadaniu 13.
