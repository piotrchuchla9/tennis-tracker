# P0 — Rig danych: plan implementacji

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aplikacja iOS na dwa iPhone'y, która nagrywa zsynchronizowane wideo z audio ze sztywno zablokowanymi parametrami kamery i zapisuje komplet metadanych pozwalających w P1 odtworzyć wspólną oś czasu i geometrię.

**Architecture:** Cała logika bezsprzętowa mieszka w pakiecie SwiftPM `RigCore`, testowanym natywnie na macOS przez `swift test` — bez symulatora, bez Xcode, pętla TDD w sekundach. Warstwa sprzętowa (AVFoundation, MultipeerConnectivity, CoreMotion) to cienkie adaptery za protokołami, wstrzykiwane do `RigCore` i weryfikowane ręcznie na urządzeniach. Aplikacja iOS jest powłoką spinającą jedno z drugim.

**Tech Stack:** Swift 5.9+, SwiftPM, XCTest, SwiftUI, AVFoundation, MultipeerConnectivity, CoreMotion. Python 3 + ffmpeg dla narzędzia weryfikującego.

**Spec:** `docs/superpowers/specs/2026-09-02-tennis-tracker-p0-rig-danych-design.md`

## Global Constraints

- Sprzęt docelowy: iPhone 16 Pro Max (**master**) oraz iPhone 15 (**slave**). Rola wybierana jawnie w UI, domyślnie 16 Pro Max jako master.
- Minimalna platforma: **iOS 17**, macOS 14 dla testów pakietu.
- Konto Apple Developer w wersji darmowej — apka wygasa po 7 dniach; UI musi pokazywać datę wygaśnięcia.
- **Wyłącznie zegar monotoniczny.** `mach_absolute_time` / `CLOCK_MONOTONIC_RAW`. Nigdy `Date()` ani żaden czas ścienny do znaczników zdarzeń. Nazwa domeny w manifeście: `"mach_absolute_time"`, jednostka: **sekundy**, typ: `Double`.
- Cel dokładności synchronizacji: **poniżej 2 ms** (`residualStdMs < 2.0`).
- Model synchronizacji jest **liniowy: offset ORAZ skew**. Sama wartość offsetu jest niewystarczająca.
- Profil nagrywania konfigurowalny, domyślny **`1080p120`**; pozostałe: `4K60`, `1080p60`. Wartość zapisywana w manifeście.
- Audio: **PCM, 48000 Hz, mono**, w tym samym pliku co wideo. Kompresja stratna wykluczona.
- Wideo: HEVC, `targetBitrateMbps` domyślnie **40**.
- Segmenty po **300 sekund** (5 minut).
- Timeout ciszy peera kończący sesję po stronie slave'a: **120 sekund**.
- `schemaVersion` manifestu: **1**.
- Utrata łączności **nigdy** nie zatrzymuje nagrywania.
- Język komentarzy w kodzie i komunikatów UI: polski. Nazwy symboli: angielski.

---

## Struktura plików

```
Package.swift                                    pakiet SwiftPM RigCore
Sources/RigCore/
  Clock/ClockSample.swift                        próbka NTP-style, offset i delay
  Clock/ClockFit.swift                           wynik dopasowania liniowego
  Clock/ClockSyncEstimator.swift                 filtr best-delay + regresja
  Clock/ClockModel.swift                         konwersja czasu w obie strony
  Clock/SyncSession.swift                        harmonogram pingów, luki
  Link/LinkMessage.swift                         wiadomości protokołu
  Link/LinkCodec.swift                           koperta i (de)serializacja
  Link/PeerTransport.swift                       protokół transportu
  Link/PeerLink.swift                            numeracja, deduplikacja, stan łącza
  Session/SessionRole.swift                      rola urządzenia
  Session/CaptureProfile.swift                   profile nagrywania
  Session/SessionManifest.swift                  model manifestu + walidacja
  Session/MotionAnalyzer.swift                   detekcja szarpnięć
  Session/RecorderPorts.swift                    protokoły: kamera, dysk, termika, zegar
  Session/SessionRecorder.swift                  maszyna stanów sesji
Tests/RigCoreTests/
  ClockSampleTests.swift
  ClockSyncEstimatorTests.swift
  ClockModelTests.swift
  SyncSessionTests.swift
  LinkCodecTests.swift
  PeerLinkTests.swift
  SessionManifestTests.swift
  MotionAnalyzerTests.swift
  SessionRecorderTests.swift
  Support/SeededRandom.swift
  Support/SampleFactory.swift
  Support/FakeTransport.swift
  Support/FakeRecorderPorts.swift
App/TennisTrackerRig.xcodeproj                   projekt iOS
App/TennisTrackerRig/
  TennisTrackerRigApp.swift                      punkt wejścia
  Adapters/MonotonicClock.swift                  Clocking na mach_absolute_time
  Adapters/MultipeerTransport.swift              PeerTransport na MultipeerConnectivity
  Adapters/CaptureRig.swift                      CaptureControlling na AVFoundation
  Adapters/SystemProbes.swift                    StorageProbing, ThermalProbing
  Adapters/MotionRecorder.swift                  CoreMotion + zapis motion.jsonl
  Session/SessionCoordinator.swift               spina RigCore z adapterami
  Session/SessionStore.swift                     katalogi sesji, zapis manifestu
  UI/RigView.swift                               ekran główny
  UI/PreviewView.swift                           podgląd kamery + poziomica
  UI/ExportView.swift                            eksport sesji
  Info.plist                                     uprawnienia i Bonjour
tools/verify_session.py                          weryfikacja sesji + test akceptacyjny
tools/ms_counter.html                            licznik milisekundowy na ekran Maca
tools/requirements.txt
```

Podział jest podporządkowany testowalności: wszystko w `Sources/RigCore` jest czystą logiką bez zależności od sprzętu i pokryte testami jednostkowymi uruchamianymi na Macu bez symulatora. Wszystko w `App/…/Adapters` to cienkie warstwy bez decyzji, weryfikowane ręcznie na urządzeniu.

---

### Task 1: Pakiet RigCore i model próbki zegara

**Files:**
- Create: `Package.swift`
- Create: `Sources/RigCore/Clock/ClockSample.swift`
- Create: `.gitignore`
- Test: `Tests/RigCoreTests/ClockSampleTests.swift`

**Interfaces:**
- Consumes: nic
- Produces: `struct ClockSample` z `init(t1:t2:t3:t4:)` oraz właściwościami `offset: Double`, `delay: Double`, `localTime: Double`

- [ ] **Step 1: Utwórz `Package.swift`**

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "RigCore",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "RigCore", targets: ["RigCore"])
    ],
    targets: [
        .target(name: "RigCore"),
        .testTarget(name: "RigCoreTests", dependencies: ["RigCore"])
    ]
)
```

- [ ] **Step 2: Utwórz `.gitignore`**

```
.DS_Store
.build/
.swiftpm/
DerivedData/
*.xcuserstate
xcuserdata/
tools/__pycache__/
tools/out/
```

- [ ] **Step 3: Napisz test, który ma nie przejść**

Plik `Tests/RigCoreTests/ClockSampleTests.swift`:

```swift
import XCTest
@testable import RigCore

final class ClockSampleTests: XCTestCase {

    /// Zegar zdalny wyprzedza lokalny o 0,5 s, opoznienie w obie strony po 10 ms,
    /// przetwarzanie po stronie zdalnej 2 ms.
    func testOffsetAndDelayForSymmetricPath() {
        let sample = ClockSample(t1: 100.000, t2: 100.510, t3: 100.512, t4: 100.022)

        XCTAssertEqual(sample.offset, 0.5, accuracy: 1e-9)
        XCTAssertEqual(sample.delay, 0.020, accuracy: 1e-9)
        XCTAssertEqual(sample.localTime, 100.011, accuracy: 1e-9)
    }

    /// Asymetria sciezki przenosi sie na blad offsetu rowny polowie roznicy opoznien.
    func testAsymmetricPathBiasesOffsetByHalfTheDifference() {
        // droga tam 30 ms, droga z powrotem 10 ms, offset prawdziwy 0
        let sample = ClockSample(t1: 0.000, t2: 0.030, t3: 0.031, t4: 0.041)

        XCTAssertEqual(sample.offset, 0.010, accuracy: 1e-9)
        XCTAssertEqual(sample.delay, 0.040, accuracy: 1e-9)
    }

    func testZeroOffsetForIdenticalClocks() {
        let sample = ClockSample(t1: 10.0, t2: 10.005, t3: 10.006, t4: 10.011)

        XCTAssertEqual(sample.offset, 0.0, accuracy: 1e-9)
        XCTAssertEqual(sample.delay, 0.010, accuracy: 1e-9)
    }
}
```

- [ ] **Step 4: Uruchom test i potwierdź, że nie przechodzi**

Run: `swift test --filter ClockSampleTests`
Expected: FAIL, kompilacja nie przechodzi — `cannot find 'ClockSample' in scope`

- [ ] **Step 5: Napisz minimalną implementację**

Plik `Sources/RigCore/Clock/ClockSample.swift`:

```swift
import Foundation

/// Pojedyncza wymiana ping-pong w stylu NTP.
///
/// Wszystkie czasy podane sa w sekundach, w monotonicznej domenie zegara
/// urzadzenia, ktore je zmierzylo: `t1` i `t4` na zegarze lokalnym,
/// `t2` i `t3` na zegarze zdalnym.
public struct ClockSample: Equatable, Sendable {

    /// Wyslanie pingu, zegar lokalny.
    public let t1: Double
    /// Odbior pingu, zegar zdalny.
    public let t2: Double
    /// Wyslanie ponga, zegar zdalny.
    public let t3: Double
    /// Odbior ponga, zegar lokalny.
    public let t4: Double

    public init(t1: Double, t2: Double, t3: Double, t4: Double) {
        self.t1 = t1
        self.t2 = t2
        self.t3 = t3
        self.t4 = t4
    }

    /// Przesuniecie zegara zdalnego wzgledem lokalnego: `zdalny = lokalny + offset`.
    public var offset: Double {
        ((t2 - t1) + (t3 - t4)) / 2
    }

    /// Czas obiegu pomniejszony o czas przetwarzania po stronie zdalnej.
    /// Nizsza wartosc oznacza mniej zaburzona probke.
    public var delay: Double {
        (t4 - t1) - (t3 - t2)
    }

    /// Moment na zegarze lokalnym, do ktorego odnosi sie ta probka.
    public var localTime: Double {
        (t1 + t4) / 2
    }
}
```

- [ ] **Step 6: Uruchom test i potwierdź, że przechodzi**

Run: `swift test --filter ClockSampleTests`
Expected: PASS, 3 testy

- [ ] **Step 7: Commit**

```bash
git add Package.swift .gitignore Sources/RigCore/Clock/ClockSample.swift Tests/RigCoreTests/ClockSampleTests.swift
git commit -m "feat(clock): pakiet RigCore i model probki NTP"
```

---

### Task 2: Estymator synchronizacji — filtr best-delay i dopasowanie liniowe

To najważniejszy moduł P0. Cała reszta projektu opiera się na tym, że wspólna oś czasu jest wiarygodna.

**Files:**
- Create: `Sources/RigCore/Clock/ClockFit.swift`
- Create: `Sources/RigCore/Clock/ClockSyncEstimator.swift`
- Test: `Tests/RigCoreTests/Support/SeededRandom.swift`
- Test: `Tests/RigCoreTests/Support/SampleFactory.swift`
- Test: `Tests/RigCoreTests/ClockSyncEstimatorTests.swift`

**Interfaces:**
- Consumes: `ClockSample` z Task 1
- Produces: `struct ClockFit` (pola `referenceTime`, `offsetSeconds`, `skewPpm`, `residualStdMs`, `sampleCount`) oraz `struct ClockSyncEstimator` z `mutating func add(_:)` i `func fit() -> ClockFit?`

- [ ] **Step 1: Napisz pomocniki testowe**

Plik `Tests/RigCoreTests/Support/SeededRandom.swift`:

```swift
import Foundation

/// Deterministyczny generator liniowy kongruentny. Testy nie moga byc losowe.
struct SeededRandom: RandomNumberGenerator {
    private var state: UInt64

    init(seed: UInt64) {
        state = seed &* 6364136223846793005 &+ 1442695040888963407
    }

    mutating func next() -> UInt64 {
        state = state &* 6364136223846793005 &+ 1442695040888963407
        return state
    }

    /// Wartosc z przedzialu [0, upper).
    mutating func uniform(upTo upper: Double) -> Double {
        Double(next() % 1_000_000) / 1_000_000.0 * upper
    }
}
```

Plik `Tests/RigCoreTests/Support/SampleFactory.swift`:

```swift
import Foundation
@testable import RigCore

/// Buduje probke o zadanych, znanych parametrach sciezki.
/// Odwrotnosc arytmetyki z `ClockSample`, dzieki czemu test wie,
/// jaka wartosc estymator powinien odzyskac.
func makeSample(
    t1: Double,
    offset: Double,
    forward: Double,
    backward: Double,
    processing: Double = 0.001
) -> ClockSample {
    let t2 = t1 + forward + offset
    let t3 = t2 + processing
    let t4 = t3 - offset + backward
    return ClockSample(t1: t1, t2: t2, t3: t3, t4: t4)
}

/// Seria probek z zadanym offsetem poczatkowym, dryfem i jitterem.
/// - Parameter outlierEvery: co ile probek wstawic pakiet o ogromnym opoznieniu.
func makeSeries(
    start: Double,
    count: Int,
    period: Double,
    offset0: Double,
    skewPpm: Double,
    baseOneWay: Double,
    jitter: Double,
    seed: UInt64,
    outlierEvery: Int? = nil
) -> [ClockSample] {
    var rng = SeededRandom(seed: seed)
    return (0..<count).map { index in
        let t1 = start + Double(index) * period
        let trueOffset = offset0 + skewPpm * 1e-6 * (t1 - start)

        var forward = baseOneWay + rng.uniform(upTo: jitter)
        var backward = baseOneWay + rng.uniform(upTo: jitter)

        if let every = outlierEvery, every > 0, index % every == 0 {
            forward += 0.250
            backward += 0.050
        }

        return makeSample(t1: t1, offset: trueOffset, forward: forward, backward: backward)
    }
}
```

- [ ] **Step 2: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/ClockSyncEstimatorTests.swift`:

```swift
import XCTest
@testable import RigCore

final class ClockSyncEstimatorTests: XCTestCase {

    private func estimator(with samples: [ClockSample]) -> ClockSyncEstimator {
        var estimator = ClockSyncEstimator()
        for sample in samples { estimator.add(sample) }
        return estimator
    }

    func testReturnsNilBelowMinimumSampleCount() {
        let samples = makeSeries(
            start: 1000, count: 5, period: 1.0,
            offset0: 0.25, skewPpm: 0, baseOneWay: 0.005, jitter: 0.0005, seed: 1
        )
        XCTAssertNil(estimator(with: samples).fit())
    }

    func testRecoversOffsetWithNoSkew() {
        let samples = makeSeries(
            start: 1000, count: 60, period: 1.0,
            offset0: 0.25, skewPpm: 0, baseOneWay: 0.005, jitter: 0.0004, seed: 2
        )
        let fit = try! XCTUnwrap(estimator(with: samples).fit())

        XCTAssertEqual(fit.offsetSeconds, 0.25, accuracy: 0.0005)
        XCTAssertEqual(fit.skewPpm, 0.0, accuracy: 5.0)
        XCTAssertLessThan(fit.residualStdMs, 1.0)
    }

    func testRecoversSkewOverLongSeries() {
        // 20 ppm przez godzine to 72 ms rozjazdu
        let samples = makeSeries(
            start: 1000, count: 3600, period: 1.0,
            offset0: 0.10, skewPpm: 20, baseOneWay: 0.005, jitter: 0.0004, seed: 3
        )
        let fit = try! XCTUnwrap(estimator(with: samples).fit())

        XCTAssertEqual(fit.skewPpm, 20.0, accuracy: 1.0)
    }

    func testOutliersAreRejectedByDelayFilter() {
        // co dziesiata probka ma 250 ms dodatkowego opoznienia w jedna strone
        let samples = makeSeries(
            start: 1000, count: 300, period: 1.0,
            offset0: 0.25, skewPpm: 5, baseOneWay: 0.005, jitter: 0.0004,
            seed: 4, outlierEvery: 10
        )
        let fit = try! XCTUnwrap(estimator(with: samples).fit())

        XCTAssertEqual(fit.offsetSeconds, 0.25, accuracy: 0.002)
        XCTAssertEqual(fit.skewPpm, 5.0, accuracy: 2.0)
    }

    func testSurvivesGapInSeries() {
        // 60 probek, przerwa 300 s, kolejne 60 probek
        let first = makeSeries(
            start: 1000, count: 60, period: 1.0,
            offset0: 0.10, skewPpm: 10, baseOneWay: 0.005, jitter: 0.0004, seed: 5
        )
        let second = makeSeries(
            start: 1360, count: 60, period: 1.0,
            offset0: 0.10 + 10 * 1e-6 * 360, skewPpm: 10,
            baseOneWay: 0.005, jitter: 0.0004, seed: 6
        )
        let fit = try! XCTUnwrap(estimator(with: first + second).fit())

        XCTAssertEqual(fit.skewPpm, 10.0, accuracy: 1.5)
    }

    func testWindowDropsOldestSamples() {
        var estimator = ClockSyncEstimator()
        estimator.windowSize = 50
        let samples = makeSeries(
            start: 1000, count: 200, period: 1.0,
            offset0: 0.10, skewPpm: 0, baseOneWay: 0.005, jitter: 0.0004, seed: 7
        )
        for sample in samples { estimator.add(sample) }

        let fit = try! XCTUnwrap(estimator.fit())
        XCTAssertLessThanOrEqual(fit.sampleCount, 50)
        // punkt odniesienia musi lezec w ostatnim oknie, nie na poczatku serii
        XCTAssertGreaterThan(fit.referenceTime, 1140.0)
    }
}
```

- [ ] **Step 3: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter ClockSyncEstimatorTests`
Expected: FAIL, `cannot find 'ClockSyncEstimator' in scope`

- [ ] **Step 4: Napisz `ClockFit`**

Plik `Sources/RigCore/Clock/ClockFit.swift`:

```swift
import Foundation

/// Wynik dopasowania liniowego modelu zegara.
///
/// Model: `offset(t) = offsetSeconds + skewPpm * 1e-6 * (t - referenceTime)`,
/// gdzie `t` jest czasem lokalnym w sekundach.
public struct ClockFit: Equatable, Sendable {
    public let referenceTime: Double
    public let offsetSeconds: Double
    public let skewPpm: Double
    public let residualStdMs: Double
    public let sampleCount: Int

    public init(
        referenceTime: Double,
        offsetSeconds: Double,
        skewPpm: Double,
        residualStdMs: Double,
        sampleCount: Int
    ) {
        self.referenceTime = referenceTime
        self.offsetSeconds = offsetSeconds
        self.skewPpm = skewPpm
        self.residualStdMs = residualStdMs
        self.sampleCount = sampleCount
    }
}
```

- [ ] **Step 5: Napisz `ClockSyncEstimator`**

Plik `Sources/RigCore/Clock/ClockSyncEstimator.swift`:

```swift
import Foundation

/// Estymuje liniowy model roznicy zegarow z serii probek ping-pong.
///
/// Dwie decyzje projektowe niosa tu cala jakosc wyniku:
///
/// 1. Do dopasowania trafiaja wylacznie probki o najnizszym `delay`.
///    Pakiet, ktory przeszedl najszybciej, przeszedl najmniej zaburzony,
///    wiec jego `offset` jest najblizszy prawdy. Filtr ten zastepuje
///    odporna statystyke i sam z siebie usuwa wartosci odstajace.
/// 2. Dopasowywana jest prosta, nie stala. Kwarce dwoch urzadzen chodza
///    z rozna predkoscia i przez godzine rozjezdzaja sie o dziesiatki
///    milisekund. Bez czlonu `skew` model rozsypuje sie w trakcie meczu.
public struct ClockSyncEstimator: Sendable {

    /// Ile ostatnich probek trzymamy.
    public var windowSize: Int = 300
    /// Jaka czesc okna, licząc od najnizszego `delay`, trafia do dopasowania.
    public var bestFraction: Double = 0.25
    /// Ponizej tej liczby probek nie zwracamy modelu.
    public var minimumSamples: Int = 8

    private var samples: [ClockSample] = []

    public init() {}

    public mutating func add(_ sample: ClockSample) {
        samples.append(sample)
        if samples.count > windowSize {
            samples.removeFirst(samples.count - windowSize)
        }
    }

    public var count: Int { samples.count }

    public mutating func reset() { samples.removeAll() }

    /// Probki wybrane do dopasowania, posortowane rosnaco po `localTime`.
    func selectedSamples() -> [ClockSample] {
        guard samples.count >= minimumSamples else { return [] }
        let byDelay = samples.sorted { $0.delay < $1.delay }
        let wanted = max(minimumSamples, Int((Double(byDelay.count) * bestFraction).rounded(.up)))
        return byDelay.prefix(min(wanted, byDelay.count)).sorted { $0.localTime < $1.localTime }
    }

    public func fit() -> ClockFit? {
        let chosen = selectedSamples()
        guard chosen.count >= minimumSamples else { return nil }

        let times = chosen.map(\.localTime)
        let offsets = chosen.map(\.offset)
        let n = Double(chosen.count)

        // Punkt odniesienia w srodku ciezkosci probek. Dzieki temu suma
        // odchylek x jest zerowa, a wyraz wolny rowna sie sredniej offsetow.
        let reference = times.reduce(0, +) / n
        let meanOffset = offsets.reduce(0, +) / n

        var sxx = 0.0
        var sxy = 0.0
        for (time, offset) in zip(times, offsets) {
            let x = time - reference
            sxx += x * x
            sxy += x * (offset - meanOffset)
        }
        let slope = sxx > 0 ? sxy / sxx : 0

        var sumSquares = 0.0
        for (time, offset) in zip(times, offsets) {
            let predicted = meanOffset + slope * (time - reference)
            let residual = offset - predicted
            sumSquares += residual * residual
        }
        // Dwa stopnie swobody zjada dopasowana prosta.
        let variance = chosen.count > 2 ? sumSquares / (n - 2) : 0
        let residualStd = variance.squareRoot()

        return ClockFit(
            referenceTime: reference,
            offsetSeconds: meanOffset,
            skewPpm: slope * 1e6,
            residualStdMs: residualStd * 1000,
            sampleCount: chosen.count
        )
    }
}
```

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter ClockSyncEstimatorTests`
Expected: PASS, 6 testów

- [ ] **Step 7: Commit**

```bash
git add Sources/RigCore/Clock/ClockFit.swift Sources/RigCore/Clock/ClockSyncEstimator.swift Tests/RigCoreTests/Support Tests/RigCoreTests/ClockSyncEstimatorTests.swift
git commit -m "feat(clock): estymator offsetu i skew z filtrem best-delay"
```

---

### Task 3: Konwersja czasu między urządzeniami

**Files:**
- Create: `Sources/RigCore/Clock/ClockModel.swift`
- Test: `Tests/RigCoreTests/ClockModelTests.swift`

**Interfaces:**
- Consumes: `ClockFit` z Task 2
- Produces: `struct ClockModel` z `init(fit:)`, `func peerTime(atLocal: Double) -> Double`, `func localTime(atPeer: Double) -> Double`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/ClockModelTests.swift`:

```swift
import XCTest
@testable import RigCore

final class ClockModelTests: XCTestCase {

    private let fit = ClockFit(
        referenceTime: 1000,
        offsetSeconds: 0.25,
        skewPpm: 20,
        residualStdMs: 0.8,
        sampleCount: 75
    )

    func testPeerTimeAtReferenceIsLocalPlusOffset() {
        let model = ClockModel(fit: fit)
        XCTAssertEqual(model.peerTime(atLocal: 1000), 1000.25, accuracy: 1e-9)
    }

    func testSkewAccumulatesOverTime() {
        let model = ClockModel(fit: fit)
        // 20 ppm przez 3600 s to 72 ms
        XCTAssertEqual(model.peerTime(atLocal: 4600), 4600.25 + 0.072, accuracy: 1e-9)
    }

    func testRoundTripIsExact() {
        let model = ClockModel(fit: fit)
        for local in stride(from: 900.0, through: 5000.0, by: 137.0) {
            let peer = model.peerTime(atLocal: local)
            XCTAssertEqual(model.localTime(atPeer: peer), local, accuracy: 1e-6)
        }
    }

    func testZeroSkewBehavesAsConstantOffset() {
        let flat = ClockFit(
            referenceTime: 0, offsetSeconds: -1.5,
            skewPpm: 0, residualStdMs: 0.1, sampleCount: 20
        )
        let model = ClockModel(fit: flat)
        XCTAssertEqual(model.peerTime(atLocal: 12345), 12343.5, accuracy: 1e-9)
        XCTAssertEqual(model.localTime(atPeer: 12343.5), 12345, accuracy: 1e-9)
    }
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter ClockModelTests`
Expected: FAIL, `cannot find 'ClockModel' in scope`

- [ ] **Step 3: Napisz implementację**

Plik `Sources/RigCore/Clock/ClockModel.swift`:

```swift
import Foundation

/// Przelicza czas miedzy zegarem lokalnym a zegarem peera.
///
/// `peer = local + offset + skew * (local - reference)`
public struct ClockModel: Equatable, Sendable {

    public let fit: ClockFit

    public init(fit: ClockFit) {
        self.fit = fit
    }

    private var skew: Double { fit.skewPpm * 1e-6 }

    public func peerTime(atLocal localTime: Double) -> Double {
        localTime + fit.offsetSeconds + skew * (localTime - fit.referenceTime)
    }

    /// Odwrocenie modelu:
    /// `peer = local * (1 + skew) + offset - skew * reference`
    public func localTime(atPeer peerTime: Double) -> Double {
        (peerTime - fit.offsetSeconds + skew * fit.referenceTime) / (1 + skew)
    }
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter ClockModelTests`
Expected: PASS, 4 testy

- [ ] **Step 5: Commit**

```bash
git add Sources/RigCore/Clock/ClockModel.swift Tests/RigCoreTests/ClockModelTests.swift
git commit -m "feat(clock): konwersja czasu miedzy urzadzeniami"
```

---

### Task 4: Protokół wiadomości i koperta

**Files:**
- Create: `Sources/RigCore/Session/SessionRole.swift`
- Create: `Sources/RigCore/Session/CaptureProfile.swift`
- Create: `Sources/RigCore/Link/LinkMessage.swift`
- Create: `Sources/RigCore/Link/LinkCodec.swift`
- Test: `Tests/RigCoreTests/LinkCodecTests.swift`

**Interfaces:**
- Consumes: `ClockSample` z Task 1
- Produces: `enum SessionRole`, `enum CaptureProfile` (z `width`, `height`, `fps`), `enum LinkMessage`, `struct LinkEnvelope`, `enum LinkCodec` z `encode(_:)` / `decode(_:)`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/LinkCodecTests.swift`:

```swift
import XCTest
@testable import RigCore

final class LinkCodecTests: XCTestCase {

    private func roundTrip(_ message: LinkMessage, line: UInt = #line) throws {
        let envelope = LinkEnvelope(sequence: 42, message: message)
        let data = try LinkCodec.encode(envelope)
        let decoded = try LinkCodec.decode(data)
        XCTAssertEqual(decoded, envelope, line: line)
    }

    func testRoundTripsEveryMessageCase() throws {
        try roundTrip(.hello(deviceModel: "iPhone17,2", appVersion: "0.1.0", preferredRole: .master))
        try roundTrip(.roleAssigned(.slave))
        try roundTrip(.ping(id: 7, t1: 1000.5))
        try roundTrip(.pong(id: 7, t1: 1000.5, t2: 1000.75, t3: 1000.751))
        try roundTrip(.startRecording(sessionId: "s-1", profile: .p1080_120, hostTime: 12.5))
        try roundTrip(.stopRecording(hostTime: 900.0))
        try roundTrip(.heartbeat(hostTime: 33.0))
        try roundTrip(.syncLog([ClockSample(t1: 1, t2: 2, t3: 3, t4: 4)]))
    }

    func testDecodingGarbageThrows() {
        XCTAssertThrowsError(try LinkCodec.decode(Data([0x00, 0x01, 0x02])))
    }

    func testCaptureProfileGeometry() {
        XCTAssertEqual(CaptureProfile.p1080_120.width, 1920)
        XCTAssertEqual(CaptureProfile.p1080_120.height, 1080)
        XCTAssertEqual(CaptureProfile.p1080_120.fps, 120)

        XCTAssertEqual(CaptureProfile.p4k_60.width, 3840)
        XCTAssertEqual(CaptureProfile.p4k_60.height, 2160)
        XCTAssertEqual(CaptureProfile.p4k_60.fps, 60)

        XCTAssertEqual(CaptureProfile.p1080_60.fps, 60)
        XCTAssertEqual(CaptureProfile.p1080_60.width, 1920)
    }

    func testProfileRawValuesMatchManifestVocabulary() {
        XCTAssertEqual(CaptureProfile.p1080_120.rawValue, "1080p120")
        XCTAssertEqual(CaptureProfile.p4k_60.rawValue, "4K60")
        XCTAssertEqual(CaptureProfile.p1080_60.rawValue, "1080p60")
    }
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter LinkCodecTests`
Expected: FAIL, `cannot find 'LinkMessage' in scope`

- [ ] **Step 3: Napisz typy pomocnicze**

Plik `Sources/RigCore/Session/SessionRole.swift`:

```swift
import Foundation

public enum SessionRole: String, Codable, Equatable, Sendable {
    case master
    case slave

    public var opposite: SessionRole {
        self == .master ? .slave : .master
    }
}
```

Plik `Sources/RigCore/Session/CaptureProfile.swift`:

```swift
import Foundation

/// Profil nagrywania. Wybor "rozdzielczosc kontra liczba klatek" jest
/// rozstrzygany empirycznie w P1, dlatego profil jest parametrem sesji
/// i trafia do manifestu.
public enum CaptureProfile: String, Codable, CaseIterable, Equatable, Sendable {
    case p1080_120 = "1080p120"
    case p4k_60 = "4K60"
    case p1080_60 = "1080p60"

    public var width: Int {
        switch self {
        case .p1080_120, .p1080_60: return 1920
        case .p4k_60: return 3840
        }
    }

    public var height: Int {
        switch self {
        case .p1080_120, .p1080_60: return 1080
        case .p4k_60: return 2160
        }
    }

    public var fps: Int {
        switch self {
        case .p1080_120: return 120
        case .p4k_60, .p1080_60: return 60
        }
    }
}
```

- [ ] **Step 4: Napisz `LinkMessage` i `LinkCodec`**

Plik `Sources/RigCore/Link/LinkMessage.swift`:

```swift
import Foundation

/// Wiadomosci wymieniane miedzy urzadzeniami.
///
/// Kazda wiadomosc niesie wlasne znaczniki czasu i jest samoopisujaca,
/// dzieki czemu odbiorca nie zaklada nic o kolejnosci dostarczenia.
public enum LinkMessage: Codable, Equatable, Sendable {
    case hello(deviceModel: String, appVersion: String, preferredRole: SessionRole)
    case roleAssigned(SessionRole)
    case ping(id: UInt64, t1: Double)
    case pong(id: UInt64, t1: Double, t2: Double, t3: Double)
    case startRecording(sessionId: String, profile: CaptureProfile, hostTime: Double)
    case stopRecording(hostTime: Double)
    case heartbeat(hostTime: Double)
    case syncLog([ClockSample])
}

extension ClockSample: Codable {
    private enum CodingKeys: String, CodingKey { case t1, t2, t3, t4 }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            t1: try container.decode(Double.self, forKey: .t1),
            t2: try container.decode(Double.self, forKey: .t2),
            t3: try container.decode(Double.self, forKey: .t3),
            t4: try container.decode(Double.self, forKey: .t4)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(t1, forKey: .t1)
        try container.encode(t2, forKey: .t2)
        try container.encode(t3, forKey: .t3)
        try container.encode(t4, forKey: .t4)
    }
}

/// Koperta z numerem porzadkowym. Numer sluzy wylacznie do wykrywania
/// duplikatow — kolejnosc dostarczenia nie ma znaczenia dla semantyki.
public struct LinkEnvelope: Codable, Equatable, Sendable {
    public let sequence: UInt64
    public let message: LinkMessage

    public init(sequence: UInt64, message: LinkMessage) {
        self.sequence = sequence
        self.message = message
    }
}
```

Plik `Sources/RigCore/Link/LinkCodec.swift`:

```swift
import Foundation

public enum LinkCodec {
    public static func encode(_ envelope: LinkEnvelope) throws -> Data {
        try JSONEncoder().encode(envelope)
    }

    public static func decode(_ data: Data) throws -> LinkEnvelope {
        try JSONDecoder().decode(LinkEnvelope.self, from: data)
    }
}
```

- [ ] **Step 5: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter LinkCodecTests`
Expected: PASS, 4 testy

- [ ] **Step 6: Commit**

```bash
git add Sources/RigCore/Session/SessionRole.swift Sources/RigCore/Session/CaptureProfile.swift Sources/RigCore/Link Tests/RigCoreTests/LinkCodecTests.swift
git commit -m "feat(link): protokol wiadomosci i koperta z numeracja"
```

---

### Task 5: Warstwa łącza — numeracja, deduplikacja, stan połączenia

**Files:**
- Create: `Sources/RigCore/Link/PeerTransport.swift`
- Create: `Sources/RigCore/Link/PeerLink.swift`
- Test: `Tests/RigCoreTests/Support/FakeTransport.swift`
- Test: `Tests/RigCoreTests/PeerLinkTests.swift`

**Interfaces:**
- Consumes: `LinkEnvelope`, `LinkCodec`, `LinkMessage` z Task 4
- Produces: `protocol PeerTransport`, `final class PeerLink` z `send(_ message: LinkMessage)`, `var onMessage: ((LinkMessage) -> Void)?`, `var onConnectionChange: ((Bool) -> Void)?`, `var isConnected: Bool`

- [ ] **Step 1: Napisz atrapę transportu**

Plik `Tests/RigCoreTests/Support/FakeTransport.swift`:

```swift
import Foundation
@testable import RigCore

/// Transport w pamieci. Pozwala testom podac dowolna sekwencje pakietow,
/// wlacznie z duplikatami i kolejnoscia odwrocona.
final class FakeTransport: PeerTransport {
    var onReceive: ((Data) -> Void)?
    var onConnectionChange: ((Bool) -> Void)?

    private(set) var sent: [Data] = []
    private(set) var isConnected: Bool = true
    var sendShouldThrow = false

    struct SendFailure: Error {}

    func send(_ data: Data) throws {
        if sendShouldThrow { throw SendFailure() }
        sent.append(data)
    }

    // MARK: sterowanie z testu

    func deliver(_ data: Data) {
        onReceive?(data)
    }

    func setConnected(_ connected: Bool) {
        isConnected = connected
        onConnectionChange?(connected)
    }

    func sentMessages() throws -> [LinkMessage] {
        try sent.map { try LinkCodec.decode($0).message }
    }

    func sentEnvelopes() throws -> [LinkEnvelope] {
        try sent.map { try LinkCodec.decode($0) }
    }
}
```

- [ ] **Step 2: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/PeerLinkTests.swift`:

```swift
import XCTest
@testable import RigCore

final class PeerLinkTests: XCTestCase {

    private var transport: FakeTransport!
    private var link: PeerLink!
    private var received: [LinkMessage]!

    override func setUp() {
        super.setUp()
        transport = FakeTransport()
        link = PeerLink(transport: transport)
        received = []
        link.onMessage = { [weak self] message in self?.received.append(message) }
    }

    func testAssignsIncreasingSequenceNumbers() throws {
        try link.send(.heartbeat(hostTime: 1))
        try link.send(.heartbeat(hostTime: 2))
        try link.send(.heartbeat(hostTime: 3))

        let sequences = try transport.sentEnvelopes().map(\.sequence)
        XCTAssertEqual(sequences, [0, 1, 2])
    }

    func testDeliversReceivedMessage() throws {
        let data = try LinkCodec.encode(LinkEnvelope(sequence: 0, message: .heartbeat(hostTime: 5)))
        transport.deliver(data)

        XCTAssertEqual(received, [.heartbeat(hostTime: 5)])
    }

    func testDuplicateSequenceIsDroppedOnce() throws {
        let data = try LinkCodec.encode(LinkEnvelope(sequence: 9, message: .heartbeat(hostTime: 5)))
        transport.deliver(data)
        transport.deliver(data)

        XCTAssertEqual(received.count, 1)
    }

    func testOutOfOrderMessagesAreAllDelivered() throws {
        let second = try LinkCodec.encode(LinkEnvelope(sequence: 2, message: .heartbeat(hostTime: 2)))
        let first = try LinkCodec.encode(LinkEnvelope(sequence: 1, message: .heartbeat(hostTime: 1)))
        transport.deliver(second)
        transport.deliver(first)

        XCTAssertEqual(received, [.heartbeat(hostTime: 2), .heartbeat(hostTime: 1)])
    }

    func testMalformedPacketIsIgnoredAndCounted() {
        transport.deliver(Data([0xFF, 0xFE]))

        XCTAssertTrue(received.isEmpty)
        XCTAssertEqual(link.malformedPacketCount, 1)
    }

    func testConnectionChangeIsForwarded() {
        var states: [Bool] = []
        link.onConnectionChange = { states.append($0) }

        transport.setConnected(false)
        transport.setConnected(true)

        XCTAssertEqual(states, [false, true])
        XCTAssertTrue(link.isConnected)
    }

    func testSendWhileDisconnectedThrows() {
        transport.setConnected(false)
        XCTAssertThrowsError(try link.send(.heartbeat(hostTime: 1)))
    }

    func testSequenceMemoryIsBounded() throws {
        // 5000 roznych numerow, pamiec ograniczona do 1024 ostatnich
        for sequence in 0..<5000 {
            let data = try LinkCodec.encode(
                LinkEnvelope(sequence: UInt64(sequence), message: .heartbeat(hostTime: Double(sequence)))
            )
            transport.deliver(data)
        }
        XCTAssertEqual(received.count, 5000)
        XCTAssertLessThanOrEqual(link.rememberedSequenceCount, 1024)
    }
}
```

- [ ] **Step 3: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter PeerLinkTests`
Expected: FAIL, `cannot find 'PeerLink' in scope`

- [ ] **Step 4: Napisz `PeerTransport`**

Plik `Sources/RigCore/Link/PeerTransport.swift`:

```swift
import Foundation

/// Surowy transport pakietow. Implementacja produkcyjna opiera sie na
/// MultipeerConnectivity; testy podstawiaja atrape w pamieci.
public protocol PeerTransport: AnyObject {
    var onReceive: ((Data) -> Void)? { get set }
    var onConnectionChange: ((Bool) -> Void)? { get set }
    var isConnected: Bool { get }
    func send(_ data: Data) throws
}
```

- [ ] **Step 5: Napisz `PeerLink`**

Plik `Sources/RigCore/Link/PeerLink.swift`:

```swift
import Foundation

/// Warstwa nad surowym transportem: numeruje wysylane koperty,
/// odrzuca duplikaty i izoluje reszte systemu od uszkodzonych pakietow.
///
/// Kolejnosc dostarczenia nie jest wymuszana. Kazda wiadomosc niesie
/// wlasne znaczniki czasu, wiec przetasowanie niczego nie psuje, a
/// wymuszanie kolejnosci kosztowaloby bufor i opoznienie.
public final class PeerLink {

    public enum Failure: Error, Equatable {
        case notConnected
    }

    public var onMessage: ((LinkMessage) -> Void)?
    public var onConnectionChange: ((Bool) -> Void)?

    public private(set) var malformedPacketCount = 0

    private let transport: PeerTransport
    private var nextSequence: UInt64 = 0
    private var seenSequences: Set<UInt64> = []
    private var seenOrder: [UInt64] = []
    private let sequenceMemory = 1024

    public init(transport: PeerTransport) {
        self.transport = transport
        transport.onReceive = { [weak self] data in self?.handle(data) }
        transport.onConnectionChange = { [weak self] connected in
            self?.onConnectionChange?(connected)
        }
    }

    public var isConnected: Bool { transport.isConnected }

    public var rememberedSequenceCount: Int { seenSequences.count }

    public func send(_ message: LinkMessage) throws {
        guard transport.isConnected else { throw Failure.notConnected }
        let envelope = LinkEnvelope(sequence: nextSequence, message: message)
        nextSequence += 1
        try transport.send(try LinkCodec.encode(envelope))
    }

    private func handle(_ data: Data) {
        guard let envelope = try? LinkCodec.decode(data) else {
            malformedPacketCount += 1
            return
        }
        guard !seenSequences.contains(envelope.sequence) else { return }
        remember(envelope.sequence)
        onMessage?(envelope.message)
    }

    private func remember(_ sequence: UInt64) {
        seenSequences.insert(sequence)
        seenOrder.append(sequence)
        if seenOrder.count > sequenceMemory {
            let evicted = seenOrder.removeFirst()
            seenSequences.remove(evicted)
        }
    }
}
```

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter PeerLinkTests`
Expected: PASS, 8 testów

- [ ] **Step 7: Commit**

```bash
git add Sources/RigCore/Link/PeerTransport.swift Sources/RigCore/Link/PeerLink.swift Tests/RigCoreTests/Support/FakeTransport.swift Tests/RigCoreTests/PeerLinkTests.swift
git commit -m "feat(link): warstwa laczna z deduplikacja i stanem polaczenia"
```

---

### Task 6: Sesja synchronizacji — harmonogram pingów i luki

**Files:**
- Create: `Sources/RigCore/Clock/SyncSession.swift`
- Test: `Tests/RigCoreTests/SyncSessionTests.swift`

**Interfaces:**
- Consumes: `PeerLink` (Task 5), `ClockSyncEstimator`/`ClockFit` (Task 2), `LinkMessage` (Task 4)
- Produces: `final class SyncSession` z `func start()`, `func tick(now: Double)`, `func handle(_ message: LinkMessage, receivedAt: Double)`, `var currentFit: ClockFit?`, `var gaps: [SyncGap]`, `var samples: [ClockSample]`; oraz `struct SyncGap` (`startHostTime`, `endHostTime`, `reason`)

Uwaga: `SyncSession` nie mierzy czasu samodzielnie — czas jest podawany z zewnątrz przez `tick(now:)` i `receivedAt`. Dzięki temu testy sterują upływem czasu bez czekania.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/SyncSessionTests.swift`:

```swift
import XCTest
@testable import RigCore

final class SyncSessionTests: XCTestCase {

    private var transport: FakeTransport!
    private var link: PeerLink!
    private var session: SyncSession!

    override func setUp() {
        super.setUp()
        transport = FakeTransport()
        link = PeerLink(transport: transport)
        session = SyncSession(link: link)
    }

    /// Odpowiada na ping tak, jak zrobilby to peer o zadanym offsecie.
    private func replyToPings(offset: Double, oneWay: Double = 0.005) throws {
        for message in try transport.sentMessages() {
            guard case let .ping(id, t1) = message else { continue }
            let t2 = t1 + oneWay + offset
            let t3 = t2 + 0.001
            session.handle(.pong(id: id, t1: t1, t2: t2, t3: t3), receivedAt: t3 - offset + oneWay)
        }
    }

    func testBurstSendsFiftyPingsOnStart() throws {
        session.start(now: 1000)
        let pings = try transport.sentMessages().filter { if case .ping = $0 { return true } else { return false } }
        XCTAssertEqual(pings.count, 50)
    }

    func testProducesFitAfterBurstIsAnswered() throws {
        session.start(now: 1000)
        try replyToPings(offset: 0.4)

        let fit = try XCTUnwrap(session.currentFit)
        XCTAssertEqual(fit.offsetSeconds, 0.4, accuracy: 0.001)
    }

    func testSteadyStateSendsOnePingPerSecond() throws {
        session.start(now: 1000)
        transport.clearSent()

        session.tick(now: 1000.5)
        XCTAssertEqual(try transport.sentMessages().count, 0)

        session.tick(now: 1001.0)
        XCTAssertEqual(try transport.sentMessages().count, 1)

        session.tick(now: 1002.0)
        XCTAssertEqual(try transport.sentMessages().count, 2)
    }

    func testUnmatchedPongIsIgnored() {
        session.start(now: 1000)
        session.handle(.pong(id: 99_999, t1: 1, t2: 2, t3: 3), receivedAt: 4)
        XCTAssertEqual(session.samples.count, 0)
    }

    func testPingIsAnsweredWithPong() throws {
        session.handle(.ping(id: 5, t1: 700.0), receivedAt: 700.31)

        let messages = try transport.sentMessages()
        guard case let .pong(id, t1, t2, t3) = try XCTUnwrap(messages.last) else {
            return XCTFail("oczekiwano ponga")
        }
        XCTAssertEqual(id, 5)
        XCTAssertEqual(t1, 700.0, accuracy: 1e-9)
        XCTAssertEqual(t2, 700.31, accuracy: 1e-9)
        XCTAssertGreaterThanOrEqual(t3, t2)
    }

    func testGapIsRecordedWhenLinkDrops() {
        session.start(now: 1000)
        session.linkDidChange(connected: false, at: 1100)
        session.linkDidChange(connected: true, at: 1160)

        XCTAssertEqual(session.gaps.count, 1)
        XCTAssertEqual(session.gaps[0].startHostTime, 1100, accuracy: 1e-9)
        XCTAssertEqual(session.gaps[0].endHostTime, 1160, accuracy: 1e-9)
        XCTAssertEqual(session.gaps[0].reason, "link-lost")
    }

    func testGapStaysOpenWhileDisconnected() {
        session.start(now: 1000)
        session.linkDidChange(connected: false, at: 1100)

        XCTAssertEqual(session.gaps.count, 1)
        XCTAssertNil(session.gaps[0].endHostTime)
    }

    func testNoPingsAreSentWhileDisconnected() throws {
        session.start(now: 1000)
        session.linkDidChange(connected: false, at: 1000)
        transport.clearSent()

        session.tick(now: 1005)

        XCTAssertEqual(transport.sent.count, 0)
    }

    func testSnapshotCarriesModelSamplesAndGaps() throws {
        session.start(now: 1000)
        try replyToPings(offset: 0.4)
        session.linkDidChange(connected: false, at: 1100)
        session.linkDidChange(connected: true, at: 1120)

        let snapshot = session.snapshot()
        XCTAssertEqual(snapshot.model.offsetSeconds, 0.4, accuracy: 0.001)
        XCTAssertEqual(snapshot.samples.count, 50)
        XCTAssertEqual(snapshot.gaps.count, 1)
    }
}
```

Dodaj do `FakeTransport` metodę czyszczącą (potrzebna powyżej):

```swift
    func clearSent() { sent.removeAll() }
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter SyncSessionTests`
Expected: FAIL, `cannot find 'SyncSession' in scope`

- [ ] **Step 3: Napisz implementację**

Plik `Sources/RigCore/Clock/SyncSession.swift`:

```swift
import Foundation

/// Luka w ciaglosci synchronizacji. `endHostTime` jest `nil`, dopoki
/// luka trwa.
public struct SyncGap: Equatable, Sendable {
    public var startHostTime: Double
    public var endHostTime: Double?
    public var reason: String

    public init(startHostTime: Double, endHostTime: Double? = nil, reason: String) {
        self.startHostTime = startHostTime
        self.endHostTime = endHostTime
        self.reason = reason
    }
}

/// Prowadzi synchronizacje zegarow: wysyla pingi, odpowiada na cudze,
/// laczy odpowiedzi w probki i utrzymuje aktualny model.
///
/// Klasa nie odczytuje zegara samodzielnie — czas jest podawany z zewnatrz.
/// Dzieki temu testy sterują uplywem czasu bez czekania, a produkcja
/// wstrzykuje zegar monotoniczny.
public final class SyncSession {

    public struct Snapshot {
        public let model: ClockFit
        public let samples: [ClockSample]
        public let gaps: [SyncGap]
    }

    /// Liczba pingow w serii startowej.
    public var burstCount = 50
    /// Odstep miedzy pingami w stanie ustalonym.
    public var steadyPeriod: Double = 1.0

    public private(set) var gaps: [SyncGap] = []
    public private(set) var samples: [ClockSample] = []
    public private(set) var currentFit: ClockFit?

    private let link: PeerLink
    private var estimator = ClockSyncEstimator()
    private var pending: [UInt64: Double] = [:]
    private var nextPingId: UInt64 = 0
    private var lastPingTime: Double = -.infinity
    private var connected = true

    public init(link: PeerLink) {
        self.link = link
    }

    /// Seria startowa. Wysylana jednym ciagiem — na lokalnej sieci
    /// zajmuje ulamek sekundy, a od razu daje uzyteczny model.
    public func start(now: Double) {
        for _ in 0..<burstCount {
            sendPing(now: now)
        }
        lastPingTime = now
    }

    public func tick(now: Double) {
        guard connected else { return }
        guard now - lastPingTime >= steadyPeriod else { return }
        sendPing(now: now)
        lastPingTime = now
    }

    public func linkDidChange(connected isConnected: Bool, at time: Double) {
        guard isConnected != connected else { return }
        connected = isConnected
        if isConnected {
            if let index = gaps.indices.last, gaps[index].endHostTime == nil {
                gaps[index].endHostTime = time
            }
            lastPingTime = -.infinity
        } else {
            gaps.append(SyncGap(startHostTime: time, reason: "link-lost"))
            pending.removeAll()
        }
    }

    public func handle(_ message: LinkMessage, receivedAt: Double) {
        switch message {
        case let .ping(id, t1):
            // Odpowiadamy natychmiast; t2 i t3 roznia sie o czas obslugi.
            try? link.send(.pong(id: id, t1: t1, t2: receivedAt, t3: receivedAt))

        case let .pong(id, t1, t2, t3):
            guard pending.removeValue(forKey: id) != nil else { return }
            let sample = ClockSample(t1: t1, t2: t2, t3: t3, t4: receivedAt)
            samples.append(sample)
            estimator.add(sample)
            currentFit = estimator.fit()

        default:
            break
        }
    }

    public func snapshot() -> Snapshot {
        Snapshot(
            model: currentFit ?? ClockFit(
                referenceTime: 0, offsetSeconds: 0, skewPpm: 0,
                residualStdMs: .infinity, sampleCount: 0
            ),
            samples: samples,
            gaps: gaps
        )
    }

    private func sendPing(now: Double) {
        let id = nextPingId
        nextPingId += 1
        pending[id] = now
        try? link.send(.ping(id: id, t1: now))
    }
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter SyncSessionTests`
Expected: PASS, 9 testów

- [ ] **Step 5: Commit**

```bash
git add Sources/RigCore/Clock/SyncSession.swift Tests/RigCoreTests/SyncSessionTests.swift Tests/RigCoreTests/Support/FakeTransport.swift
git commit -m "feat(clock): sesja synchronizacji z harmonogramem pingow i lukami"
```

---

### Task 7: Manifest sesji

**Files:**
- Create: `Sources/RigCore/Session/SessionManifest.swift`
- Test: `Tests/RigCoreTests/SessionManifestTests.swift`

**Interfaces:**
- Consumes: `SessionRole`, `CaptureProfile`, `SyncGap`
- Produces: `struct SessionManifest` oraz zagnieżdżone `DeviceInfo`, `CameraInfo`, `AudioInfo`, `LockedCameraSettings`, `WhiteBalanceGains`, `TimingInfo`, `SyncModel`, `SyncSampleRecord`, `SyncGapRecord`, `SegmentInfo`, `SessionEvent`; metody `func encoded() throws -> Data`, `static func decode(_:) throws -> SessionManifest`, `func validate() throws`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/SessionManifestTests.swift`:

```swift
import XCTest
@testable import RigCore

final class SessionManifestTests: XCTestCase {

    private func sampleManifest() -> SessionManifest {
        SessionManifest(
            schemaVersion: 1,
            sessionId: "2026-09-14T17:32:10Z-a3f9",
            role: .master,
            peerDeviceModel: "iPhone15,4",
            device: .init(model: "iPhone17,2", osVersion: "26.0", appVersion: "0.1.0"),
            camera: .init(
                lens: "builtInWideAngleCamera",
                profile: .p1080_120,
                codec: "hevc",
                targetBitrateMbps: 40,
                audio: .init(sampleRateHz: 48000, channels: 1, codec: "pcm"),
                locked: .init(
                    exposureDurationSeconds: 0.001,
                    iso: 64,
                    focusLensPosition: 0.82,
                    whiteBalanceGains: .init(r: 1.9, g: 1.0, b: 1.6)
                ),
                intrinsicMatrix: [[1580, 0, 960], [0, 1580, 540], [0, 0, 1]]
            ),
            timing: .init(
                clockDomain: "mach_absolute_time",
                firstFrameHostTime: 123456.789012,
                syncModel: .init(
                    referenceHostTime: 123456,
                    offsetSeconds: 0.0142,
                    skewPpm: 7.3,
                    residualStdMs: 0.9
                ),
                syncSamples: [.init(hostTime: 123456, offsetSeconds: 0.0142, delaySeconds: 0.0031)],
                syncGaps: [.init(startHostTime: 124000, endHostTime: 124035, reason: "link-lost")]
            ),
            segments: [
                .init(file: "video-000.mov", startHostTime: 123456.789, endHostTime: 123756.789, frameCount: 36000)
            ],
            events: [
                .init(hostTime: 123500.1, type: "motion-spike", magnitude: 0.42, reason: nil, state: nil)
            ],
            motionLog: "motion.jsonl"
        )
    }

    func testJSONRoundTrip() throws {
        let original = sampleManifest()
        let decoded = try SessionManifest.decode(original.encoded())
        XCTAssertEqual(decoded, original)
    }

    func testEncodedJSONUsesSpecKeys() throws {
        let json = try XCTUnwrap(String(data: sampleManifest().encoded(), encoding: .utf8))
        for key in ["schemaVersion", "sessionId", "peerDeviceModel", "intrinsicMatrix",
                    "firstFrameHostTime", "syncModel", "skewPpm", "residualStdMs",
                    "syncGaps", "targetBitrateMbps", "motionLog"] {
            XCTAssertTrue(json.contains("\"\(key)\""), "brak klucza \(key)")
        }
    }

    func testProfileIsEncodedAsSpecString() throws {
        let json = try XCTUnwrap(String(data: sampleManifest().encoded(), encoding: .utf8))
        XCTAssertTrue(json.contains("\"1080p120\""))
    }

    func testValidateAcceptsWellFormedManifest() throws {
        XCTAssertNoThrow(try sampleManifest().validate())
    }

    func testValidateRejectsWrongSchemaVersion() {
        var manifest = sampleManifest()
        manifest.schemaVersion = 2
        XCTAssertThrowsError(try manifest.validate())
    }

    func testValidateRejectsEmptySegments() {
        var manifest = sampleManifest()
        manifest.segments = []
        XCTAssertThrowsError(try manifest.validate())
    }

    func testValidateRejectsNonContiguousSegments() {
        var manifest = sampleManifest()
        manifest.segments = [
            .init(file: "video-000.mov", startHostTime: 100, endHostTime: 400, frameCount: 36000),
            .init(file: "video-001.mov", startHostTime: 420, endHostTime: 720, frameCount: 36000)
        ]
        XCTAssertThrowsError(try manifest.validate())
    }

    func testValidateRejectsFirstFrameOutsideFirstSegment() {
        var manifest = sampleManifest()
        manifest.timing.firstFrameHostTime = 999999
        XCTAssertThrowsError(try manifest.validate())
    }

    func testValidateRejectsNonThreeByThreeIntrinsics() {
        var manifest = sampleManifest()
        manifest.camera.intrinsicMatrix = [[1, 0], [0, 1]]
        XCTAssertThrowsError(try manifest.validate())
    }

    func testValidateRejectsNonFiniteResidual() {
        var manifest = sampleManifest()
        manifest.timing.syncModel.residualStdMs = .infinity
        XCTAssertThrowsError(try manifest.validate())
    }
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter SessionManifestTests`
Expected: FAIL, `cannot find 'SessionManifest' in scope`

- [ ] **Step 3: Napisz implementację**

Plik `Sources/RigCore/Session/SessionManifest.swift`:

```swift
import Foundation

/// Sidecar opisujacy sesje nagraniowa. To wlasciwy produkt P0 —
/// wideo bez tego pliku jest w P1 bezuzyteczne.
///
/// Wszystkie znaczniki czasu sa w sekundach, w monotonicznej domenie
/// zegara lokalnego urzadzenia (`timing.clockDomain`).
public struct SessionManifest: Codable, Equatable, Sendable {

    public struct DeviceInfo: Codable, Equatable, Sendable {
        public var model: String
        public var osVersion: String
        public var appVersion: String

        public init(model: String, osVersion: String, appVersion: String) {
            self.model = model
            self.osVersion = osVersion
            self.appVersion = appVersion
        }
    }

    public struct WhiteBalanceGains: Codable, Equatable, Sendable {
        public var r: Double
        public var g: Double
        public var b: Double

        public init(r: Double, g: Double, b: Double) {
            self.r = r; self.g = g; self.b = b
        }
    }

    public struct LockedCameraSettings: Codable, Equatable, Sendable {
        public var exposureDurationSeconds: Double
        public var iso: Double
        public var focusLensPosition: Double
        public var whiteBalanceGains: WhiteBalanceGains

        public init(
            exposureDurationSeconds: Double,
            iso: Double,
            focusLensPosition: Double,
            whiteBalanceGains: WhiteBalanceGains
        ) {
            self.exposureDurationSeconds = exposureDurationSeconds
            self.iso = iso
            self.focusLensPosition = focusLensPosition
            self.whiteBalanceGains = whiteBalanceGains
        }
    }

    public struct AudioInfo: Codable, Equatable, Sendable {
        public var sampleRateHz: Int
        public var channels: Int
        public var codec: String

        public init(sampleRateHz: Int, channels: Int, codec: String) {
            self.sampleRateHz = sampleRateHz
            self.channels = channels
            self.codec = codec
        }
    }

    public struct CameraInfo: Codable, Equatable, Sendable {
        public var lens: String
        public var profile: CaptureProfile
        public var width: Int
        public var height: Int
        public var fps: Int
        public var codec: String
        public var targetBitrateMbps: Int
        public var audio: AudioInfo
        public var locked: LockedCameraSettings
        public var intrinsicMatrix: [[Double]]

        /// Szerokosc, wysokosc i fps sa wyprowadzane z profilu, zeby
        /// manifest nie mogl sam sobie zaprzeczyc.
        public init(
            lens: String,
            profile: CaptureProfile,
            codec: String,
            targetBitrateMbps: Int,
            audio: AudioInfo,
            locked: LockedCameraSettings,
            intrinsicMatrix: [[Double]]
        ) {
            self.lens = lens
            self.profile = profile
            self.width = profile.width
            self.height = profile.height
            self.fps = profile.fps
            self.codec = codec
            self.targetBitrateMbps = targetBitrateMbps
            self.audio = audio
            self.locked = locked
            self.intrinsicMatrix = intrinsicMatrix
        }
    }

    public struct SyncModel: Codable, Equatable, Sendable {
        public var referenceHostTime: Double
        public var offsetSeconds: Double
        public var skewPpm: Double
        public var residualStdMs: Double

        public init(referenceHostTime: Double, offsetSeconds: Double, skewPpm: Double, residualStdMs: Double) {
            self.referenceHostTime = referenceHostTime
            self.offsetSeconds = offsetSeconds
            self.skewPpm = skewPpm
            self.residualStdMs = residualStdMs
        }
    }

    public struct SyncSampleRecord: Codable, Equatable, Sendable {
        public var hostTime: Double
        public var offsetSeconds: Double
        public var delaySeconds: Double

        public init(hostTime: Double, offsetSeconds: Double, delaySeconds: Double) {
            self.hostTime = hostTime
            self.offsetSeconds = offsetSeconds
            self.delaySeconds = delaySeconds
        }
    }

    public struct SyncGapRecord: Codable, Equatable, Sendable {
        public var startHostTime: Double
        public var endHostTime: Double?
        public var reason: String

        public init(startHostTime: Double, endHostTime: Double?, reason: String) {
            self.startHostTime = startHostTime
            self.endHostTime = endHostTime
            self.reason = reason
        }
    }

    public struct TimingInfo: Codable, Equatable, Sendable {
        public var clockDomain: String
        public var firstFrameHostTime: Double
        public var syncModel: SyncModel
        public var syncSamples: [SyncSampleRecord]
        public var syncGaps: [SyncGapRecord]

        public init(
            clockDomain: String,
            firstFrameHostTime: Double,
            syncModel: SyncModel,
            syncSamples: [SyncSampleRecord],
            syncGaps: [SyncGapRecord]
        ) {
            self.clockDomain = clockDomain
            self.firstFrameHostTime = firstFrameHostTime
            self.syncModel = syncModel
            self.syncSamples = syncSamples
            self.syncGaps = syncGaps
        }
    }

    public struct SegmentInfo: Codable, Equatable, Sendable {
        public var file: String
        public var startHostTime: Double
        public var endHostTime: Double
        public var frameCount: Int

        public init(file: String, startHostTime: Double, endHostTime: Double, frameCount: Int) {
            self.file = file
            self.startHostTime = startHostTime
            self.endHostTime = endHostTime
            self.frameCount = frameCount
        }
    }

    /// Zdarzenie sesji. Pola opcjonalne odpowiadaja roznym typom zdarzen:
    /// `magnitude` dla `motion-spike`, `reason` dla `capture-interrupted`,
    /// `state` dla `thermal`.
    public struct SessionEvent: Codable, Equatable, Sendable {
        public var hostTime: Double
        public var type: String
        public var magnitude: Double?
        public var reason: String?
        public var state: String?

        public init(
            hostTime: Double,
            type: String,
            magnitude: Double? = nil,
            reason: String? = nil,
            state: String? = nil
        ) {
            self.hostTime = hostTime
            self.type = type
            self.magnitude = magnitude
            self.reason = reason
            self.state = state
        }
    }

    public enum ValidationError: Error, Equatable {
        case unsupportedSchemaVersion(Int)
        case noSegments
        case segmentsNotContiguous(index: Int)
        case firstFrameOutsideFirstSegment
        case malformedIntrinsicMatrix
        case nonFiniteSyncModel
    }

    public var schemaVersion: Int
    public var sessionId: String
    public var role: SessionRole
    public var peerDeviceModel: String?
    public var device: DeviceInfo
    public var camera: CameraInfo
    public var timing: TimingInfo
    public var segments: [SegmentInfo]
    public var events: [SessionEvent]
    public var motionLog: String

    public init(
        schemaVersion: Int = 1,
        sessionId: String,
        role: SessionRole,
        peerDeviceModel: String?,
        device: DeviceInfo,
        camera: CameraInfo,
        timing: TimingInfo,
        segments: [SegmentInfo],
        events: [SessionEvent],
        motionLog: String
    ) {
        self.schemaVersion = schemaVersion
        self.sessionId = sessionId
        self.role = role
        self.peerDeviceModel = peerDeviceModel
        self.device = device
        self.camera = camera
        self.timing = timing
        self.segments = segments
        self.events = events
        self.motionLog = motionLog
    }

    public func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }

    public static func decode(_ data: Data) throws -> SessionManifest {
        try JSONDecoder().decode(SessionManifest.self, from: data)
    }

    /// Dopuszczalna przerwa miedzy segmentami: jedna klatka.
    public func validate() throws {
        guard schemaVersion == 1 else {
            throw ValidationError.unsupportedSchemaVersion(schemaVersion)
        }
        guard let first = segments.first else {
            throw ValidationError.noSegments
        }
        let tolerance = 1.0 / Double(camera.fps)
        for index in 1..<max(segments.count, 1) {
            let previous = segments[index - 1]
            let current = segments[index]
            if current.startHostTime - previous.endHostTime > tolerance {
                throw ValidationError.segmentsNotContiguous(index: index)
            }
        }
        guard timing.firstFrameHostTime >= first.startHostTime - tolerance,
              timing.firstFrameHostTime <= first.endHostTime else {
            throw ValidationError.firstFrameOutsideFirstSegment
        }
        guard camera.intrinsicMatrix.count == 3,
              camera.intrinsicMatrix.allSatisfy({ $0.count == 3 }) else {
            throw ValidationError.malformedIntrinsicMatrix
        }
        let model = timing.syncModel
        guard model.offsetSeconds.isFinite, model.skewPpm.isFinite, model.residualStdMs.isFinite else {
            throw ValidationError.nonFiniteSyncModel
        }
    }
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter SessionManifestTests`
Expected: PASS, 10 testów

- [ ] **Step 5: Commit**

```bash
git add Sources/RigCore/Session/SessionManifest.swift Tests/RigCoreTests/SessionManifestTests.swift
git commit -m "feat(session): model manifestu sesji z walidacja"
```

---

### Task 8: Detekcja szarpnięć kamery

**Files:**
- Create: `Sources/RigCore/Session/MotionAnalyzer.swift`
- Test: `Tests/RigCoreTests/MotionAnalyzerTests.swift`

**Interfaces:**
- Consumes: `SessionManifest.SessionEvent` z Task 7
- Produces: `struct MotionSample`, `struct MotionAnalyzer` z `mutating func process(_:) -> SessionManifest.SessionEvent?`

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/MotionAnalyzerTests.swift`:

```swift
import XCTest
@testable import RigCore

final class MotionAnalyzerTests: XCTestCase {

    private func calmSample(at time: Double) -> MotionSample {
        MotionSample(hostTime: time, rotationRate: .init(x: 0.01, y: 0.005, z: 0.002))
    }

    private func spikeSample(at time: Double, magnitude: Double = 1.2) -> MotionSample {
        MotionSample(hostTime: time, rotationRate: .init(x: magnitude, y: 0, z: 0))
    }

    func testCalmMotionProducesNoEvent() {
        var analyzer = MotionAnalyzer()
        for step in 0..<100 {
            XCTAssertNil(analyzer.process(calmSample(at: Double(step) * 0.01)))
        }
    }

    func testSpikeProducesEventWithMagnitude() {
        var analyzer = MotionAnalyzer()
        let event = analyzer.process(spikeSample(at: 5.0, magnitude: 1.2))

        let unwrapped = try! XCTUnwrap(event)
        XCTAssertEqual(unwrapped.type, "motion-spike")
        XCTAssertEqual(unwrapped.hostTime, 5.0, accuracy: 1e-9)
        XCTAssertEqual(try! XCTUnwrap(unwrapped.magnitude), 1.2, accuracy: 1e-6)
    }

    func testRefractoryPeriodSuppressesRepeatedSpikes() {
        var analyzer = MotionAnalyzer()
        analyzer.refractorySeconds = 1.0

        XCTAssertNotNil(analyzer.process(spikeSample(at: 5.0)))
        XCTAssertNil(analyzer.process(spikeSample(at: 5.2)))
        XCTAssertNil(analyzer.process(spikeSample(at: 5.9)))
        XCTAssertNotNil(analyzer.process(spikeSample(at: 6.1)))
    }

    func testThresholdIsConfigurable() {
        var analyzer = MotionAnalyzer()
        analyzer.rotationThreshold = 2.0

        XCTAssertNil(analyzer.process(spikeSample(at: 1.0, magnitude: 1.5)))
        XCTAssertNotNil(analyzer.process(spikeSample(at: 2.0, magnitude: 2.5)))
    }

    func testMagnitudeUsesVectorNorm() {
        var analyzer = MotionAnalyzer()
        analyzer.rotationThreshold = 0.35

        // norma (0.3, 0.4, 0) = 0.5
        let event = analyzer.process(
            MotionSample(hostTime: 1.0, rotationRate: .init(x: 0.3, y: 0.4, z: 0.0))
        )
        XCTAssertEqual(try! XCTUnwrap(event?.magnitude), 0.5, accuracy: 1e-9)
    }
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter MotionAnalyzerTests`
Expected: FAIL, `cannot find 'MotionAnalyzer' in scope`

- [ ] **Step 3: Napisz implementację**

Plik `Sources/RigCore/Session/MotionAnalyzer.swift`:

```swift
import Foundation

public struct MotionVector: Equatable, Sendable {
    public var x: Double
    public var y: Double
    public var z: Double

    public init(x: Double, y: Double, z: Double) {
        self.x = x; self.y = y; self.z = z
    }

    public var norm: Double { (x * x + y * y + z * z).squareRoot() }
}

public struct MotionSample: Equatable, Sendable {
    public var hostTime: Double
    /// Predkosc katowa w rad/s.
    public var rotationRate: MotionVector

    public init(hostTime: Double, rotationRate: MotionVector) {
        self.hostTime = hostTime
        self.rotationRate = rotationRate
    }
}

/// Wykrywa szarpniecia telefonu zawieszonego na ogrodzeniu.
///
/// Znaczenie tych zdarzen jest w calosci po stronie P1: sa markerem
/// "tutaj homografia mogla przestac pasowac, przelicz ja od nowa".
/// Dlatego liczy sie czas i skala, a nie klasyfikacja przyczyny.
public struct MotionAnalyzer: Sendable {

    /// Prog predkosci katowej w rad/s.
    public var rotationThreshold: Double = 0.35
    /// Po wykryciu skoku ignorujemy kolejne przez ten czas, zeby jedno
    /// szarpniecie nie zamienilo sie w setke zdarzen.
    public var refractorySeconds: Double = 1.0

    private var lastSpikeTime: Double = -.infinity

    public init() {}

    public mutating func process(_ sample: MotionSample) -> SessionManifest.SessionEvent? {
        let magnitude = sample.rotationRate.norm
        guard magnitude >= rotationThreshold else { return nil }
        guard sample.hostTime - lastSpikeTime >= refractorySeconds else { return nil }

        lastSpikeTime = sample.hostTime
        return SessionManifest.SessionEvent(
            hostTime: sample.hostTime,
            type: "motion-spike",
            magnitude: magnitude
        )
    }
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter MotionAnalyzerTests`
Expected: PASS, 5 testów

- [ ] **Step 5: Commit**

```bash
git add Sources/RigCore/Session/MotionAnalyzer.swift Tests/RigCoreTests/MotionAnalyzerTests.swift
git commit -m "feat(session): detekcja szarpniec kamery z okresem refrakcji"
```

---

### Task 9: Maszyna stanów sesji nagraniowej

Serce obsługi awarii ze specyfikacji, sekcja 8.

**Files:**
- Create: `Sources/RigCore/Session/RecorderPorts.swift`
- Create: `Sources/RigCore/Session/SessionRecorder.swift`
- Test: `Tests/RigCoreTests/Support/FakeRecorderPorts.swift`
- Test: `Tests/RigCoreTests/SessionRecorderTests.swift`

**Interfaces:**
- Consumes: `SessionManifest` i typy zagnieżdżone (Task 7), `CaptureProfile`, `SessionRole`, `SyncSession.Snapshot` (Task 6)
- Produces: `protocol CaptureControlling`, `protocol StorageProbing`, `protocol ThermalProbing`, `protocol SyncSnapshotProviding`, `enum ThermalLevel`, `struct RecorderConfig`, `enum RecorderState`, `final class SessionRecorder` z `arm()`, `start(sessionId:profile:now:)`, `tick(now:)`, `noteEvent(_:)`, `notePeerHeartbeat(at:)`, `stop(now:)`

- [ ] **Step 1: Napisz porty**

Plik `Sources/RigCore/Session/RecorderPorts.swift`:

```swift
import Foundation

public enum ThermalLevel: Int, Comparable, Sendable {
    case nominal = 0
    case fair = 1
    case serious = 2
    case critical = 3

    public static func < (lhs: ThermalLevel, rhs: ThermalLevel) -> Bool {
        lhs.rawValue < rhs.rawValue
    }
}

/// Sterowanie kamera. Implementacja produkcyjna opakowuje AVFoundation.
public protocol CaptureControlling: AnyObject {
    func lockSettings() throws -> SessionManifest.LockedCameraSettings
    func startRecording(sessionId: String, profile: CaptureProfile) throws
    /// Zamyka biezacy segment i otwiera nastepny. Zwraca zamkniety segment.
    func rollSegment(now: Double) throws -> SessionManifest.SegmentInfo
    /// Zamyka biezacy segment i konczy nagrywanie.
    func stopRecording(now: Double) throws -> SessionManifest.SegmentInfo
    var firstFrameHostTime: Double? { get }
    var intrinsicMatrix: [[Double]]? { get }
    var lensName: String { get }
}

public protocol StorageProbing: AnyObject {
    var freeBytes: Int64 { get }
}

public protocol ThermalProbing: AnyObject {
    var thermalLevel: ThermalLevel { get }
}

public protocol SyncSnapshotProviding: AnyObject {
    func snapshot() -> SyncSession.Snapshot
}

/// `SyncSession` ma juz metode o tej sygnaturze, wiec zgodnosc jest pusta.
/// Deklaracja jest jednak konieczna: bez niej aplikacja nie moze podac
/// sesji synchronizacji do `SessionRecorder`.
extension SyncSession: SyncSnapshotProviding {}

public struct RecorderConfig: Sendable {
    public var segmentSeconds: Double = 300
    /// Ponizej tej rezerwy konczymy sesje w kontrolowany sposob.
    public var minimumFreeBytes: Int64 = 500_000_000
    /// Cisza peera dluzsza niz to konczy sesje po stronie slave'a.
    public var peerSilenceTimeout: Double = 120
    public var targetBitrateMbps: Int = 40
    public var videoCodec: String = "hevc"

    public init() {}
}

public enum RecorderState: Equatable, Sendable {
    case idle
    case armed
    case recording
    case finalizing
    case stopped(reason: StopReason)
}

public enum StopReason: String, Equatable, Sendable {
    case userRequested = "user-requested"
    case storageExhausted = "storage-exhausted"
    case thermalCritical = "thermal-critical"
    case peerSilence = "peer-silence"
}
```

- [ ] **Step 2: Napisz atrapy portów**

Plik `Tests/RigCoreTests/Support/FakeRecorderPorts.swift`:

```swift
import Foundation
@testable import RigCore

final class FakeCapture: CaptureControlling {
    var lensName = "builtInWideAngleCamera"
    var firstFrameHostTime: Double? = 1000.05
    var intrinsicMatrix: [[Double]]? = [[1580, 0, 960], [0, 1580, 540], [0, 0, 1]]

    private(set) var didLock = false
    private(set) var didStart = false
    private(set) var rollCount = 0
    private(set) var didStop = false

    var lockShouldThrow = false
    private var segmentStart: Double = 0
    private var segmentIndex = 0

    struct LockFailure: Error {}

    func lockSettings() throws -> SessionManifest.LockedCameraSettings {
        if lockShouldThrow { throw LockFailure() }
        didLock = true
        return .init(
            exposureDurationSeconds: 0.001,
            iso: 64,
            focusLensPosition: 0.82,
            whiteBalanceGains: .init(r: 1.9, g: 1.0, b: 1.6)
        )
    }

    func startRecording(sessionId: String, profile: CaptureProfile) throws {
        didStart = true
        segmentStart = firstFrameHostTime ?? 0
    }

    func rollSegment(now: Double) throws -> SessionManifest.SegmentInfo {
        rollCount += 1
        let info = segment(endingAt: now)
        segmentStart = now
        return info
    }

    func stopRecording(now: Double) throws -> SessionManifest.SegmentInfo {
        didStop = true
        return segment(endingAt: now)
    }

    private func segment(endingAt now: Double) -> SessionManifest.SegmentInfo {
        let info = SessionManifest.SegmentInfo(
            file: String(format: "video-%03d.mov", segmentIndex),
            startHostTime: segmentStart,
            endHostTime: now,
            frameCount: Int((now - segmentStart) * 120)
        )
        segmentIndex += 1
        return info
    }
}

final class FakeStorage: StorageProbing {
    var freeBytes: Int64 = 64_000_000_000
}

final class FakeThermal: ThermalProbing {
    var thermalLevel: ThermalLevel = .nominal
}

final class FakeSyncProvider: SyncSnapshotProviding {
    var fit = ClockFit(
        referenceTime: 1000, offsetSeconds: 0.014,
        skewPpm: 7.3, residualStdMs: 0.9, sampleCount: 60
    )
    var samples: [ClockSample] = [ClockSample(t1: 1000, t2: 1000.019, t3: 1000.020, t4: 1000.006)]
    var gaps: [SyncGap] = []

    func snapshot() -> SyncSession.Snapshot {
        SyncSession.Snapshot(model: fit, samples: samples, gaps: gaps)
    }
}
```

- [ ] **Step 3: Napisz testy, które mają nie przejść**

Plik `Tests/RigCoreTests/SessionRecorderTests.swift`:

```swift
import XCTest
@testable import RigCore

final class SessionRecorderTests: XCTestCase {

    private var capture: FakeCapture!
    private var storage: FakeStorage!
    private var thermal: FakeThermal!
    private var sync: FakeSyncProvider!
    private var recorder: SessionRecorder!

    override func setUp() {
        super.setUp()
        capture = FakeCapture()
        storage = FakeStorage()
        thermal = FakeThermal()
        sync = FakeSyncProvider()
        recorder = SessionRecorder(
            role: .master,
            device: .init(model: "iPhone17,2", osVersion: "26.0", appVersion: "0.1.0"),
            peerDeviceModel: "iPhone15,4",
            capture: capture,
            storage: storage,
            thermal: thermal,
            sync: sync,
            config: RecorderConfig()
        )
    }

    private func startRecording(now: Double = 1000) throws {
        try recorder.arm()
        try recorder.start(sessionId: "s-1", profile: .p1080_120, now: now)
    }

    // MARK: przejscia stanow

    func testStartsIdle() {
        XCTAssertEqual(recorder.state, .idle)
    }

    func testArmLocksCameraSettings() throws {
        try recorder.arm()
        XCTAssertEqual(recorder.state, .armed)
        XCTAssertTrue(capture.didLock)
    }

    func testStartRequiresArmedState() {
        XCTAssertThrowsError(try recorder.start(sessionId: "s-1", profile: .p1080_120, now: 1000))
    }

    func testLockFailureLeavesRecorderIdle() {
        capture.lockShouldThrow = true
        XCTAssertThrowsError(try recorder.arm())
        XCTAssertEqual(recorder.state, .idle)
    }

    func testStartMovesToRecording() throws {
        try startRecording()
        XCTAssertEqual(recorder.state, .recording)
        XCTAssertTrue(capture.didStart)
    }

    // MARK: segmentacja

    func testSegmentRollsAfterConfiguredDuration() throws {
        try startRecording(now: 1000)

        recorder.tick(now: 1200)
        XCTAssertEqual(capture.rollCount, 0)

        recorder.tick(now: 1300.1)
        XCTAssertEqual(capture.rollCount, 1)

        recorder.tick(now: 1600.2)
        XCTAssertEqual(capture.rollCount, 2)
    }

    // MARK: awarie

    func testCriticalThermalStopsSession() throws {
        try startRecording()
        thermal.thermalLevel = .serious
        recorder.tick(now: 1010)
        XCTAssertEqual(recorder.state, .recording)

        thermal.thermalLevel = .critical
        recorder.tick(now: 1020)
        XCTAssertEqual(recorder.state, .stopped(reason: .thermalCritical))
        XCTAssertTrue(capture.didStop)
    }

    func testSeriousThermalIsRecordedAsEvent() throws {
        try startRecording()
        thermal.thermalLevel = .serious
        recorder.tick(now: 1010)

        let manifest = try recorder.buildManifest(now: 1010)
        XCTAssertTrue(manifest.events.contains { $0.type == "thermal" && $0.state == "serious" })
    }

    func testLowStorageStopsSession() throws {
        try startRecording()
        storage.freeBytes = 100_000_000
        recorder.tick(now: 1010)

        XCTAssertEqual(recorder.state, .stopped(reason: .storageExhausted))
    }

    func testArmRejectsInsufficientStorage() {
        storage.freeBytes = 10_000_000
        XCTAssertThrowsError(try recorder.arm())
    }

    func testSlaveStopsAfterPeerSilenceTimeout() throws {
        let slave = SessionRecorder(
            role: .slave,
            device: .init(model: "iPhone15,4", osVersion: "26.0", appVersion: "0.1.0"),
            peerDeviceModel: "iPhone17,2",
            capture: capture, storage: storage, thermal: thermal, sync: sync,
            config: RecorderConfig()
        )
        try slave.arm()
        try slave.start(sessionId: "s-1", profile: .p1080_120, now: 1000)

        slave.notePeerHeartbeat(at: 1050)
        slave.tick(now: 1150)
        XCTAssertEqual(slave.state, .recording)

        slave.tick(now: 1180)
        XCTAssertEqual(slave.state, .stopped(reason: .peerSilence))
    }

    func testMasterIgnoresPeerSilence() throws {
        try startRecording(now: 1000)
        recorder.notePeerHeartbeat(at: 1000)
        recorder.tick(now: 2000)
        XCTAssertEqual(recorder.state, .recording)
    }

    func testLinkLossDoesNotStopRecording() throws {
        try startRecording()
        recorder.noteEvent(.init(hostTime: 1100, type: "link-lost"))
        recorder.tick(now: 1110)
        XCTAssertEqual(recorder.state, .recording)
    }

    // MARK: manifest

    func testStopProducesValidManifest() throws {
        try startRecording(now: 1000)
        recorder.tick(now: 1300.1)
        let manifest = try recorder.stop(now: 1450)

        XCTAssertNoThrow(try manifest.validate())
        XCTAssertEqual(manifest.sessionId, "s-1")
        XCTAssertEqual(manifest.role, .master)
        XCTAssertEqual(manifest.camera.profile, .p1080_120)
        XCTAssertEqual(manifest.camera.fps, 120)
        XCTAssertEqual(manifest.segments.count, 2)
        XCTAssertEqual(manifest.timing.clockDomain, "mach_absolute_time")
        XCTAssertEqual(manifest.motionLog, "motion.jsonl")
    }

    func testManifestCarriesSyncModelSamplesAndGaps() throws {
        sync.gaps = [SyncGap(startHostTime: 1100, endHostTime: 1160, reason: "link-lost")]
        try startRecording(now: 1000)
        let manifest = try recorder.stop(now: 1200)

        XCTAssertEqual(manifest.timing.syncModel.skewPpm, 7.3, accuracy: 1e-9)
        XCTAssertEqual(manifest.timing.syncSamples.count, 1)
        XCTAssertEqual(manifest.timing.syncGaps.count, 1)
        XCTAssertEqual(manifest.timing.syncGaps[0].reason, "link-lost")
    }

    func testStopIsIdempotentAfterAutomaticStop() throws {
        try startRecording()
        thermal.thermalLevel = .critical
        recorder.tick(now: 1010)

        let manifest = try recorder.stop(now: 1020)
        XCTAssertNoThrow(try manifest.validate())
        XCTAssertEqual(capture.rollCount, 0)
    }
}
```

- [ ] **Step 4: Uruchom testy i potwierdź, że nie przechodzą**

Run: `swift test --filter SessionRecorderTests`
Expected: FAIL, `cannot find 'SessionRecorder' in scope`

- [ ] **Step 5: Napisz implementację**

Plik `Sources/RigCore/Session/SessionRecorder.swift`:

```swift
import Foundation

/// Prowadzi sesje nagraniowa i buduje manifest.
///
/// Klasa nie odczytuje zegara ani nie uruchamia timerow — czas przychodzi
/// z zewnatrz przez `tick(now:)`. Cala obsluga awarii ze specyfikacji
/// sprowadza sie dzieki temu do testow jednostkowych.
///
/// Zasada nadrzedna: utrata lacznosci nigdy nie zatrzymuje nagrywania.
/// Zdarzenie `link-lost` jest zapisywane do manifestu, ale nie zmienia stanu.
public final class SessionRecorder {

    public enum Failure: Error, Equatable {
        case wrongState(RecorderState)
        case insufficientStorage
        case notStarted
    }

    public private(set) var state: RecorderState = .idle

    private let role: SessionRole
    private let device: SessionManifest.DeviceInfo
    private let peerDeviceModel: String?
    private let capture: CaptureControlling
    private let storage: StorageProbing
    private let thermal: ThermalProbing
    private let sync: SyncSnapshotProviding
    private let config: RecorderConfig

    private var sessionId: String?
    private var profile: CaptureProfile?
    private var locked: SessionManifest.LockedCameraSettings?
    private var segments: [SessionManifest.SegmentInfo] = []
    private var events: [SessionManifest.SessionEvent] = []
    private var segmentStarted: Double = 0
    private var lastPeerHeartbeat: Double?
    private var lastThermalLevel: ThermalLevel = .nominal

    public init(
        role: SessionRole,
        device: SessionManifest.DeviceInfo,
        peerDeviceModel: String?,
        capture: CaptureControlling,
        storage: StorageProbing,
        thermal: ThermalProbing,
        sync: SyncSnapshotProviding,
        config: RecorderConfig
    ) {
        self.role = role
        self.device = device
        self.peerDeviceModel = peerDeviceModel
        self.capture = capture
        self.storage = storage
        self.thermal = thermal
        self.sync = sync
        self.config = config
    }

    // MARK: cykl zycia

    /// Sprawdza zapas miejsca i blokuje parametry kamery.
    public func arm() throws {
        guard state == .idle else { throw Failure.wrongState(state) }
        guard storage.freeBytes >= config.minimumFreeBytes else {
            throw Failure.insufficientStorage
        }
        locked = try capture.lockSettings()
        state = .armed
    }

    public func start(sessionId: String, profile: CaptureProfile, now: Double) throws {
        guard state == .armed else { throw Failure.wrongState(state) }
        try capture.startRecording(sessionId: sessionId, profile: profile)
        self.sessionId = sessionId
        self.profile = profile
        self.segmentStarted = now
        self.state = .recording
    }

    /// Wolajac to co sekunde, oddajemy sterowanie czasem na zewnatrz.
    public func tick(now: Double) {
        guard state == .recording else { return }

        let level = thermal.thermalLevel
        if level != lastThermalLevel {
            lastThermalLevel = level
            events.append(.init(hostTime: now, type: "thermal", state: String(describing: level)))
        }
        if level == .critical {
            finish(now: now, reason: .thermalCritical)
            return
        }

        if storage.freeBytes < config.minimumFreeBytes {
            finish(now: now, reason: .storageExhausted)
            return
        }

        if role == .slave, let last = lastPeerHeartbeat,
           now - last > config.peerSilenceTimeout {
            finish(now: now, reason: .peerSilence)
            return
        }

        if now - segmentStarted >= config.segmentSeconds {
            if let segment = try? capture.rollSegment(now: now) {
                segments.append(segment)
                segmentStarted = now
            }
        }
    }

    public func noteEvent(_ event: SessionManifest.SessionEvent) {
        events.append(event)
    }

    public func notePeerHeartbeat(at time: Double) {
        lastPeerHeartbeat = time
    }

    /// Zatrzymanie na zadanie uzytkownika. Jesli sesja zostala juz
    /// zakonczona automatycznie, zwraca manifest bez ponownego dotykania kamery.
    public func stop(now: Double) throws -> SessionManifest {
        switch state {
        case .recording:
            finish(now: now, reason: .userRequested)
        case .stopped:
            break
        default:
            throw Failure.wrongState(state)
        }
        return try buildManifest(now: now)
    }

    private func finish(now: Double, reason: StopReason) {
        state = .finalizing
        if let segment = try? capture.stopRecording(now: now) {
            segments.append(segment)
        }
        events.append(.init(hostTime: now, type: "session-stopped", reason: reason.rawValue))
        state = .stopped(reason: reason)
    }

    // MARK: manifest

    public func buildManifest(now: Double) throws -> SessionManifest {
        guard let sessionId, let profile, let locked else { throw Failure.notStarted }
        let snapshot = sync.snapshot()

        return SessionManifest(
            sessionId: sessionId,
            role: role,
            peerDeviceModel: peerDeviceModel,
            device: device,
            camera: .init(
                lens: capture.lensName,
                profile: profile,
                codec: config.videoCodec,
                targetBitrateMbps: config.targetBitrateMbps,
                audio: .init(sampleRateHz: 48000, channels: 1, codec: "pcm"),
                locked: locked,
                intrinsicMatrix: capture.intrinsicMatrix ?? [[0, 0, 0], [0, 0, 0], [0, 0, 0]]
            ),
            timing: .init(
                clockDomain: "mach_absolute_time",
                firstFrameHostTime: capture.firstFrameHostTime ?? segments.first?.startHostTime ?? now,
                syncModel: .init(
                    referenceHostTime: snapshot.model.referenceTime,
                    offsetSeconds: snapshot.model.offsetSeconds,
                    skewPpm: snapshot.model.skewPpm,
                    residualStdMs: snapshot.model.residualStdMs
                ),
                syncSamples: snapshot.samples.map {
                    .init(hostTime: $0.localTime, offsetSeconds: $0.offset, delaySeconds: $0.delay)
                },
                syncGaps: snapshot.gaps.map {
                    .init(startHostTime: $0.startHostTime, endHostTime: $0.endHostTime, reason: $0.reason)
                }
            ),
            segments: segments,
            events: events,
            motionLog: "motion.jsonl"
        )
    }
}
```

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `swift test --filter SessionRecorderTests`
Expected: PASS, 16 testów

- [ ] **Step 7: Uruchom cały pakiet**

Run: `swift test`
Expected: PASS, wszystkie testy z Tasków 1-9

- [ ] **Step 8: Commit**

```bash
git add Sources/RigCore/Session/RecorderPorts.swift Sources/RigCore/Session/SessionRecorder.swift Tests/RigCoreTests/Support/FakeRecorderPorts.swift Tests/RigCoreTests/SessionRecorderTests.swift
git commit -m "feat(session): maszyna stanow sesji z obsluga awarii"
```

---

### Task 10: Aplikacja iOS, transport MultipeerConnectivity, ekran parowania

Pierwszy kamień milowy weryfikowany na urządzeniach. Od tego momentu część weryfikacji jest ręczna — jest to zaznaczone przy każdym kroku.

**Files:**
- Create: `App/TennisTrackerRig.xcodeproj` (przez Xcode)
- Create: `App/TennisTrackerRig/TennisTrackerRigApp.swift`
- Create: `App/TennisTrackerRig/Adapters/MonotonicClock.swift`
- Create: `App/TennisTrackerRig/Adapters/MultipeerTransport.swift`
- Create: `App/TennisTrackerRig/Session/SessionCoordinator.swift`
- Create: `App/TennisTrackerRig/UI/RigView.swift`
- Modify: `App/TennisTrackerRig/Info.plist`

**Interfaces:**
- Consumes: `PeerLink`, `PeerTransport`, `SyncSession`, `SessionRole` z RigCore
- Produces: `final class MonotonicClock` z `var now: Double`; `final class MultipeerTransport: PeerTransport` z `init(role:serviceType:)` i `func start()`; `final class SessionCoordinator: ObservableObject` z `@Published var isConnected`, `@Published var syncQualityMs`, `@Published var role`

- [ ] **Step 1: Utwórz projekt Xcode**

W Xcode: File → New → Project → iOS → App.
- Product Name: `TennisTrackerRig`
- Interface: SwiftUI, Language: Swift
- Zapisz w katalogu `App/` repozytorium.

Następnie: File → Add Package Dependencies → Add Local → wskaż katalog główny repozytorium (pakiet `RigCore`). Dodaj bibliotekę `RigCore` do targetu aplikacji.

W ustawieniach targetu: Minimum Deployment iOS 17.0. Signing → Team → twój darmowy profil, Bundle Identifier ustaw na własny (np. `com.<twoje-id>.tennistrackerrig`).

- [ ] **Step 2: Uzupełnij `Info.plist`**

Dodaj klucze (Xcode → target → Info):

```xml
<key>NSCameraUsageDescription</key>
<string>Nagrywanie meczu z kamery zawieszonej na ogrodzeniu kortu.</string>
<key>NSMicrophoneUsageDescription</key>
<string>Nagrywanie dzwieku uderzen, uzywanego do weryfikacji synchronizacji.</string>
<key>NSMotionUsageDescription</key>
<string>Wykrywanie szarpniec telefonu zawieszonego na ogrodzeniu.</string>
<key>NSLocalNetworkUsageDescription</key>
<string>Polaczenie z drugim telefonem nagrywajacym mecz.</string>
<key>NSBonjourServices</key>
<array>
    <string>_ttrig._tcp</string>
    <string>_ttrig._udp</string>
</array>
```

Bez `NSBonjourServices` MultipeerConnectivity na iOS 14+ nie wykryje peera. To najczęstsza przyczyna „nie widzą się nawzajem".

- [ ] **Step 3: Napisz zegar monotoniczny**

Plik `App/TennisTrackerRig/Adapters/MonotonicClock.swift`:

```swift
import Foundation
import QuartzCore

/// Jedyne zrodlo czasu w aplikacji.
///
/// `CACurrentMediaTime()` opiera sie na `mach_absolute_time`, czyli na tej
/// samej podstawie co `CMSampleBuffer.presentationTimeStamp` z kamery
/// (`CMClockGetHostTimeClock()`). Znaczniki klatek sa wiec bezposrednio
/// porownywalne ze znacznikami synchronizacji, bez zadnej konwersji.
///
/// Czasu sciennego nie uzywamy nigdzie: systemowa korekta NTP przesunelaby
/// os czasu w srodku nagrania.
final class MonotonicClock {
    var now: Double { CACurrentMediaTime() }
}
```

- [ ] **Step 4: Napisz transport MultipeerConnectivity**

Plik `App/TennisTrackerRig/Adapters/MultipeerTransport.swift`:

```swift
import Foundation
import MultipeerConnectivity
import UIKit
import RigCore

/// Transport peer-to-peer po WiFi i Bluetooth, bez internetu i bez serwera.
///
/// Master oglasza usluge, slave jej szuka. Podzial jest arbitralny —
/// chodzi tylko o to, zeby obie strony nie probowaly zapraszac sie nawzajem.
final class MultipeerTransport: NSObject, PeerTransport {

    var onReceive: ((Data) -> Void)?
    var onConnectionChange: ((Bool) -> Void)?

    private(set) var isConnected = false

    private let serviceType = "ttrig"
    private let peerId = MCPeerID(displayName: UIDevice.current.name)
    private let role: SessionRole
    private lazy var session: MCSession = {
        let session = MCSession(peer: peerId, securityIdentity: nil, encryptionPreference: .none)
        session.delegate = self
        return session
    }()
    private var advertiser: MCNearbyServiceAdvertiser?
    private var browser: MCNearbyServiceBrowser?

    init(role: SessionRole) {
        self.role = role
        super.init()
    }

    func start() {
        stop()
        switch role {
        case .master:
            let advertiser = MCNearbyServiceAdvertiser(
                peer: peerId, discoveryInfo: nil, serviceType: serviceType
            )
            advertiser.delegate = self
            advertiser.startAdvertisingPeer()
            self.advertiser = advertiser
        case .slave:
            let browser = MCNearbyServiceBrowser(peer: peerId, serviceType: serviceType)
            browser.delegate = self
            browser.startBrowsingForPeers()
            self.browser = browser
        }
    }

    func stop() {
        advertiser?.stopAdvertisingPeer()
        browser?.stopBrowsingForPeers()
        advertiser = nil
        browser = nil
    }

    func send(_ data: Data) throws {
        guard !session.connectedPeers.isEmpty else { throw PeerLink.Failure.notConnected }
        try session.send(data, toPeers: session.connectedPeers, with: .reliable)
    }
}

extension MultipeerTransport: MCSessionDelegate {
    func session(_ session: MCSession, peer: MCPeerID, didChange state: MCSessionState) {
        let connected = state == .connected
        DispatchQueue.main.async {
            self.isConnected = connected
            self.onConnectionChange?(connected)
        }
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        DispatchQueue.main.async { self.onReceive?(data) }
    }

    func session(_ s: MCSession, didReceive stream: InputStream, withName: String, fromPeer: MCPeerID) {}
    func session(_ s: MCSession, didStartReceivingResourceWithName: String, fromPeer: MCPeerID, with: Progress) {}
    func session(_ s: MCSession, didFinishReceivingResourceWithName: String, fromPeer: MCPeerID, at: URL?, withError: Error?) {}
}

extension MultipeerTransport: MCNearbyServiceAdvertiserDelegate {
    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext context: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        invitationHandler(true, session)
    }
}

extension MultipeerTransport: MCNearbyServiceBrowserDelegate {
    func browser(_ browser: MCNearbyServiceBrowser, foundPeer peerID: MCPeerID, withDiscoveryInfo info: [String: String]?) {
        browser.invitePeer(peerID, to: session, withContext: nil, timeout: 15)
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {}
}
```

- [ ] **Step 5: Napisz koordynator**

Plik `App/TennisTrackerRig/Session/SessionCoordinator.swift`:

```swift
import AVFoundation
import Combine
import Foundation
import UIKit
import RigCore

/// Spina RigCore z adapterami sprzetowymi i wystawia stan do UI.
@MainActor
final class SessionCoordinator: ObservableObject {

    @Published private(set) var isConnected = false
    @Published private(set) var syncQualityMs: Double?
    @Published private(set) var sampleCount = 0
    @Published var role: SessionRole = .master

    private let clock = MonotonicClock()
    private var transport: MultipeerTransport?
    private var link: PeerLink?
    private var syncSession: SyncSession?
    private var ticker: Timer?

    func connect() {
        let transport = MultipeerTransport(role: role)
        let link = PeerLink(transport: transport)
        let sync = SyncSession(link: link)

        link.onMessage = { [weak self] message in
            guard let self else { return }
            sync.handle(message, receivedAt: self.clock.now)
        }
        link.onConnectionChange = { [weak self] connected in
            guard let self else { return }
            self.isConnected = connected
            sync.linkDidChange(connected: connected, at: self.clock.now)
            if connected { sync.start(now: self.clock.now) }
        }

        self.transport = transport
        self.link = link
        self.syncSession = sync
        transport.start()
        startTicking()
    }

    private func startTicking() {
        ticker?.invalidate()
        ticker = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }

    private func tick() {
        guard let syncSession else { return }
        syncSession.tick(now: clock.now)
        syncQualityMs = syncSession.currentFit?.residualStdMs
        sampleCount = syncSession.samples.count
    }
}
```

- [ ] **Step 6: Napisz ekran główny**

Plik `App/TennisTrackerRig/UI/RigView.swift`:

```swift
import SwiftUI
import RigCore

struct RigView: View {
    @StateObject private var coordinator = SessionCoordinator()

    var body: some View {
        VStack(spacing: 24) {
            Picker("Rola", selection: $coordinator.role) {
                Text("Master").tag(SessionRole.master)
                Text("Slave").tag(SessionRole.slave)
            }
            .pickerStyle(.segmented)

            HStack {
                Circle()
                    .fill(coordinator.isConnected ? .green : .red)
                    .frame(width: 14, height: 14)
                Text(coordinator.isConnected ? "Polaczono" : "Brak polaczenia")
            }

            VStack(spacing: 4) {
                Text(syncText).font(.system(size: 40, weight: .semibold, design: .monospaced))
                    .foregroundStyle(syncColor)
                Text("\(coordinator.sampleCount) probek").font(.caption)
            }

            Button("Polacz") { coordinator.connect() }
                .buttonStyle(.borderedProminent)

            Spacer()

            Text("Apka wygasa: \(Self.expiryText)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding()
    }

    private var syncText: String {
        guard let quality = coordinator.syncQualityMs else { return "—" }
        return String(format: "±%.2f ms", quality)
    }

    /// Zielony ponizej celu 2 ms, zolty do 5 ms, czerwony powyzej.
    private var syncColor: Color {
        guard let quality = coordinator.syncQualityMs else { return .secondary }
        if quality < 2.0 { return .green }
        if quality < 5.0 { return .yellow }
        return .red
    }

    /// Darmowy provisioning wygasa po 7 dniach od zbudowania.
    private static var expiryText: String {
        let buildDate = (try? FileManager.default.attributesOfItem(
            atPath: Bundle.main.executablePath ?? ""
        )[.creationDate] as? Date) ?? nil
        guard let buildDate else { return "nieznana" }
        let expiry = buildDate.addingTimeInterval(7 * 24 * 3600)
        let formatter = DateFormatter()
        formatter.dateFormat = "d MMM, HH:mm"
        return formatter.string(from: expiry)
    }
}
```

Plik `App/TennisTrackerRig/TennisTrackerRigApp.swift`:

```swift
import SwiftUI

@main
struct TennisTrackerRigApp: App {
    var body: some Scene {
        WindowGroup {
            RigView()
        }
    }
}
```

- [ ] **Step 7: Zbuduj i sprawdź kompilację**

Run: `xcodebuild -project App/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 8: Weryfikacja ręczna na dwóch telefonach**

Zainstaluj apkę na obu iPhone'ach. Na 16 Pro Max wybierz Master, na 15 wybierz Slave, na obu naciśnij „Połącz".

Oczekiwane:
1. Kropka staje się zielona na obu urządzeniach w ciągu kilku sekund.
2. Liczba próbek rośnie i przekracza 50 niemal natychmiast (seria startowa).
3. Wartość synchronizacji ustala się i jest **zielona, poniżej 2 ms**.
4. Wyłączenie WiFi na jednym telefonie: kropka czerwienieje na obu; po włączeniu wraca zielona, a liczba próbek znowu rośnie.
5. Tryb samolotowy z włączonym Bluetooth: połączenie nadal działa (wolniej).

Jeśli urządzenia się nie widzą — sprawdź `NSBonjourServices` w `Info.plist` oraz zgodę na dostęp do sieci lokalnej w Ustawieniach.

- [ ] **Step 9: Commit**

```bash
git add App
git commit -m "feat(app): projekt iOS, transport MultipeerConnectivity, ekran parowania"
```

---

### Task 11: CaptureRig — kamera z zablokowanymi parametrami i zapis segmentowany

**Files:**
- Create: `App/TennisTrackerRig/Adapters/CaptureRig.swift`
- Create: `App/TennisTrackerRig/Session/SessionStore.swift`
- Create: `App/TennisTrackerRig/UI/PreviewView.swift`
- Modify: `App/TennisTrackerRig/UI/RigView.swift`

**Interfaces:**
- Consumes: `CaptureControlling`, `CaptureProfile`, `SessionManifest.LockedCameraSettings`, `SessionManifest.SegmentInfo` z RigCore
- Produces: `final class CaptureRig: CaptureControlling` z `init(store:)`, `func configure(profile:) throws`, `func startSession()`, `var previewLayer: AVCaptureVideoPreviewLayer`, `var onEvent: ((SessionManifest.SessionEvent) -> Void)?`; `final class SessionStore` z `func makeSessionDirectory(sessionId:) throws -> URL`, `func write(manifest:to:) throws`, `var sessionDirectories: [URL]`; `struct PreviewView`, `final class LevelMeter`

- [ ] **Step 1: Napisz magazyn sesji**

Plik `App/TennisTrackerRig/Session/SessionStore.swift`:

```swift
import Foundation
import RigCore

/// Uklad katalogow sesji na dysku telefonu.
/// Katalog lezy w Documents, zeby byl widoczny w aplikacji Pliki.
final class SessionStore {

    private let root: URL

    init() {
        root = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    func makeSessionDirectory(sessionId: String) throws -> URL {
        let directory = root.appendingPathComponent("session-\(sessionId)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    func write(manifest: SessionManifest, to directory: URL) throws {
        try manifest.encoded().write(to: directory.appendingPathComponent("manifest.json"))
    }

    var sessionDirectories: [URL] {
        (try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil))?
            .filter { $0.lastPathComponent.hasPrefix("session-") }
            .sorted { $0.lastPathComponent > $1.lastPathComponent } ?? []
    }
}
```

- [ ] **Step 2: Napisz `CaptureRig`**

Plik `App/TennisTrackerRig/Adapters/CaptureRig.swift`:

```swift
import AVFoundation
import Foundation
import RigCore

/// Kamera i mikrofon z parametrami zablokowanymi na sztywno.
///
/// Trzy rzeczy decyduja o przydatnosci nagrania w P1:
/// 1. Ekspozycja, ISO, ostrosc i balans bieli sa zablokowane. Autoekspozycja
///    polujaca w trakcie wymiany daje skoki jasnosci miedzy klatkami.
/// 2. Czas naswietlania jest krotki. Pilka przy 40 m/s w 1/1000 s przesuwa
///    sie 4 cm, czyli mniej niz wlasna srednica, i zostaje kropka.
/// 3. Bitrate jest wysoki. Kompresja traktuje siedmiopikselowa szybka plamke
///    jak szum i usuwa ja jako pierwsza.
final class CaptureRig: NSObject, CaptureControlling {

    enum Failure: Error {
        case noCamera
        case noMicrophone
        case profileUnavailable(CaptureProfile)
        case notRecording
    }

    /// Docelowy czas naswietlania. Krotszy niz typowa autoekspozycja.
    private let targetExposure = CMTime(value: 1, timescale: 1000)

    let session = AVCaptureSession()
    private(set) var lensName = "builtInWideAngleCamera"
    private(set) var firstFrameHostTime: Double?
    private(set) var intrinsicMatrix: [[Double]]?

    private let store: SessionStore
    private var device: AVCaptureDevice?
    private var videoOutput: AVCaptureVideoDataOutput?
    private var audioOutput: AVCaptureAudioDataOutput?
    private var writer: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var audioInput: AVAssetWriterInput?
    private var directory: URL?
    private var profile: CaptureProfile = .p1080_120
    private var segmentIndex = 0
    private var segmentStartHostTime: Double = 0
    private var segmentFrameCount = 0
    private var isRecording = false

    private let queue = DispatchQueue(label: "capture.rig")

    lazy var previewLayer: AVCaptureVideoPreviewLayer = {
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        return layer
    }()

    init(store: SessionStore) {
        self.store = store
        super.init()
    }

    // MARK: konfiguracja

    func configure(profile: CaptureProfile) throws {
        self.profile = profile
        session.beginConfiguration()
        session.sessionPreset = .inputPriority

        guard let camera = AVCaptureDevice.default(
            .builtInWideAngleCamera, for: .video, position: .back
        ) else { throw Failure.noCamera }
        self.device = camera

        guard let format = bestFormat(for: profile, on: camera) else {
            throw Failure.profileUnavailable(profile)
        }

        try camera.lockForConfiguration()
        camera.activeFormat = format
        let duration = CMTime(value: 1, timescale: CMTimeScale(profile.fps))
        camera.activeVideoMinFrameDuration = duration
        camera.activeVideoMaxFrameDuration = duration
        camera.unlockForConfiguration()

        let videoDeviceInput = try AVCaptureDeviceInput(device: camera)
        if session.canAddInput(videoDeviceInput) { session.addInput(videoDeviceInput) }

        guard let microphone = AVCaptureDevice.default(for: .audio) else {
            throw Failure.noMicrophone
        }
        let audioDeviceInput = try AVCaptureDeviceInput(device: microphone)
        if session.canAddInput(audioDeviceInput) { session.addInput(audioDeviceInput) }

        let videoOutput = AVCaptureVideoDataOutput()
        videoOutput.alwaysDiscardsLateVideoFrames = false
        videoOutput.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(videoOutput) { session.addOutput(videoOutput) }
        self.videoOutput = videoOutput

        // Macierz intrinsics per klatka. To gotowa kalibracja wewnetrzna
        // kamery, ktora w P1 oszczedza calej procedury z szachownica.
        if let connection = videoOutput.connection(with: .video),
           connection.isCameraIntrinsicMatrixDeliverySupported {
            connection.isCameraIntrinsicMatrixDeliveryEnabled = true
        }

        let audioOutput = AVCaptureAudioDataOutput()
        audioOutput.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(audioOutput) { session.addOutput(audioOutput) }
        self.audioOutput = audioOutput

        session.commitConfiguration()
    }

    private func bestFormat(for profile: CaptureProfile, on device: AVCaptureDevice) -> AVCaptureDevice.Format? {
        device.formats.first { format in
            let dimensions = CMVideoFormatDescriptionGetDimensions(format.formatDescription)
            guard dimensions.width == Int32(profile.width),
                  dimensions.height == Int32(profile.height) else { return false }
            return format.videoSupportedFrameRateRanges.contains {
                $0.maxFrameRate >= Double(profile.fps)
            }
        }
    }

    func startSession() {
        queue.async { [weak self] in self?.session.startRunning() }
    }

    // MARK: CaptureControlling

    func lockSettings() throws -> SessionManifest.LockedCameraSettings {
        guard let device else { throw Failure.noCamera }

        try device.lockForConfiguration()
        defer { device.unlockForConfiguration() }

        // Krotki czas naswietlania, ISO dobrane przez system pod ten czas.
        let exposure = max(targetExposure, device.activeFormat.minExposureDuration)
        device.setExposureModeCustom(duration: exposure, iso: AVCaptureDevice.currentISO)
        if device.isFocusModeSupported(.locked) {
            device.focusMode = .locked
        }
        if device.isWhiteBalanceModeSupported(.locked) {
            device.whiteBalanceMode = .locked
        }

        let gains = device.deviceWhiteBalanceGains
        return .init(
            exposureDurationSeconds: CMTimeGetSeconds(device.exposureDuration),
            iso: Double(device.iso),
            focusLensPosition: Double(device.lensPosition),
            whiteBalanceGains: .init(
                r: Double(gains.redGain),
                g: Double(gains.greenGain),
                b: Double(gains.blueGain)
            )
        )
    }

    func startRecording(sessionId: String, profile: CaptureProfile) throws {
        let directory = try store.makeSessionDirectory(sessionId: sessionId)
        self.directory = directory
        self.profile = profile
        self.segmentIndex = 0
        self.segmentFrameCount = 0
        self.firstFrameHostTime = nil
        try openWriter()
        isRecording = true
    }

    func rollSegment(now: Double) throws -> SessionManifest.SegmentInfo {
        let closed = try closeWriter(now: now)
        segmentIndex += 1
        segmentFrameCount = 0
        try openWriter()
        return closed
    }

    func stopRecording(now: Double) throws -> SessionManifest.SegmentInfo {
        isRecording = false
        return try closeWriter(now: now)
    }

    // MARK: zapis

    private func openWriter() throws {
        guard let directory else { throw Failure.notRecording }
        let url = directory.appendingPathComponent(String(format: "video-%03d.mov", segmentIndex))
        let writer = try AVAssetWriter(outputURL: url, fileType: .mov)

        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.hevc,
            AVVideoWidthKey: profile.width,
            AVVideoHeightKey: profile.height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: 40_000_000,
                AVVideoExpectedSourceFrameRateKey: profile.fps
            ]
        ]
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput.expectsMediaDataInRealTime = true
        if writer.canAdd(videoInput) { writer.add(videoInput) }

        // PCM, bez kompresji. Interesujace zdarzenia to krotkie transjenty,
        // a te gina w kodekach stratnych.
        let audioSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 48000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false
        ]
        let audioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
        audioInput.expectsMediaDataInRealTime = true
        if writer.canAdd(audioInput) { writer.add(audioInput) }

        self.writer = writer
        self.videoInput = videoInput
        self.audioInput = audioInput
    }

    private func closeWriter(now: Double) throws -> SessionManifest.SegmentInfo {
        guard let writer, let videoInput, let audioInput else { throw Failure.notRecording }
        let file = writer.outputURL.lastPathComponent
        let start = segmentStartHostTime
        let frames = segmentFrameCount

        videoInput.markAsFinished()
        audioInput.markAsFinished()

        let semaphore = DispatchSemaphore(value: 0)
        writer.finishWriting { semaphore.signal() }
        semaphore.wait()

        self.writer = nil
        self.videoInput = nil
        self.audioInput = nil

        return .init(file: file, startHostTime: start, endHostTime: now, frameCount: frames)
    }
}

extension CaptureRig: AVCaptureVideoDataOutputSampleBufferDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard isRecording, let writer else { return }

        let isVideo = output is AVCaptureVideoDataOutput
        let timestamp = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        if writer.status == .unknown {
            writer.startWriting()
            writer.startSession(atSourceTime: timestamp)
            segmentStartHostTime = CMTimeGetSeconds(timestamp)
            if firstFrameHostTime == nil {
                firstFrameHostTime = segmentStartHostTime
            }
        }
        guard writer.status == .writing else { return }

        if isVideo {
            captureIntrinsicsIfNeeded(from: sampleBuffer)
            if videoInput?.isReadyForMoreMediaData == true {
                videoInput?.append(sampleBuffer)
                segmentFrameCount += 1
            }
        } else if audioInput?.isReadyForMoreMediaData == true {
            audioInput?.append(sampleBuffer)
        }
    }

    private func captureIntrinsicsIfNeeded(from sampleBuffer: CMSampleBuffer) {
        guard intrinsicMatrix == nil else { return }
        guard let attachment = CMGetAttachment(
            sampleBuffer,
            key: kCMSampleBufferAttachmentKey_CameraIntrinsicMatrix,
            attachmentModeOut: nil
        ) as? Data else { return }

        let matrix: matrix_float3x3 = attachment.withUnsafeBytes { $0.load(as: matrix_float3x3.self) }
        intrinsicMatrix = (0..<3).map { row in
            (0..<3).map { column in Double(matrix[column][row]) }
        }
    }
}
```

- [ ] **Step 3: Dodaj obsługę przerwania sesji kamery**

Specyfikacja, sekcja 8, wymaga odnotowania przerwania `AVCaptureSession` (połączenie przychodzące, presja systemowa) jako zdarzenia w manifeście oraz automatycznego wznowienia. Dodaj do `CaptureRig`:

```swift
    /// Zdarzenia sprzetowe trafiajace do manifestu.
    var onEvent: ((SessionManifest.SessionEvent) -> Void)?

    private func observeInterruptions() {
        let center = NotificationCenter.default
        center.addObserver(
            forName: .AVCaptureSessionWasInterrupted, object: session, queue: .main
        ) { [weak self] notification in
            let raw = notification.userInfo?[AVCaptureSessionInterruptionReasonKey] as? Int
            let reason = raw.map(String.init) ?? "unknown"
            self?.onEvent?(.init(
                hostTime: CACurrentMediaTime(),
                type: "capture-interrupted",
                reason: reason
            ))
        }
        center.addObserver(
            forName: .AVCaptureSessionInterruptionEnded, object: session, queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            self.onEvent?(.init(
                hostTime: CACurrentMediaTime(),
                type: "capture-resumed"
            ))
            // System wstrzymuje sesje sam; wznowienie jest po naszej stronie.
            self.queue.async { self.session.startRunning() }
        }
    }
```

Dodaj `import QuartzCore` na górze pliku i wywołaj `observeInterruptions()` na końcu `configure(profile:)`, tuż po `session.commitConfiguration()`.

- [ ] **Step 4: Napisz podgląd z poziomicą**

Plik `App/TennisTrackerRig/UI/PreviewView.swift`:

```swift
import AVFoundation
import CoreMotion
import SwiftUI

/// Podglad kamery z pozioma kreska odchylenia.
/// Telefon wiszacy krzywo zaniza dokladnosc kalibracji w P1.
struct PreviewView: UIViewRepresentable {
    let layer: AVCaptureVideoPreviewLayer

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.layer.addSublayer(layer)
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        layer.frame = view.bounds
    }
}

@MainActor
final class LevelMeter: ObservableObject {
    @Published private(set) var rollDegrees: Double = 0

    private let motion = CMMotionManager()

    func start() {
        guard motion.isDeviceMotionAvailable else { return }
        motion.deviceMotionUpdateInterval = 0.1
        motion.startDeviceMotionUpdates(to: .main) { [weak self] data, _ in
            guard let data else { return }
            self?.rollDegrees = data.attitude.roll * 180 / .pi
        }
    }

    func stop() { motion.stopDeviceMotionUpdates() }
}
```

- [ ] **Step 5: Podłącz podgląd do `RigView`**

W `RigView` dodaj nad przełącznikiem roli:

```swift
            PreviewView(layer: coordinator.previewLayer)
                .frame(height: 220)
                .overlay(alignment: .center) {
                    Rectangle()
                        .fill(abs(levelMeter.rollDegrees) < 2 ? .green : .orange)
                        .frame(height: 2)
                        .rotationEffect(.degrees(levelMeter.rollDegrees))
                }
```

oraz `@StateObject private var levelMeter = LevelMeter()` i `.onAppear { levelMeter.start() }`.

W `SessionCoordinator` dodaj:

```swift
    let store = SessionStore()
    lazy var captureRig = CaptureRig(store: store)
    var previewLayer: AVCaptureVideoPreviewLayer { captureRig.previewLayer }

    func prepareCamera(profile: CaptureProfile = .p1080_120) throws {
        try captureRig.configure(profile: profile)
        captureRig.startSession()
    }
```

i wywołaj `try? prepareCamera()` w `connect()`. Podłącz też zdarzenia sprzętowe: `captureRig.onEvent = { [weak self] event in self?.recorder?.noteEvent(event) }`.

- [ ] **Step 6: Zbuduj**

Run: `xcodebuild -project App/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 7: Weryfikacja ręczna na urządzeniu**

1. Uruchom apkę, przyznaj dostęp do kamery i mikrofonu. Podgląd działa, kreska poziomicy reaguje na przechylanie telefonu.
2. Sprawdź w logu, że `configure(profile: .p1080_120)` nie rzucił `profileUnavailable`. Jeśli rzucił — iPhone 15 może nie mieć formatu 1920×1080 przy 120 fps na tym obiektywie; wtedy przetestuj z `.p1080_60` i odnotuj to w README.
3. Po `lockSettings()` sprawdź w logu wartości: `exposureDurationSeconds` powinno być około 0.001, `iso` w rozsądnym zakresie, `focusLensPosition` stałe.
4. Skieruj telefon na zmieniające się oświetlenie (zapal i zgaś lampę). Obraz **nie** może się rozjaśniać ani ściemniać — to dowód, że blokada działa.
5. Zadzwoń na testowany telefon w trakcie działania podglądu. Po odrzuceniu połączenia podgląd musi wrócić sam, a `onEvent` musi zgłosić `capture-interrupted`, a następnie `capture-resumed`.

- [ ] **Step 8: Commit**

```bash
git add App
git commit -m "feat(capture): kamera z zablokowanymi parametrami, audio PCM i zapis segmentowany"
```

---

### Task 12: Zapis ruchu, sondy systemowe i pełny przebieg sesji

**Files:**
- Create: `App/TennisTrackerRig/Adapters/MotionRecorder.swift`
- Create: `App/TennisTrackerRig/Adapters/SystemProbes.swift`
- Modify: `App/TennisTrackerRig/Session/SessionCoordinator.swift`
- Modify: `App/TennisTrackerRig/UI/RigView.swift`

**Interfaces:**
- Consumes: `MotionAnalyzer`, `MotionSample`, `StorageProbing`, `ThermalProbing`, `SessionRecorder`, `SyncSnapshotProviding` z RigCore
- Produces: `final class MotionRecorder` z `func start(directory:analyzer:onSpike:)` i `func stop()`; `final class SystemStorage: StorageProbing`; `final class SystemThermal: ThermalProbing`; rozszerzenie `SessionCoordinator` o `func startRecording()` i `func stopRecording()`

- [ ] **Step 1: Napisz sondy systemowe**

Plik `App/TennisTrackerRig/Adapters/SystemProbes.swift`:

```swift
import Foundation
import RigCore

final class SystemStorage: StorageProbing {
    var freeBytes: Int64 {
        let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let values = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        return values?.volumeAvailableCapacityForImportantUsage ?? 0
    }
}

final class SystemThermal: ThermalProbing {
    var thermalLevel: ThermalLevel {
        switch ProcessInfo.processInfo.thermalState {
        case .nominal: return .nominal
        case .fair: return .fair
        case .serious: return .serious
        case .critical: return .critical
        @unknown default: return .fair
        }
    }
}
```

- [ ] **Step 2: Napisz zapis ruchu**

Plik `App/TennisTrackerRig/Adapters/MotionRecorder.swift`:

```swift
import CoreMotion
import Foundation
import QuartzCore
import RigCore

/// Zapisuje zyroskop do motion.jsonl i zglasza szarpniecia.
///
/// Log sluzy w P1 do oceny, jak bardzo kamera dryfowala na ogrodzeniu,
/// a wykryte skoki sa markerami "tutaj przelicz homografie od nowa".
final class MotionRecorder {

    private let motion = CMMotionManager()
    private var handle: FileHandle?
    private var analyzer = MotionAnalyzer()

    func start(
        directory: URL,
        onSpike: @escaping (SessionManifest.SessionEvent) -> Void
    ) throws {
        let url = directory.appendingPathComponent("motion.jsonl")
        FileManager.default.createFile(atPath: url.path, contents: nil)
        handle = try FileHandle(forWritingTo: url)

        guard motion.isDeviceMotionAvailable else { return }
        motion.deviceMotionUpdateInterval = 0.01
        motion.startDeviceMotionUpdates(to: .main) { [weak self] data, _ in
            guard let self, let data else { return }
            let now = CACurrentMediaTime()
            let sample = MotionSample(
                hostTime: now,
                rotationRate: .init(
                    x: data.rotationRate.x,
                    y: data.rotationRate.y,
                    z: data.rotationRate.z
                )
            )
            self.append(sample, gravity: data.gravity)
            if let event = self.analyzer.process(sample) {
                onSpike(event)
            }
        }
    }

    func stop() {
        motion.stopDeviceMotionUpdates()
        try? handle?.close()
        handle = nil
    }

    private func append(_ sample: MotionSample, gravity: CMAcceleration) {
        let line = """
        {"hostTime":\(sample.hostTime),\
        "rx":\(sample.rotationRate.x),"ry":\(sample.rotationRate.y),"rz":\(sample.rotationRate.z),\
        "gx":\(gravity.x),"gy":\(gravity.y),"gz":\(gravity.z)}

        """
        if let data = line.data(using: .utf8) {
            handle?.write(data)
        }
    }
}
```

- [ ] **Step 3: Rozszerz koordynator o pełny przebieg sesji**

Dodaj do `SessionCoordinator`:

```swift
    @Published private(set) var isRecording = false
    @Published private(set) var thermalState = "nominal"
    @Published private(set) var freeGigabytes = 0.0
    @Published private(set) var elapsedSeconds = 0.0

    private let storageProbe = SystemStorage()
    private let thermalProbe = SystemThermal()
    private let motionRecorder = MotionRecorder()
    private var recorder: SessionRecorder?
    private var sessionDirectory: URL?
    private var recordingStarted: Double = 0

    /// Identyfikator wspolny dla obu telefonow — nadaje go master.
    private func makeSessionId() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
        formatter.timeZone = TimeZone(identifier: "UTC")
        let suffix = String(UUID().uuidString.prefix(4)).lowercased()
        return "\(formatter.string(from: Date()))-\(suffix)"
    }

    func startRecording(profile: CaptureProfile = .p1080_120) {
        guard let syncSession, let link else { return }
        let sessionId = makeSessionId()
        beginRecording(sessionId: sessionId, profile: profile, sync: syncSession)
        try? link.send(.startRecording(sessionId: sessionId, profile: profile, hostTime: clock.now))
    }

    func beginRecording(sessionId: String, profile: CaptureProfile, sync: SyncSession) {
        let recorder = SessionRecorder(
            role: role,
            device: .init(
                model: UIDevice.current.modelIdentifier,
                osVersion: UIDevice.current.systemVersion,
                appVersion: Bundle.main.appVersion
            ),
            peerDeviceModel: nil,
            capture: captureRig,
            storage: storageProbe,
            thermal: thermalProbe,
            sync: sync,
            config: RecorderConfig()
        )
        do {
            try recorder.arm()
            try recorder.start(sessionId: sessionId, profile: profile, now: clock.now)
            let directory = try store.makeSessionDirectory(sessionId: sessionId)
            sessionDirectory = directory
            try motionRecorder.start(directory: directory) { [weak recorder] event in
                recorder?.noteEvent(event)
            }
            self.recorder = recorder
            self.recordingStarted = clock.now
            self.isRecording = true
        } catch {
            self.recorder = nil
        }
    }

    func stopRecording() {
        try? link?.send(.stopRecording(hostTime: clock.now))
        finishRecording()
    }

    func finishRecording() {
        guard let recorder, let sessionDirectory else { return }
        motionRecorder.stop()
        if let manifest = try? recorder.stop(now: clock.now) {
            try? store.write(manifest: manifest, to: sessionDirectory)
        }
        self.recorder = nil
        self.isRecording = false
    }
```

Rozszerz `tick()`:

```swift
    private func tick() {
        guard let syncSession else { return }
        syncSession.tick(now: clock.now)
        syncQualityMs = syncSession.currentFit?.residualStdMs
        sampleCount = syncSession.samples.count

        thermalState = String(describing: thermalProbe.thermalLevel)
        freeGigabytes = Double(storageProbe.freeBytes) / 1_000_000_000

        if let recorder {
            recorder.tick(now: clock.now)
            elapsedSeconds = clock.now - recordingStarted
            if case .stopped = recorder.state { finishRecording() }
        }
        try? link?.send(.heartbeat(hostTime: clock.now))
    }
```

Rozszerz obsługę wiadomości w `connect()`:

```swift
        link.onMessage = { [weak self] message in
            guard let self else { return }
            sync.handle(message, receivedAt: self.clock.now)
            switch message {
            case let .startRecording(sessionId, profile, _):
                if self.role == .slave, !self.isRecording {
                    self.beginRecording(sessionId: sessionId, profile: profile, sync: sync)
                }
            case .stopRecording:
                if self.role == .slave { self.finishRecording() }
            case let .heartbeat(hostTime):
                self.recorder?.notePeerHeartbeat(at: hostTime)
                _ = hostTime
            default:
                break
            }
        }
```

Dodaj pomocnicze rozszerzenia w tym samym pliku:

```swift
extension UIDevice {
    /// Identyfikator sprzetowy, np. "iPhone17,2". `model` zwraca tylko "iPhone".
    var modelIdentifier: String {
        var info = utsname()
        uname(&info)
        return withUnsafePointer(to: &info.machine) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: 1) { String(cString: $0) }
        }
    }
}

extension Bundle {
    var appVersion: String {
        (infoDictionary?["CFBundleShortVersionString"] as? String) ?? "0.0.0"
    }
}
```

- [ ] **Step 4: Dodaj sterowanie do `RigView`**

```swift
            Button(coordinator.isRecording ? "Zatrzymaj" : "Nagrywaj") {
                coordinator.isRecording ? coordinator.stopRecording() : coordinator.startRecording()
            }
            .buttonStyle(.borderedProminent)
            .tint(coordinator.isRecording ? .red : .accentColor)
            .disabled(!coordinator.isConnected)

            if coordinator.isRecording {
                HStack(spacing: 16) {
                    Text(String(format: "%.0f s", coordinator.elapsedSeconds))
                    Text(String(format: "%.1f GB", coordinator.freeGigabytes))
                    Text(coordinator.thermalState)
                }
                .font(.caption.monospaced())
            }
```

- [ ] **Step 5: Zbuduj**

Run: `xcodebuild -project App/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 6: Weryfikacja ręczna — pełna sesja**

1. Połącz oba telefony, poczekaj na zieloną jakość synchronizacji.
2. Na masterze naciśnij „Nagrywaj". **Slave musi zacząć nagrywać sam** — sprawdź, że jego przycisk zmienił się na „Zatrzymaj".
3. Nagrywaj 6 minut, żeby wymusić przewinięcie segmentu.
4. Zatrzymaj na masterze. Slave musi się zatrzymać sam.
5. W aplikacji Pliki na obu telefonach sprawdź katalog `session-…`: musi zawierać `manifest.json`, co najmniej dwa pliki `video-*.mov` i `motion.jsonl`.
6. Otwórz `manifest.json` i sprawdź, że `sessionId` jest **identyczny na obu telefonach**, `role` się różni, a `syncModel.residualStdMs` jest poniżej 2.
7. Szarpnij telefonem w trakcie nagrania i sprawdź, że w `events` pojawił się `motion-spike`.
8. **Test zerwania łącza:** w trakcie nagrania wyłącz WiFi na slavie na 30 sekund. Nagrywanie musi trwać dalej na obu urządzeniach, a po zakończeniu w `syncGaps` musi być wpis.

- [ ] **Step 7: Commit**

```bash
git add App
git commit -m "feat(app): zapis ruchu, sondy systemowe i pelny przebieg sesji"
```

---

### Task 13: Eksport sesji

**Files:**
- Create: `App/TennisTrackerRig/UI/ExportView.swift`
- Modify: `App/TennisTrackerRig/UI/RigView.swift`
- Modify: `App/TennisTrackerRig/Info.plist`

**Interfaces:**
- Consumes: `SessionStore` z Task 11
- Produces: `struct ExportView` — lista sesji z akcją udostępniania

- [ ] **Step 1: Włącz widoczność w aplikacji Pliki**

Dodaj do `Info.plist`:

```xml
<key>UIFileSharingEnabled</key>
<true/>
<key>LSSupportsOpeningDocumentsInPlace</key>
<true/>
```

Dzięki temu katalogi sesji są widoczne w Plikach i można je przeciągnąć na Maca kablem albo przez AirDrop bez pisania własnego eksportera.

- [ ] **Step 2: Napisz listę sesji z udostępnianiem**

Plik `App/TennisTrackerRig/UI/ExportView.swift`:

```swift
import SwiftUI

struct ExportView: View {
    let store: SessionStore
    @State private var shared: URL?

    var body: some View {
        List(store.sessionDirectories, id: \.self) { directory in
            HStack {
                VStack(alignment: .leading) {
                    Text(directory.lastPathComponent).font(.body.monospaced())
                    Text(describe(directory)).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button("Udostepnij") { shared = directory }
            }
        }
        .navigationTitle("Sesje")
        .sheet(item: $shared) { url in
            ShareSheet(items: [url])
        }
    }

    private func describe(_ directory: URL) -> String {
        let contents = (try? FileManager.default.contentsOfDirectory(
            at: directory, includingPropertiesForKeys: [.fileSizeKey]
        )) ?? []
        let segments = contents.filter { $0.pathExtension == "mov" }.count
        let bytes = contents.reduce(Int64(0)) { total, url in
            let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            return total + Int64(size)
        }
        return String(format: "%d segmentow, %.1f GB", segments, Double(bytes) / 1_000_000_000)
    }
}

extension URL: Identifiable {
    public var id: String { absoluteString }
}

struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
```

- [ ] **Step 3: Podłącz do `RigView`**

Owiń zawartość `RigView` w `NavigationStack` i dodaj:

```swift
            NavigationLink("Sesje") {
                ExportView(store: coordinator.store)
            }
```

- [ ] **Step 4: Zbuduj**

Run: `xcodebuild -project App/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 5: Weryfikacja ręczna**

1. Ekran „Sesje" pokazuje nagrane sesje z liczbą segmentów i rozmiarem.
2. Katalogi sesji są widoczne w aplikacji Pliki → Na moim iPhonie → TennisTrackerRig.
3. Przenieś sesję z obu telefonów na Maca do `~/tennis-sessions/master/` i `~/tennis-sessions/slave/`.

- [ ] **Step 6: Commit**

```bash
git add App
git commit -m "feat(app): lista sesji i eksport na Maca"
```

---

### Task 14: Narzędzie weryfikujące i test akceptacyjny

**Files:**
- Create: `tools/requirements.txt`
- Create: `tools/ms_counter.html`
- Create: `tools/verify_session.py`
- Create: `tools/test_verify_session.py`
- Create: `README.md`

**Interfaces:**
- Consumes: katalogi sesji z Tasków 12-13 (`manifest.json` w formacie z Task 7)
- Produces: `verify_session.py` z funkcjami `load_manifest(path)`, `peer_time(sync_model, host_time)`, `check_pair(master, slave)`, `locate_frame(manifest, host_time)`; CLI `python tools/verify_session.py <master_dir> <slave_dir> [--extract N --out DIR]`

- [ ] **Step 1: Napisz licznik milisekundowy**

Plik `tools/ms_counter.html`:

```html
<!doctype html>
<meta charset="utf-8">
<title>Licznik ms</title>
<style>
  body { margin: 0; background: #000; color: #fff;
         display: flex; align-items: center; justify-content: center;
         height: 100vh; font-family: monospace; }
  #t { font-size: 16vw; font-weight: 700; letter-spacing: 0.05em; }
</style>
<div id="t">0000.000</div>
<script>
  // Licznik odswiezany co klatke. Sluzy jako wspolne, widoczne zrodlo
  // prawdy dla obu kamer w tescie akceptacyjnym synchronizacji.
  const el = document.getElementById('t');
  const start = performance.now();
  function frame() {
    const seconds = (performance.now() - start) / 1000;
    el.textContent = seconds.toFixed(3).padStart(8, '0');
    requestAnimationFrame(frame);
  }
  frame();
</script>
```

- [ ] **Step 2: Napisz zależności**

Plik `tools/requirements.txt`:

```
pytest==8.3.3
```

Dodatkowo w systemie wymagany jest `ffmpeg` (`brew install ffmpeg`).

- [ ] **Step 3: Napisz testy narzędzia**

Plik `tools/test_verify_session.py`:

```python
import json
from pathlib import Path

import pytest

from verify_session import (
    ManifestError,
    check_pair,
    load_manifest,
    locate_frame,
    peer_time,
)


def make_manifest(role, session_id="s-1", offset=0.0, skew_ppm=0.0):
    return {
        "schemaVersion": 1,
        "sessionId": session_id,
        "role": role,
        "device": {"model": "iPhone17,2", "osVersion": "26.0", "appVersion": "0.1.0"},
        "camera": {
            "lens": "builtInWideAngleCamera",
            "profile": "1080p120",
            "width": 1920,
            "height": 1080,
            "fps": 120,
            "codec": "hevc",
            "targetBitrateMbps": 40,
            "audio": {"sampleRateHz": 48000, "channels": 1, "codec": "pcm"},
            "locked": {
                "exposureDurationSeconds": 0.001,
                "iso": 64.0,
                "focusLensPosition": 0.82,
                "whiteBalanceGains": {"r": 1.9, "g": 1.0, "b": 1.6},
            },
            "intrinsicMatrix": [[1580, 0, 960], [0, 1580, 540], [0, 0, 1]],
        },
        "timing": {
            "clockDomain": "mach_absolute_time",
            "firstFrameHostTime": 1000.0,
            "syncModel": {
                "referenceHostTime": 1000.0,
                "offsetSeconds": offset,
                "skewPpm": skew_ppm,
                "residualStdMs": 0.9,
            },
            "syncSamples": [],
            "syncGaps": [],
        },
        "segments": [
            {"file": "video-000.mov", "startHostTime": 1000.0,
             "endHostTime": 1300.0, "frameCount": 36000},
            {"file": "video-001.mov", "startHostTime": 1300.0,
             "endHostTime": 1600.0, "frameCount": 36000},
        ],
        "events": [],
        "motionLog": "motion.jsonl",
    }


def write_session(tmp_path, name, manifest):
    directory = tmp_path / name
    directory.mkdir()
    (directory / "manifest.json").write_text(json.dumps(manifest))
    return directory


def test_load_manifest_reads_json(tmp_path):
    directory = write_session(tmp_path, "master", make_manifest("master"))
    assert load_manifest(directory)["role"] == "master"


def test_load_manifest_rejects_missing_file(tmp_path):
    (tmp_path / "empty").mkdir()
    with pytest.raises(ManifestError):
        load_manifest(tmp_path / "empty")


def test_peer_time_applies_offset_and_skew():
    model = {"referenceHostTime": 1000.0, "offsetSeconds": 0.25, "skewPpm": 20.0}
    assert peer_time(model, 1000.0) == pytest.approx(1000.25)
    # 20 ppm przez 3600 s to 72 ms
    assert peer_time(model, 4600.0) == pytest.approx(4600.322)


def test_check_pair_accepts_matching_session(tmp_path):
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", make_manifest("slave"))
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert report.ok, report.problems


def test_check_pair_rejects_different_session_ids(tmp_path):
    master = write_session(tmp_path, "master", make_manifest("master", session_id="a"))
    slave = write_session(tmp_path, "slave", make_manifest("slave", session_id="b"))
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert not report.ok
    assert any("sessionId" in problem for problem in report.problems)


def test_check_pair_rejects_two_masters(tmp_path):
    master = write_session(tmp_path, "a", make_manifest("master"))
    other = write_session(tmp_path, "b", make_manifest("master"))
    report = check_pair(load_manifest(master), load_manifest(other))
    assert not report.ok
    assert any("role" in problem for problem in report.problems)


def test_check_pair_flags_poor_sync_quality(tmp_path):
    bad = make_manifest("slave")
    bad["timing"]["syncModel"]["residualStdMs"] = 7.5
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", bad)
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert not report.ok
    assert any("residualStdMs" in problem for problem in report.problems)


def test_check_pair_flags_segment_discontinuity(tmp_path):
    broken = make_manifest("slave")
    broken["segments"][1]["startHostTime"] = 1305.0
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", broken)
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert not report.ok
    assert any("ciaglosc" in problem for problem in report.problems)


def test_locate_frame_finds_segment_and_offset():
    manifest = make_manifest("master")
    segment, offset = locate_frame(manifest, 1350.5)
    assert segment == "video-001.mov"
    assert offset == pytest.approx(50.5)


def test_locate_frame_returns_none_outside_range():
    manifest = make_manifest("master")
    assert locate_frame(manifest, 5000.0) is None
```

- [ ] **Step 4: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cd tools && python -m pytest test_verify_session.py -v`
Expected: FAIL, `ModuleNotFoundError: No module named 'verify_session'`

- [ ] **Step 5: Napisz narzędzie**

Plik `tools/verify_session.py`:

```python
#!/usr/bin/env python3
"""Weryfikuje pare sesji nagranych przez rig P0.

Sprawdza spojnosc manifestow, jakosc synchronizacji i ciaglosc segmentow.
Opcjonalnie wyciaga pary klatek jednoczesnych wedlug modelu synchronizacji —
to test akceptacyjny: na obu klatkach licznik milisekundowy musi pokazywac
te sama wartosc.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

#: Maksymalny akceptowalny rozrzut synchronizacji.
MAX_RESIDUAL_MS = 2.0


class ManifestError(Exception):
    pass


@dataclass
class Report:
    problems: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.problems


def load_manifest(directory: Path) -> dict:
    path = Path(directory) / "manifest.json"
    if not path.exists():
        raise ManifestError(f"brak manifest.json w {directory}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise ManifestError(f"uszkodzony manifest w {directory}: {error}") from error


def peer_time(sync_model: dict, host_time: float) -> float:
    """Przelicza czas lokalny na czas peera wedlug modelu liniowego."""
    skew = sync_model["skewPpm"] * 1e-6
    return (
        host_time
        + sync_model["offsetSeconds"]
        + skew * (host_time - sync_model["referenceHostTime"])
    )


def _check_segments(manifest: dict, label: str, report: Report) -> None:
    segments = manifest["segments"]
    if not segments:
        report.problems.append(f"{label}: brak segmentow")
        return
    tolerance = 1.0 / manifest["camera"]["fps"]
    for index in range(1, len(segments)):
        gap = segments[index]["startHostTime"] - segments[index - 1]["endHostTime"]
        if gap > tolerance:
            report.problems.append(
                f"{label}: ciaglosc segmentow przerwana przed {segments[index]['file']} "
                f"(przerwa {gap * 1000:.1f} ms)"
            )


def check_pair(master: dict, slave: dict) -> Report:
    report = Report()

    if master["sessionId"] != slave["sessionId"]:
        report.problems.append(
            f"rozne sessionId: {master['sessionId']} vs {slave['sessionId']}"
        )

    roles = {master["role"], slave["role"]}
    if roles != {"master", "slave"}:
        report.problems.append(f"niepoprawny uklad role: {sorted(roles)}")

    for manifest, label in ((master, "master"), (slave, "slave")):
        if manifest["schemaVersion"] != 1:
            report.problems.append(
                f"{label}: nieobslugiwany schemaVersion {manifest['schemaVersion']}"
            )
        if manifest["timing"]["clockDomain"] != "mach_absolute_time":
            report.problems.append(f"{label}: nieoczekiwana domena zegara")

        residual = manifest["timing"]["syncModel"]["residualStdMs"]
        if residual > MAX_RESIDUAL_MS:
            report.problems.append(
                f"{label}: residualStdMs {residual:.2f} ms przekracza cel {MAX_RESIDUAL_MS} ms"
            )
        else:
            report.notes.append(f"{label}: synchronizacja ±{residual:.2f} ms")

        _check_segments(manifest, label, report)

        gaps = manifest["timing"]["syncGaps"]
        if gaps:
            total = sum(
                (gap["endHostTime"] or gap["startHostTime"]) - gap["startHostTime"]
                for gap in gaps
            )
            report.notes.append(f"{label}: {len(gaps)} luk synchronizacji, lacznie {total:.1f} s")

        duration = manifest["segments"][-1]["endHostTime"] - manifest["segments"][0]["startHostTime"]
        report.notes.append(f"{label}: {len(manifest['segments'])} segmentow, {duration:.1f} s")

    return report


def locate_frame(manifest: dict, host_time: float) -> tuple[str, float] | None:
    """Zwraca (nazwa pliku, przesuniecie w sekundach) dla podanego czasu lokalnego."""
    for segment in manifest["segments"]:
        if segment["startHostTime"] <= host_time <= segment["endHostTime"]:
            return segment["file"], host_time - segment["startHostTime"]
    return None


def extract_pair(
    master_dir: Path,
    master: dict,
    slave_dir: Path,
    slave: dict,
    host_time: float,
    out_dir: Path,
    index: int,
) -> bool:
    """Wyciaga dwie klatki, ktore wedlug modelu synchronizacji sa jednoczesne."""
    master_hit = locate_frame(master, host_time)
    slave_hit = locate_frame(slave, peer_time(master["timing"]["syncModel"], host_time))
    if master_hit is None or slave_hit is None:
        return False

    out_dir.mkdir(parents=True, exist_ok=True)
    for directory, (file_name, offset), label in (
        (master_dir, master_hit, "master"),
        (slave_dir, slave_hit, "slave"),
    ):
        subprocess.run(
            [
                "ffmpeg", "-y", "-loglevel", "error",
                "-ss", f"{offset:.4f}",
                "-i", str(Path(directory) / file_name),
                "-frames:v", "1",
                str(out_dir / f"pair-{index:02d}-{label}.png"),
            ],
            check=True,
        )
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("master_dir", type=Path)
    parser.add_argument("slave_dir", type=Path)
    parser.add_argument(
        "--extract", type=int, default=0,
        help="ile par klatek jednoczesnych wyciagnac do porownania"
    )
    parser.add_argument("--out", type=Path, default=Path("out"))
    args = parser.parse_args()

    master = load_manifest(args.master_dir)
    slave = load_manifest(args.slave_dir)
    if master["role"] == "slave":
        master, slave = slave, master
        args.master_dir, args.slave_dir = args.slave_dir, args.master_dir

    report = check_pair(master, slave)
    for note in report.notes:
        print(f"  {note}")
    for problem in report.problems:
        print(f"BLAD: {problem}")

    if args.extract > 0:
        segments = master["segments"]
        start = segments[0]["startHostTime"] + 2.0
        end = segments[-1]["endHostTime"] - 2.0
        step = (end - start) / max(args.extract - 1, 1)
        extracted = 0
        for index in range(args.extract):
            if extract_pair(
                args.master_dir, master, args.slave_dir, slave,
                start + index * step, args.out, index
            ):
                extracted += 1
        print(f"  wyciagnieto {extracted} par klatek do {args.out}")
        print("  Porownaj licznik na kazdej parze — musi pokazywac te sama wartosc.")

    print("OK" if report.ok else "NIEPOWODZENIE")
    return 0 if report.ok else 1


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 6: Uruchom testy i potwierdź, że przechodzą**

Run: `cd tools && python -m pytest test_verify_session.py -v`
Expected: PASS, 10 testów

- [ ] **Step 7: Wykonaj test akceptacyjny**

1. Otwórz `tools/ms_counter.html` na Macu w przeglądarce, tryb pełnoekranowy.
2. Ustaw oba telefony obok siebie, oba skierowane na ekran Maca, tak żeby licznik był czytelny w kadrze.
3. Połącz je, poczekaj na zieloną jakość synchronizacji.
4. Nagrywaj 2 minuty.
5. Przenieś obie sesje na Maca.
6. Uruchom:

```bash
python tools/verify_session.py ~/tennis-sessions/master ~/tennis-sessions/slave --extract 10 --out out/
```

7. Otwórz pary PNG z `out/` i porównaj odczyt licznika na klatce mastera i slave'a.

**Kryterium zaliczenia:** różnica odczytu licznika w każdej parze nie przekracza **jednego okresu klatki** (8,3 ms przy 120 fps). Większa różnica oznacza błąd w łańcuchu synchronizacji i musi zostać zdiagnozowana przed zamknięciem P0.

- [ ] **Step 8: Napisz README**

Plik `README.md`:

```markdown
# Tennis Tracker

System automatycznej punktacji meczu tenisowego z dwoch iPhone'ow
zawieszonych na ogrodzeniu kortu.

Podprojekty: **P0** rig danych (ten kod) → P1 detektor pilki i kalibracja
kortu → P2 tablica wynikow na zywo → P3 challenge 3D → P4 debel.

## Struktura

- `Sources/RigCore` — logika bezsprzetowa, testowana na macOS przez `swift test`
- `App/TennisTrackerRig` — aplikacja iOS, adaptery sprzetowe
- `tools/` — weryfikacja sesji i test akceptacyjny
- `docs/superpowers/specs` — specyfikacje
- `docs/superpowers/plans` — plany implementacji

## Testy

```bash
swift test                                   # logika RigCore
cd tools && python -m pytest                 # narzedzie weryfikujace
```

## Uzycie na korcie

1. Zainstaluj apke na oba telefony (darmowy provisioning wygasa co 7 dni).
2. Zawies telefony na przeciwnych liniach bocznych, w polowie dlugosci
   swojej polowki kortu, na wysokosci okolo 3 m.
3. Na iPhone 16 Pro Max wybierz Master, na iPhone 15 wybierz Slave, polacz.
4. Poczekaj na zielona jakosc synchronizacji (ponizej 2 ms).
5. Nagrywaj. Slave startuje i zatrzymuje sie automatycznie.
6. Przenies obie sesje na Maca i sprawdz:

```bash
python tools/verify_session.py <katalog-master> <katalog-slave>
```

## Uwagi

- 1080p120 zajmuje okolo 18 GB na godzine. Sprawdz miejsce przed meczem.
- Jesli telefony sie nie widza — sprawdz `NSBonjourServices` w Info.plist
  oraz zgode na dostep do sieci lokalnej w Ustawieniach.
```

- [ ] **Step 9: Commit**

```bash
git add tools README.md
git commit -m "feat(tools): weryfikacja sesji i test akceptacyjny synchronizacji"
```

---

## Kryteria ukończenia P0

Odwzorowanie sekcji 10 specyfikacji na sprawdzalne fakty:

1. `swift test` przechodzi w całości — Taski 1-9.
2. `cd tools && python -m pytest` przechodzi w całości — Task 14.
3. Aplikacja paruje telefony bez internetu — Task 10, krok 8.
4. Jakość synchronizacji poniżej 2 ms widoczna w UI — Task 10, krok 8.
5. Sesja 30-minutowa z poprawnymi manifestami po obu stronach — Task 12, krok 6.
6. Zerwanie łącza nie przerywa zapisu, luka odnotowana w `syncGaps` — Task 12, krok 6, punkt 8.
7. Test akceptacyjny z licznikiem milisekundowym zaliczony — Task 14, krok 7.
8. Sesja eksportowalna na Maca i wczytywalna narzędziem — Task 13, krok 5 oraz Task 14, krok 6.
