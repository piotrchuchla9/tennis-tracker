# P0a część 2 — aplikacja iOS: plan implementacji

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aplikacja iOS, która na dwóch iPhone'ach nagrywa zsynchronizowany materiał ze sztywno zablokowanymi parametrami kamery i zapisuje kompletny manifest — plus narzędzie w Pythonie, które ten materiał weryfikuje i przeprowadza test akceptacyjny synchronizacji.

**Architecture:** Cała logika mieszka w gotowym crate'cie `tracker-core` (część 1). Ten plan dokłada wyłącznie **adaptery**: AVFoundation, MultipeerConnectivity, CoreMotion oraz cienką powłokę SwiftUI. Warstwa natywna implementuje traity wystawione przez rdzeń i wywołuje jego metody; żadna decyzja nie jest podejmowana w Swifcie.

**Tech Stack:** Swift 5.9, SwiftUI, AVFoundation, MultipeerConnectivity, CoreMotion, UniFFI 0.28, Python 3 + ffmpeg.

**Spec:** `docs/superpowers/specs/2026-09-07-tennis-tracker-p0a-rdzen-ios-design.md`

**Poprzedni plan:** `docs/superpowers/plans/2026-09-07-p0a-1-rdzen-rust.md` — ukończony, `cargo test` zielony.

**Część 3** (aplikacja watchOS, transport BLE) powstanie osobno. Ten plan kończy się działającym rigiem: dwa iPhone'y nagrywają, a skrypt potwierdza zgodność osi czasu.

## Global Constraints

- Minimalna platforma: **iOS 17**. Sterowanie kamerą wymaga urządzenia — symulator nie wystarczy.
- Konto Apple Developer w wersji darmowej: **apka wygasa po 7 dniach**, UI pokazuje datę wygaśnięcia.
- **Jedyne źródło czasu w aplikacji to `CACurrentMediaTime()`.** Nigdy `Date()` do znaczników zdarzeń. Ta sama podstawa co `CMSampleBuffer.presentationTimeStamp` (`CMClockGetHostTimeClock()`), więc znaczniki klatek są bezpośrednio porównywalne z synchronizacją bez konwersji.
- Rdzeń **nie odczytuje zegara** — każde wywołanie przyjmujące `now` dostaje odczyt z `MonotonicClock`.
- Profil domyślny: **`1080p120`**. Czas naświetlania **1/1000 s**, blokada ekspozycji, ISO, ostrości i balansu bieli przed startem.
- Audio: **PCM, 48000 Hz, mono**, w tym samym pliku co wideo. Kompresja wykluczona.
- Segmenty po **300 sekund**. Bitrate **40 Mb/s**, HEVC.
- Cel synchronizacji: **poniżej 2 ms** (`residualStdMs < 2.0`).
- **Utrata łączności nigdy nie zatrzymuje nagrywania.**
- Pingi idą kanałem `Delivery.unreliable`, komendy `Delivery.reliable`. W MultipeerConnectivity odpowiednio `.unreliable` i `.reliable`.
- Teksty UI: **angielski jako język bazowy**, polski jako dodana lokalizacja (String Catalog). Komunikaty błędów z rdzenia **nigdy nie trafiają na ekran** — UI mapuje wariant enuma.
- Nazwy symboli: angielski. Komentarze w kodzie: polski bez znaków diakrytycznych.

---

## Struktura plików

```
scripts/build-xcframework.sh          buduje tracker_core.xcframework
TrackerCore.xcframework/              artefakt, ignorowany w git
app-ios/TennisTrackerRig.xcodeproj
app-ios/TennisTrackerRig/
  TennisTrackerRigApp.swift           punkt wejscia
  Adapters/MonotonicClock.swift       jedyne zrodlo czasu
  Adapters/MultipeerTransport.swift   PeerTransport na MultipeerConnectivity
  Adapters/CaptureRig.swift           CaptureControlling na AVFoundation
  Adapters/SystemProbes.swift         StorageProbing, ThermalProbing
  Adapters/MotionRecorder.swift       CoreMotion + zapis motion.jsonl
  Adapters/CapabilityProbe.swift      odczyt zdolnosci urzadzenia
  Session/SessionStore.swift          katalogi sesji, zapis manifestu
  Session/PlayerRoster.swift          trwala lista graczy
  Session/SessionCoordinator.swift    spina rdzen z adapterami
  UI/RigView.swift                    ekran glowny
  UI/PreviewView.swift                podglad kamery + poziomica
  UI/MatchSetupView.swift             wybor graczy i stron
  UI/DiagnosticsView.swift            raport zdolnosci
  UI/ExportView.swift                 lista sesji i eksport
  Localizable.xcstrings               String Catalog
  Info.plist
tools/verify_session.py               weryfikacja sesji + test akceptacyjny
tools/test_verify_session.py
tools/ms_counter.html                 licznik milisekundowy
tools/requirements.txt
README.md
```

Podział jest ten sam co w rdzeniu: pliki w `Adapters/` są cienkie i pozbawione decyzji, wszystko poniżej progu decyzyjności zostało już napisane i przetestowane w części 1.

---

### Task 1: Uzupełnienie fasady FFI o nagrywanie

**To zadanie naprawia lukę w części 1.** Fasada eksportuje `SyncEngine`, ale `SessionRecorder` nie przechodzi przez granicę FFI, a traity portów nie mają `with_foreign` — Swift nie ma jak zaimplementować kamery ani uruchomić maszyny stanów. Bez tego reszta planu nie ma się o co oprzeć.

**Files:**
- Modify: `src/session/ports.rs`, `src/api.rs`, `src/session/recorder.rs`
- Test: `tests/api.rs`

**Interfaces:**
- Consumes: `SessionRecorder`, `CaptureControlling`, `StorageProbing`, `ThermalProbing`, `RecorderConfig`, `RecorderState` z części 1
- Produces: traity portów eksportowane `with_foreign`; obiekt UniFFI `Recorder` z `new(...)`, `arm()`, `start(session_id, profile, now)`, `tick(now)`, `note_event(event)`, `note_peer_heartbeat(at)`, `stop(now, snapshot) -> SessionManifest`, `state()`, `manifest_json(now, snapshot) -> String`; `SyncEngine.receive()` zmienione tak, by zwracało `Option<LinkMessage>`, oraz nowe `SyncEngine.send(message)`

Poza `Recorder`em to zadanie naprawia drugą wadę fasady: `SyncEngine.receive()` zwraca `bool`, więc **komendy `startRecording` i `stopRecording` nigdy nie docierają do aplikacji** — slave nie miałby jak ruszyć. Zwracanie wiadomości rozwiązuje to jednym dekodowaniem. Wysyłka dostaje własną metodę, żeby aplikacja nie sięgała do transportu z pominięciem `PeerLink` — to on trasuje tryb dostarczenia, a ping wysłany kanałem niezawodnym psuje pomiar synchronizacji.

- [ ] **Step 1: Napisz testy, które mają nie przejść**

Dopisz na końcu `tests/api.rs`:

```rust
use std::sync::Arc;
use support::{FakeCapture, FakeStorage, FakeThermal};
use tracker_core::api::Recorder;
use tracker_core::clock::{ClockFit, SyncSnapshot};
use tracker_core::session::{
    CaptureProfile, DeviceInfo, MatchSetup, Platform, RecorderConfig, RecorderState, SessionRole,
};

fn recorder_snapshot() -> SyncSnapshot {
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

fn facade_recorder() -> Arc<Recorder> {
    Recorder::new(
        SessionRole::Master,
        Platform::Ios,
        DeviceInfo {
            model: "iPhone17,2".into(),
            os_version: "26.0".into(),
            app_version: "0.1.0".into(),
        },
        Some("iPhone15,4".into()),
        MatchSetup::default_singles(),
        capability_report(tracker_core::device::CapabilityProbeResult {
            manual_sensor: true,
            exposure_lock: true,
            focus_lock: true,
            timestamp_source: tracker_core::device::TimestampSource::Realtime,
            intrinsics_available: true,
            max_fps: 120,
            min_exposure_seconds: 0.000125,
        }),
        "multipeer".into(),
        Arc::new(FakeCapture::new()),
        Arc::new(FakeStorage::new()),
        Arc::new(FakeThermal::new()),
        RecorderConfig::default(),
    )
}

#[test]
fn facade_recorder_runs_a_full_session() {
    let recorder = facade_recorder();
    assert_eq!(recorder.state(), RecorderState::Idle);

    recorder.arm().unwrap();
    assert_eq!(recorder.state(), RecorderState::Armed);

    recorder.start("s-1".into(), CaptureProfile::P1080p120, 1000.0).unwrap();
    assert_eq!(recorder.state(), RecorderState::Recording);

    recorder.tick(1300.1);
    let manifest = recorder.stop(1450.0, recorder_snapshot()).unwrap();

    assert_eq!(manifest.session_id, "s-1");
    assert_eq!(manifest.segments.len(), 2);
    assert!(manifest.validate().is_ok());
}

#[test]
fn facade_recorder_serialises_the_manifest() {
    let recorder = facade_recorder();
    recorder.arm().unwrap();
    recorder.start("s-2".into(), CaptureProfile::P1080p120, 1000.0).unwrap();
    recorder.stop(1200.0, recorder_snapshot()).unwrap();

    let json = recorder.manifest_json(1200.0, recorder_snapshot()).unwrap();
    assert!(json.contains("\"sessionId\""));
    assert!(json.contains("s-2"));
}

#[test]
fn facade_recorder_accepts_marks() {
    use tracker_core::remote::{command_to_event, MarkTag, RemoteCommand};

    let recorder = facade_recorder();
    recorder.arm().unwrap();
    recorder.start("s-3".into(), CaptureProfile::P1080p120, 1000.0).unwrap();

    let event = command_to_event(&RemoteCommand::MarkMoment { tag: MarkTag::Changeover }, 1100.0)
        .expect("oczekiwano zdarzenia");
    recorder.note_event(event);

    let manifest = recorder.stop(1200.0, recorder_snapshot()).unwrap();
    assert!(manifest.events.iter().any(|e| e.tag.as_deref() == Some("changeover")));
}
```

- [ ] **Step 2: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cargo test --test api`
Expected: FAIL — `unresolved import tracker_core::api::Recorder`

- [ ] **Step 3: Wyeksportuj traity portów**

W `src/session/ports.rs` dodaj `#[uniffi::export(with_foreign)]` nad każdym z trzech traitów oraz **zmień `&str` na `String`** w `start_recording` — UniFFI z `with_foreign` wymaga typów własnych:

```rust
#[uniffi::export(with_foreign)]
pub trait CaptureControlling: Send + Sync {
    fn lock_settings(&self) -> Result<LockedCameraSettings, CaptureError>;
    fn start_recording(
        &self,
        session_id: String,
        profile: CaptureProfile,
    ) -> Result<(), CaptureError>;
    /// Zamyka biezacy segment i otwiera nastepny. Zwraca zamkniety segment.
    fn roll_segment(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    /// Zamyka biezacy segment i konczy nagrywanie.
    fn stop_recording(&self, now: f64) -> Result<SegmentInfo, CaptureError>;
    fn first_frame_host_time(&self) -> Option<f64>;
    fn intrinsic_matrix(&self) -> Option<Vec<Vec<f64>>>;
    fn lens_name(&self) -> String;
}

#[uniffi::export(with_foreign)]
pub trait StorageProbing: Send + Sync {
    fn free_bytes(&self) -> i64;
}

#[uniffi::export(with_foreign)]
pub trait ThermalProbing: Send + Sync {
    fn thermal_level(&self) -> ThermalLevel;
}
```

W `src/session/recorder.rs` popraw wywołanie na `self.capture.start_recording(session_id.to_string(), profile)`.

W `tests/support/mod.rs` popraw sygnaturę atrapy na `fn start_recording(&self, _session_id: String, _profile: CaptureProfile)`.

- [ ] **Step 4: Popraw `SyncEngine.receive` i dodaj `send`**

W `src/api.rs`, w bloku `#[uniffi::export] impl SyncEngine`, zamień `receive` i dopisz `send`:

```rust
    /// Wolane, gdy z sieci przyszedl pakiet.
    ///
    /// Zwraca wiadomosc, jesli pakiet byl poprawny i nie byl duplikatem.
    /// Pingi i pongi sa obsluzone po drodze; reszta wraca do aplikacji,
    /// bo to ona wie, co zrobic z komenda startu nagrywania.
    pub fn receive(&self, data: Vec<u8>, received_at: f64) -> Option<LinkMessage> {
        let message = self.link.receive(&data)?;
        self.session.lock().unwrap().handle(&message, received_at);
        Some(message)
    }

    /// Wysyla wiadomosc przez lacze.
    ///
    /// Tryb dostarczenia wybiera `PeerLink` na podstawie typu wiadomosci —
    /// aplikacja nie ma jak sie pomylic i wyslac pinga kanalem niezawodnym.
    pub fn send(&self, message: LinkMessage) -> Result<(), LinkError> {
        self.link.send(message)
    }
```

Dopisz `LinkError` do importów: `use crate::link::{LinkError, LinkMessage, PeerLink, PeerTransport};`

Popraw test `sync_engine_runs_a_full_burst_and_reports_quality` — `receive` zwraca teraz `Option`, więc asercje na `bool` przestają się kompilować. Dodaj też test:

```rust
#[test]
fn receive_returns_commands_to_the_caller() {
    let transport = Arc::new(FakeTransport::new());
    let engine = SyncEngine::new(transport.clone());

    let envelope = tracker_core::link::LinkEnvelope {
        sequence: 1,
        message: tracker_core::link::LinkMessage::StopRecording { host_time: 42.0 },
    };
    let bytes = tracker_core::link::encode(&envelope).unwrap();

    match engine.receive(bytes.clone(), 100.0) {
        Some(tracker_core::link::LinkMessage::StopRecording { host_time }) => {
            assert!((host_time - 42.0).abs() < 1e-9);
        }
        other => panic!("oczekiwano komendy, otrzymano {other:?}"),
    }
    // duplikat nie wraca po raz drugi
    assert!(engine.receive(bytes, 101.0).is_none());
}
```

- [ ] **Step 5: Dodaj obiekt `Recorder` do fasady**

Na końcu `src/api.rs`:

```rust
use crate::session::{
    CaptureControlling, CaptureProfile, DeviceInfo, Platform, RecorderConfig, RecorderError,
    RecorderState, SessionEvent, SessionRecorder, SessionRole, StorageProbing, ThermalProbing,
};

/// Maszyna stanow sesji widziana ze Swifta i Kotlina.
///
/// `SessionRecorder` wymaga `&mut self`, czego UniFFI nie eksportuje,
/// wiec siedzi tu w `Mutex` — dokladnie tak samo jak `SyncSession`
/// w `SyncEngine`.
#[derive(uniffi::Object)]
pub struct Recorder {
    inner: Mutex<SessionRecorder>,
}

#[uniffi::export]
impl Recorder {
    #[allow(clippy::too_many_arguments)]
    #[uniffi::constructor]
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
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(SessionRecorder::new(
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
            )),
        })
    }

    pub fn state(&self) -> RecorderState {
        self.inner.lock().unwrap().state()
    }

    pub fn arm(&self) -> Result<(), RecorderError> {
        self.inner.lock().unwrap().arm()
    }

    pub fn start(
        &self,
        session_id: String,
        profile: CaptureProfile,
        now: f64,
    ) -> Result<(), RecorderError> {
        self.inner.lock().unwrap().start(&session_id, profile, now)
    }

    pub fn tick(&self, now: f64) {
        self.inner.lock().unwrap().tick(now);
    }

    pub fn note_event(&self, event: SessionEvent) {
        self.inner.lock().unwrap().note_event(event);
    }

    pub fn note_peer_heartbeat(&self, at: f64) {
        self.inner.lock().unwrap().note_peer_heartbeat(at);
    }

    pub fn stop(
        &self,
        now: f64,
        snapshot: SyncSnapshot,
    ) -> Result<SessionManifest, RecorderError> {
        self.inner.lock().unwrap().stop(now, &snapshot)
    }

    /// Manifest gotowy do zapisania na dysk. Swift nie musi znac serde.
    pub fn manifest_json(
        &self,
        now: f64,
        snapshot: SyncSnapshot,
    ) -> Result<String, RecorderError> {
        let manifest = self.inner.lock().unwrap().build_manifest(now, &snapshot)?;
        manifest
            .encode()
            .map_err(|error| RecorderError::Manifest { reason: error.to_string() })
    }
}
```

- [ ] **Step 6: Uruchom cały pakiet**

Run: `cargo test`
Expected: PASS, wszystkie testy z części 1 plus 3 nowe

- [ ] **Step 7: Sprawdź, że nowe typy przechodzą przez FFI**

Run: `./scripts/build-bindings.sh`

Następnie:

```bash
grep -c "class Recorder" bindings/swift/tracker_core.swift
grep -c "protocol CaptureControlling" bindings/swift/tracker_core.swift
grep -c "protocol StorageProbing" bindings/swift/tracker_core.swift
grep -c "protocol ThermalProbing" bindings/swift/tracker_core.swift
```

Każde polecenie musi zwrócić wartość większą od zera.

- [ ] **Step 8: Commit**

```bash
cargo clippy -- -D warnings
cargo fmt
git add src tests
git commit -m "feat(api): eksport Recordera i traitow portow przez FFI"
```

---

### Task 2: xcframework i szkielet aplikacji iOS

**Files:**
- Create: `scripts/build-xcframework.sh`
- Create: `app-ios/TennisTrackerRig.xcodeproj` (przez Xcode)
- Create: `app-ios/TennisTrackerRig/TennisTrackerRigApp.swift`
- Create: `app-ios/TennisTrackerRig/Adapters/MonotonicClock.swift`
- Create: `app-ios/TennisTrackerRig/UI/RigView.swift`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `tracker_core.swift` wygenerowany w zadaniu 1
- Produces: `MonotonicClock` z `var now: Double`; aplikacja, która się buduje i uruchamia

- [ ] **Step 1: Napisz skrypt budujący xcframework**

`scripts/build-xcframework.sh`:

```bash
#!/usr/bin/env bash
# Buduje TrackerCore.xcframework z crate'u tracker-core i generuje
# bindingi Swift.
#
# Uzycie: ./scripts/build-xcframework.sh
set -euo pipefail

cd "$(dirname "$0")/.."

TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)

echo "==> instalacja targetow"
for target in "${TARGETS[@]}"; do
    rustup target add "$target"
done

echo "==> budowanie bibliotek"
for target in "${TARGETS[@]}"; do
    cargo build --release --target "$target"
done

echo "==> generowanie bindingow Swift"
cargo build --release
rm -rf bindings/swift
cargo run --features cli --bin uniffi-bindgen -- \
    generate --library target/release/libtracker_core.dylib \
    --language swift --out-dir bindings/swift

# UniFFI generuje modulemap pod nazwa pliku; xcframework wymaga
# dokladnie "module.modulemap".
mv bindings/swift/tracker_coreFFI.modulemap bindings/swift/module.modulemap

echo "==> skladanie xcframework"
rm -rf TrackerCore.xcframework
xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/libtracker_core.a \
    -headers bindings/swift \
    -library target/aarch64-apple-ios-sim/release/libtracker_core.a \
    -headers bindings/swift \
    -output TrackerCore.xcframework

echo "==> gotowe"
echo "Dodaj do projektu Xcode:"
echo "  - TrackerCore.xcframework (Frameworks, Libraries and Embedded Content)"
echo "  - bindings/swift/tracker_core.swift (jako plik zrodlowy)"
```

Nadaj prawa: `chmod +x scripts/build-xcframework.sh`

- [ ] **Step 2: Zbuduj xcframework**

Run: `./scripts/build-xcframework.sh`
Expected: powstaje `TrackerCore.xcframework/` oraz `bindings/swift/tracker_core.swift`

- [ ] **Step 3: Uzupełnij `.gitignore`**

Dopisz:

```
TrackerCore.xcframework/
app-ios/build/
```

- [ ] **Step 4: Utwórz projekt Xcode**

W Xcode: File → New → Project → iOS → App.
- Product Name: `TennisTrackerRig`
- Interface: SwiftUI, Language: Swift
- Zapisz w katalogu `app-ios/`

W ustawieniach targetu:
- Minimum Deployment: **iOS 17.0**
- Signing → Team: twój darmowy profil; Bundle Identifier na własny, np. `com.<twoje-id>.tennistrackerrig`

Dodaj do targetu:
- `TrackerCore.xcframework` w sekcji **Frameworks, Libraries and Embedded Content** (Do Not Embed — to biblioteka statyczna)
- `bindings/swift/tracker_core.swift` jako plik źródłowy (Add Files, **bez** kopiowania — ma się regenerować)

- [ ] **Step 5: Napisz zegar monotoniczny**

`app-ios/TennisTrackerRig/Adapters/MonotonicClock.swift`:

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
struct MonotonicClock {
    var now: Double { CACurrentMediaTime() }
}
```

- [ ] **Step 6: Napisz ekran tymczasowy sprawdzający, że rdzeń działa**

`app-ios/TennisTrackerRig/UI/RigView.swift`:

```swift
import SwiftUI

struct RigView: View {
    private let clock = MonotonicClock()

    var body: some View {
        VStack(spacing: 16) {
            Text("Tennis Tracker Rig")
                .font(.headline)

            // Wywolanie rdzenia dowodzi, ze xcframework i bindingi
            // sa poprawnie podpiete.
            Text(coreCheck)
                .font(.system(.body, design: .monospaced))
                .multilineTextAlignment(.center)

            Text(String(format: "clock: %.3f", clock.now))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding()
    }

    private var coreCheck: String {
        let setup = defaultMatchSetup()
        return "core OK\n\(setup.players.count) players\nserver: \(setup.firstServer)"
    }
}
```

`app-ios/TennisTrackerRig/TennisTrackerRigApp.swift`:

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

- [ ] **Step 7: Zbuduj i uruchom na urządzeniu**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

Zainstaluj na iPhonie i sprawdź, że ekran pokazuje `core OK`, `2 players`, `server: p1`.

To jest kamień milowy: **rdzeń Rust wykonuje się na telefonie**. Jeśli tu coś nie działa, dalsze zadania nie mają sensu.

- [ ] **Step 8: Commit**

```bash
git add scripts .gitignore app-ios
git commit -m "feat(ios): xcframework, szkielet aplikacji i zegar monotoniczny"
```

---

### Task 3: Transport MultipeerConnectivity i parowanie

**Files:**
- Create: `app-ios/TennisTrackerRig/Adapters/MultipeerTransport.swift`
- Create: `app-ios/TennisTrackerRig/Session/SessionCoordinator.swift`
- Modify: `app-ios/TennisTrackerRig/UI/RigView.swift`, `Info.plist`

**Interfaces:**
- Consumes: `PeerTransport`, `SyncEngine`, `Delivery`, `SessionRole` z rdzenia
- Produces: `MultipeerTransport: PeerTransport` z `init(role:)`, `start()`, `stop()`; `SessionCoordinator: ObservableObject` z `@Published isConnected`, `syncQualityMs`, `sampleCount`, `role`, oraz `connect()`

- [ ] **Step 1: Uzupełnij `Info.plist`**

W Xcode → target → Info dodaj:

```xml
<key>NSCameraUsageDescription</key>
<string>Records the match from a camera mounted on the court fence.</string>
<key>NSMicrophoneUsageDescription</key>
<string>Records impact sounds used to verify camera synchronisation.</string>
<key>NSMotionUsageDescription</key>
<string>Detects knocks to the phone mounted on the fence.</string>
<key>NSLocalNetworkUsageDescription</key>
<string>Connects to the second phone recording the match.</string>
<key>NSBonjourServices</key>
<array>
    <string>_ttrig._tcp</string>
    <string>_ttrig._udp</string>
</array>
```

Bez `NSBonjourServices` MultipeerConnectivity na iOS 14+ **nie wykryje peera**. To najczęstsza przyczyna „telefony się nie widzą".

- [ ] **Step 2: Napisz transport**

`app-ios/TennisTrackerRig/Adapters/MultipeerTransport.swift`:

```swift
import Foundation
import MultipeerConnectivity
import UIKit

/// Transport peer-to-peer po WiFi i Bluetooth, bez internetu i bez serwera.
///
/// Master oglasza usluge, slave jej szuka. Podzial jest arbitralny — chodzi
/// tylko o to, zeby obie strony nie probowaly zapraszac sie nawzajem.
///
/// Tryb dostarczenia wybiera rdzen: pingi ida `.unreliable`, komendy
/// `.reliable`. TCP z retransmisjami klamie o opoznieniu, wiec ping wyslany
/// niezawodnym kanalem zepsulby pomiar synchronizacji.
final class MultipeerTransport: NSObject, PeerTransport {

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

    /// Wolane, gdy z sieci przyszedl pakiet. Ustawiane przez koordynatora.
    var onReceive: ((Data) -> Void)?
    var onConnectionChange: ((Bool) -> Void)?

    private let connectedLock = NSLock()
    private var connected = false

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

    // MARK: PeerTransport

    func send(data: Data, delivery: Delivery) throws {
        guard !session.connectedPeers.isEmpty else {
            throw TransportError.SendFailed(reason: "no connected peers")
        }
        let mode: MCSessionSendDataMode = (delivery == .unreliable) ? .unreliable : .reliable
        do {
            try session.send(data, toPeers: session.connectedPeers, with: mode)
        } catch {
            throw TransportError.SendFailed(reason: error.localizedDescription)
        }
    }

    func isConnected() -> Bool {
        connectedLock.lock()
        defer { connectedLock.unlock() }
        return connected
    }

    private func setConnected(_ value: Bool) {
        connectedLock.lock()
        connected = value
        connectedLock.unlock()
    }
}

extension MultipeerTransport: MCSessionDelegate {
    func session(_ session: MCSession, peer: MCPeerID, didChange state: MCSessionState) {
        let isConnected = (state == .connected)
        setConnected(isConnected)
        DispatchQueue.main.async { self.onConnectionChange?(isConnected) }
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        // Bez skoku na glowny watek: znacznik odbioru musi byc pobrany
        // jak najblizej momentu przyjscia pakietu, inaczej kolejkowanie
        // zaklamie pomiar opoznienia.
        onReceive?(data)
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
    func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) {
        browser.invitePeer(peerID, to: session, withContext: nil, timeout: 15)
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {}
}
```

- [ ] **Step 3: Napisz koordynator**

`app-ios/TennisTrackerRig/Session/SessionCoordinator.swift`:

```swift
import Combine
import Foundation

/// Spina rdzen z adapterami sprzetowymi i wystawia stan do UI.
///
/// Zadna decyzja nie zapada tutaj — koordynator wyłacznie podaje rdzeniowi
/// odczyt zegara i przekazuje dalej to, co rdzen zwroci.
@MainActor
final class SessionCoordinator: ObservableObject {

    @Published private(set) var isConnected = false
    @Published private(set) var syncQualityMs: Double?
    @Published private(set) var sampleCount = 0
    @Published var role: SessionRole = .master

    private let clock = MonotonicClock()
    private var transport: MultipeerTransport?
    private var syncEngine: SyncEngine?
    private var ticker: Timer?

    func connect() {
        let transport = MultipeerTransport(role: role)
        let engine = SyncEngine(transport: transport)

        transport.onReceive = { [weak self] data in
            guard let self else { return }
            // Znacznik pobrany natychmiast po odbiorze pakietu — skok na
            // glowny watek przed odczytem zaklamalby pomiar opoznienia.
            let message = engine.receive(data: data, receivedAt: self.clock.now)
            guard let message else { return }
            Task { @MainActor in self.handle(command: message) }
        }
        transport.onConnectionChange = { [weak self] connected in
            guard let self else { return }
            self.isConnected = connected
            engine.linkDidChange(connected: connected, at: self.clock.now)
            if connected {
                engine.start(now: self.clock.now)
            }
        }

        self.transport = transport
        self.syncEngine = engine
        transport.start()
        startTicking()
    }

    func disconnect() {
        ticker?.invalidate()
        ticker = nil
        transport?.stop()
        transport = nil
        syncEngine = nil
        isConnected = false
        syncQualityMs = nil
        sampleCount = 0
    }

    private func startTicking() {
        ticker?.invalidate()
        ticker = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }

    private func tick() {
        guard let syncEngine else { return }
        syncEngine.tick(now: clock.now)
        syncQualityMs = syncEngine.residualMs()
        sampleCount = Int(syncEngine.snapshot().samples.count)
    }

    /// Komendy sterujace. W tym zadaniu jeszcze nic nie robia — pelna
    /// obsluga dochodzi w zadaniu 8, razem z nagrywaniem.
    private func handle(command: LinkMessage) {
        switch command {
        default:
            break
        }
    }
}
```

- [ ] **Step 4: Rozbuduj ekran główny**

`app-ios/TennisTrackerRig/UI/RigView.swift`:

```swift
import SwiftUI

struct RigView: View {
    @StateObject private var coordinator = SessionCoordinator()

    var body: some View {
        VStack(spacing: 24) {
            Picker("Role", selection: $coordinator.role) {
                Text("Master").tag(SessionRole.master)
                Text("Slave").tag(SessionRole.slave)
            }
            .pickerStyle(.segmented)
            .disabled(coordinator.isConnected)

            HStack {
                Circle()
                    .fill(coordinator.isConnected ? .green : .red)
                    .frame(width: 14, height: 14)
                Text(coordinator.isConnected ? "Connected" : "Not connected")
            }

            VStack(spacing: 4) {
                Text(syncText)
                    .font(.system(size: 40, weight: .semibold, design: .monospaced))
                    .foregroundStyle(syncColor)
                Text("\(coordinator.sampleCount) samples")
                    .font(.caption)
            }

            Button(coordinator.isConnected ? "Disconnect" : "Connect") {
                coordinator.isConnected ? coordinator.disconnect() : coordinator.connect()
            }
            .buttonStyle(.borderedProminent)

            Spacer()

            Text("App expires: \(Self.expiryText)")
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
        guard let path = Bundle.main.executablePath,
              let attributes = try? FileManager.default.attributesOfItem(atPath: path),
              let built = attributes[.creationDate] as? Date
        else { return "unknown" }

        let formatter = DateFormatter()
        formatter.dateFormat = "d MMM, HH:mm"
        return formatter.string(from: built.addingTimeInterval(7 * 24 * 3600))
    }
}
```

- [ ] **Step 5: Zbuduj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 6: Weryfikacja na dwóch telefonach**

Zainstaluj na obu iPhone'ach. Na 16 Pro Max wybierz Master, na 15 Slave, na obu naciśnij Connect.

Oczekiwane:
1. Kropka zielenieje na obu w ciągu kilku sekund.
2. Liczba próbek przekracza 50 niemal natychmiast (seria startowa).
3. Jakość synchronizacji ustala się **zielona, poniżej 2 ms**.
4. Wyłączenie WiFi na jednym telefonie: kropka czerwienieje na obu; po włączeniu wraca zielona i próbki znowu rosną.
5. Tryb samolotowy z włączonym Bluetooth: połączenie nadal działa, wolniej.

Jeśli telefony się nie widzą — sprawdź `NSBonjourServices` w `Info.plist` oraz zgodę na dostęp do sieci lokalnej w Ustawieniach.

Jeśli jakość jest gorsza niż 2 ms — sprawdź, czy `send` faktycznie mapuje `.unreliable`, bo ping po kanale niezawodnym zawyża i rozrzuca pomiar.

- [ ] **Step 7: Commit**

```bash
git add app-ios
git commit -m "feat(ios): transport MultipeerConnectivity i ekran parowania"
```

---

### Task 4: Raport zdolności urządzenia

**Files:**
- Create: `app-ios/TennisTrackerRig/Adapters/CapabilityProbe.swift`
- Create: `app-ios/TennisTrackerRig/UI/DiagnosticsView.swift`
- Modify: `app-ios/TennisTrackerRig/UI/RigView.swift`

**Interfaces:**
- Consumes: `CapabilityProbeResult`, `CapabilityReport`, `TimestampSource`, `capabilityReport(probe:)` z rdzenia
- Produces: `CapabilityProbe` z `static func probe() -> CapabilityProbeResult` oraz `static func report() -> CapabilityReport`; `DiagnosticsView` z eksportem raportu do pliku

Uzasadnienie: dostęp do Androida jest okazjonalny, z pożyczenia. Ten ekran ma sprawić, że **kwadrans z pożyczonym telefonem daje komplet twardych danych** zamiast wrażenia. Na iOS działa tak samo i od razu potwierdza, że odczyty są sensowne.

- [ ] **Step 1: Napisz sondę**

`app-ios/TennisTrackerRig/Adapters/CapabilityProbe.swift`:

```swift
import AVFoundation
import Foundation

/// Odczytuje zdolnosci kamery. Werdykt (poziom pracy) wylicza rdzen —
/// tutaj jest wylacznie odczyt wartosci z platformy.
enum CapabilityProbe {

    static func probe() -> CapabilityProbeResult {
        guard let device = AVCaptureDevice.default(
            .builtInWideAngleCamera, for: .video, position: .back
        ) else {
            return CapabilityProbeResult(
                manualSensor: false,
                exposureLock: false,
                focusLock: false,
                timestampSource: .unknown,
                intrinsicsAvailable: false,
                maxFps: 0,
                minExposureSeconds: 0
            )
        }

        let maxFps = device.formats
            .flatMap(\.videoSupportedFrameRateRanges)
            .map(\.maxFrameRate)
            .max() ?? 0

        let minExposure = device.formats
            .map { CMTimeGetSeconds($0.minExposureDuration) }
            .filter { $0 > 0 }
            .min() ?? 0

        return CapabilityProbeResult(
            manualSensor: device.isExposureModeSupported(.custom),
            exposureLock: device.isExposureModeSupported(.locked),
            focusLock: device.isFocusModeSupported(.locked),
            // AVFoundation zawsze wystawia znaczniki na zegarze hosta,
            // czyli tej samej bazie co CACurrentMediaTime.
            timestampSource: .realtime,
            intrinsicsAvailable: true,
            maxFps: UInt32(maxFps.rounded()),
            minExposureSeconds: minExposure
        )
    }

    static func report() -> CapabilityReport {
        capabilityReport(probe: probe())
    }
}
```

- [ ] **Step 2: Napisz ekran diagnostyczny**

`app-ios/TennisTrackerRig/UI/DiagnosticsView.swift`:

```swift
import SwiftUI

struct DiagnosticsView: View {
    @State private var report = CapabilityProbe.report()
    @State private var exported: URL?

    var body: some View {
        List {
            Section("Verdict") {
                row("Tier", tierText)
            }
            Section("Camera") {
                row("Manual sensor", report.manualSensor ? "yes" : "no")
                row("Max fps", "\(report.maxFps)")
                row("Min exposure", String(format: "1/%.0f s", 1 / max(report.minExposureSeconds, 1e-9)))
                row("Intrinsics", report.intrinsicsAvailable ? "yes" : "no")
            }
            Section("Timing") {
                row("Timestamp source", report.timestampSource == .realtime ? "REALTIME" : "UNKNOWN")
                row("Base offset", report.timestampBaseOffsetMs.map { String(format: "%.2f ms", $0) } ?? "n/a")
            }
            Section {
                Button("Export report") { export() }
            }
        }
        .navigationTitle("Diagnostics")
        .sheet(item: $exported) { url in ShareSheet(items: [url]) }
    }

    private var tierText: String {
        switch report.tier {
        case .full: return "full"
        case .limited: return "limited"
        case .rejected: return "rejected"
        }
    }

    private func row(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
            Spacer()
            Text(value).foregroundStyle(.secondary).font(.system(.body, design: .monospaced))
        }
    }

    /// Raport ma opuscic telefon jednym dotknieciem — przy sprzecie
    /// z pozyczenia liczy sie kazda minuta.
    private func export() {
        let device = UIDevice.current
        let text = """
        {
          "deviceModel": "\(device.modelIdentifier)",
          "osVersion": "\(device.systemVersion)",
          "tier": "\(tierText)",
          "manualSensor": \(report.manualSensor),
          "timestampSource": "\(report.timestampSource == .realtime ? "REALTIME" : "UNKNOWN")",
          "intrinsicsAvailable": \(report.intrinsicsAvailable),
          "maxFps": \(report.maxFps),
          "minExposureSeconds": \(report.minExposureSeconds)
        }
        """
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("capability-\(device.modelIdentifier).json")
        try? text.write(to: url, atomically: true, encoding: .utf8)
        exported = url
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
```

Dodaj `import UIKit` na górze pliku.

- [ ] **Step 3: Podłącz do ekranu głównego**

Owiń zawartość `RigView` w `NavigationStack` i dodaj przed `Spacer()`:

```swift
            NavigationLink("Diagnostics") {
                DiagnosticsView()
            }
```

- [ ] **Step 4: Zbuduj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 5: Weryfikacja na urządzeniu**

1. Ekran Diagnostics pokazuje `tier: full` na obu iPhone'ach.
2. `Max fps` wynosi co najmniej 120 na 16 Pro Max.
3. `Min exposure` jest krótszy niż 1/1000 s.
4. `Timestamp source` to REALTIME.
5. Export report tworzy plik JSON, który da się wysłać AirDropem.

Jeśli `tier` jest inny niż `full`, zanotuj który odczyt zawiódł — to informacja dla P0b, nie błąd.

- [ ] **Step 6: Commit**

```bash
git add app-ios
git commit -m "feat(ios): sonda zdolnosci urzadzenia i ekran diagnostyczny"
```

---

### Task 5: CaptureRig — kamera z zablokowanymi parametrami

Najbardziej wrażliwa część planu. Trzy rzeczy decydują o tym, czy nagranie da się w P1 do czegokolwiek użyć.

**Files:**
- Create: `app-ios/TennisTrackerRig/Adapters/CaptureRig.swift`
- Create: `app-ios/TennisTrackerRig/Session/SessionStore.swift`

**Interfaces:**
- Consumes: `CaptureControlling`, `CaptureProfile`, `LockedCameraSettings`, `WhiteBalanceGains`, `SegmentInfo`, `CaptureError`, `SessionEvent` z rdzenia
- Produces: `SessionStore` z `makeSessionDirectory(sessionId:) throws -> URL`, `write(manifestJson:to:) throws`, `var sessionDirectories: [URL]`; `CaptureRig: CaptureControlling` z `init(store:)`, `configure(profile:) throws`, `startSession()`, `var previewLayer: AVCaptureVideoPreviewLayer`, `var onEvent: ((SessionEvent) -> Void)?`

- [ ] **Step 1: Napisz magazyn sesji**

`app-ios/TennisTrackerRig/Session/SessionStore.swift`:

```swift
import Foundation

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

    /// Manifest przychodzi z rdzenia juz jako JSON — Swift nie serializuje
    /// go samodzielnie, zeby nie powstala druga, rozjezdzajaca sie definicja.
    func write(manifestJson: String, to directory: URL) throws {
        try manifestJson.write(
            to: directory.appendingPathComponent("manifest.json"),
            atomically: true,
            encoding: .utf8
        )
    }

    var sessionDirectories: [URL] {
        let contents = try? FileManager.default.contentsOfDirectory(
            at: root, includingPropertiesForKeys: nil
        )
        return (contents ?? [])
            .filter { $0.lastPathComponent.hasPrefix("session-") }
            .sorted { $0.lastPathComponent > $1.lastPathComponent }
    }
}
```

- [ ] **Step 2: Napisz `CaptureRig`**

`app-ios/TennisTrackerRig/Adapters/CaptureRig.swift`:

```swift
import AVFoundation
import Foundation
import QuartzCore

/// Kamera i mikrofon z parametrami zablokowanymi na sztywno.
///
/// Trzy rzeczy decyduja o przydatnosci nagrania w P1:
///
/// 1. Ekspozycja, ISO, ostrosc i balans bieli sa zablokowane. Autoekspozycja
///    polujaca w trakcie wymiany daje skoki jasnosci miedzy klatkami, co psuje
///    detekcje i pozniejsze roznicowanie klatek.
/// 2. Czas naswietlania jest krotki. Pilka przy 40 m/s w 1/1000 s przesuwa sie
///    4 cm, czyli mniej niz wlasna srednica, i zostaje kropka. Przy 1/250 s
///    rozmazuje sie w kreske dlugosci dwoch i pol pilki.
/// 3. Bitrate jest wysoki. Kompresja traktuje siedmiopikselowa szybka plamke
///    jak szum i usuwa ja jako pierwsza.
final class CaptureRig: NSObject, CaptureControlling {

    /// Docelowy czas naswietlania.
    private let targetExposure = CMTime(value: 1, timescale: 1000)
    private let targetBitrate = 40_000_000

    let session = AVCaptureSession()

    /// Zdarzenia sprzetowe trafiajace do manifestu.
    var onEvent: ((SessionEvent) -> Void)?

    private let store: SessionStore
    private let queue = DispatchQueue(label: "capture.rig")
    private let stateLock = NSLock()

    private var device: AVCaptureDevice?
    private var videoOutput: AVCaptureVideoDataOutput?
    private var audioOutput: AVCaptureAudioDataOutput?
    private var writer: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var audioInput: AVAssetWriterInput?
    private var directory: URL?
    private var profile: CaptureProfile = .p1080p120
    private var segmentIndex = 0
    private var segmentStartHostTime: Double = 0
    private var segmentFrameCount: UInt64 = 0
    private var recording = false

    private var storedFirstFrame: Double?
    private var storedIntrinsics: [[Double]]?
    private var lens = "builtInWideAngleCamera"

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
        ) else {
            session.commitConfiguration()
            throw CaptureError.NoCamera
        }
        device = camera

        guard let format = bestFormat(for: profile, on: camera) else {
            session.commitConfiguration()
            throw CaptureError.ProfileUnavailable(profile: String(describing: profile))
        }

        try camera.lockForConfiguration()
        camera.activeFormat = format
        let duration = CMTime(value: 1, timescale: CMTimeScale(profile.fps()))
        camera.activeVideoMinFrameDuration = duration
        camera.activeVideoMaxFrameDuration = duration
        camera.unlockForConfiguration()

        let videoDeviceInput = try AVCaptureDeviceInput(device: camera)
        if session.canAddInput(videoDeviceInput) { session.addInput(videoDeviceInput) }

        guard let microphone = AVCaptureDevice.default(for: .audio) else {
            session.commitConfiguration()
            throw CaptureError.Other(reason: "no microphone")
        }
        let audioDeviceInput = try AVCaptureDeviceInput(device: microphone)
        if session.canAddInput(audioDeviceInput) { session.addInput(audioDeviceInput) }

        let video = AVCaptureVideoDataOutput()
        video.alwaysDiscardsLateVideoFrames = false
        video.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(video) { session.addOutput(video) }
        videoOutput = video

        // Macierz intrinsics per klatka. To gotowa kalibracja wewnetrzna
        // kamery, ktora w P1 oszczedza calej procedury z szachownica.
        if let connection = video.connection(with: .video),
           connection.isCameraIntrinsicMatrixDeliverySupported {
            connection.isCameraIntrinsicMatrixDeliveryEnabled = true
        }

        let audio = AVCaptureAudioDataOutput()
        audio.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(audio) { session.addOutput(audio) }
        audioOutput = audio

        session.commitConfiguration()
        observeInterruptions()
    }

    private func bestFormat(for profile: CaptureProfile, on device: AVCaptureDevice) -> AVCaptureDevice.Format? {
        device.formats.first { format in
            let dimensions = CMVideoFormatDescriptionGetDimensions(format.formatDescription)
            guard dimensions.width == Int32(profile.width()),
                  dimensions.height == Int32(profile.height()) else { return false }
            return format.videoSupportedFrameRateRanges.contains {
                $0.maxFrameRate >= Double(profile.fps())
            }
        }
    }

    func startSession() {
        queue.async { [weak self] in self?.session.startRunning() }
    }

    /// Przerwanie sesji (polaczenie przychodzace, presja systemowa) trafia
    /// do manifestu, a wznowienie jest po naszej stronie.
    private func observeInterruptions() {
        let center = NotificationCenter.default
        center.addObserver(
            forName: .AVCaptureSessionWasInterrupted, object: session, queue: .main
        ) { [weak self] notification in
            let raw = notification.userInfo?[AVCaptureSessionInterruptionReasonKey] as? Int
            self?.onEvent?(SessionEvent.captureInterrupted(
                hostTime: CACurrentMediaTime(),
                reason: raw.map(String.init) ?? "unknown"
            ))
        }
        center.addObserver(
            forName: .AVCaptureSessionInterruptionEnded, object: session, queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            self.onEvent?(SessionEvent.captureResumed(hostTime: CACurrentMediaTime()))
            self.queue.async { self.session.startRunning() }
        }
    }

    // MARK: CaptureControlling

    func lockSettings() throws -> LockedCameraSettings {
        guard let device else { throw CaptureError.NoCamera }

        try device.lockForConfiguration()
        defer { device.unlockForConfiguration() }

        let exposure = max(targetExposure, device.activeFormat.minExposureDuration)
        device.setExposureModeCustom(duration: exposure, iso: AVCaptureDevice.currentISO)
        if device.isFocusModeSupported(.locked) { device.focusMode = .locked }
        if device.isWhiteBalanceModeSupported(.locked) { device.whiteBalanceMode = .locked }

        let gains = device.deviceWhiteBalanceGains
        return LockedCameraSettings(
            exposureDurationSeconds: CMTimeGetSeconds(device.exposureDuration),
            iso: Double(device.iso),
            focusLensPosition: Double(device.lensPosition),
            whiteBalanceGains: WhiteBalanceGains(
                r: Double(gains.redGain),
                g: Double(gains.greenGain),
                b: Double(gains.blueGain)
            )
        )
    }

    func startRecording(sessionId: String, profile: CaptureProfile) throws {
        let directory = try store.makeSessionDirectory(sessionId: sessionId)
        stateLock.lock()
        self.directory = directory
        self.profile = profile
        segmentIndex = 0
        segmentFrameCount = 0
        storedFirstFrame = nil
        stateLock.unlock()

        try openWriter()
        stateLock.lock(); recording = true; stateLock.unlock()
    }

    func rollSegment(now: Double) throws -> SegmentInfo {
        let closed = try closeWriter(now: now)
        stateLock.lock()
        segmentIndex += 1
        segmentFrameCount = 0
        stateLock.unlock()
        try openWriter()
        return closed
    }

    func stopRecording(now: Double) throws -> SegmentInfo {
        stateLock.lock(); recording = false; stateLock.unlock()
        return try closeWriter(now: now)
    }

    func firstFrameHostTime() -> Double? {
        stateLock.lock(); defer { stateLock.unlock() }
        return storedFirstFrame
    }

    func intrinsicMatrix() -> [[Double]]? {
        stateLock.lock(); defer { stateLock.unlock() }
        return storedIntrinsics
    }

    func lensName() -> String { lens }

    // MARK: zapis

    private func openWriter() throws {
        stateLock.lock()
        guard let directory else { stateLock.unlock(); throw CaptureError.NotRecording }
        let index = segmentIndex
        let currentProfile = profile
        stateLock.unlock()

        let url = directory.appendingPathComponent(String(format: "video-%03d.mov", index))
        let writer: AVAssetWriter
        do {
            writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        } catch {
            throw CaptureError.Other(reason: error.localizedDescription)
        }

        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.hevc,
            AVVideoWidthKey: Int(currentProfile.width()),
            AVVideoHeightKey: Int(currentProfile.height()),
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: targetBitrate,
                AVVideoExpectedSourceFrameRateKey: Int(currentProfile.fps())
            ]
        ]
        let video = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        video.expectsMediaDataInRealTime = true
        if writer.canAdd(video) { writer.add(video) }

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
        let audio = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
        audio.expectsMediaDataInRealTime = true
        if writer.canAdd(audio) { writer.add(audio) }

        stateLock.lock()
        self.writer = writer
        self.videoInput = video
        self.audioInput = audio
        stateLock.unlock()
    }

    private func closeWriter(now: Double) throws -> SegmentInfo {
        stateLock.lock()
        guard let writer, let videoInput, let audioInput else {
            stateLock.unlock()
            throw CaptureError.NotRecording
        }
        let file = writer.outputURL.lastPathComponent
        let start = segmentStartHostTime
        let frames = segmentFrameCount
        stateLock.unlock()

        videoInput.markAsFinished()
        audioInput.markAsFinished()

        let semaphore = DispatchSemaphore(value: 0)
        writer.finishWriting { semaphore.signal() }
        semaphore.wait()

        stateLock.lock()
        self.writer = nil
        self.videoInput = nil
        self.audioInput = nil
        stateLock.unlock()

        return SegmentInfo(
            file: file,
            startHostTime: start,
            endHostTime: now,
            frameCount: frames
        )
    }
}

extension CaptureRig: AVCaptureVideoDataOutputSampleBufferDelegate,
                      AVCaptureAudioDataOutputSampleBufferDelegate {

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        stateLock.lock()
        let isRecording = recording
        let writer = self.writer
        stateLock.unlock()

        guard isRecording, let writer else { return }

        let isVideo = output is AVCaptureVideoDataOutput
        let timestamp = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        if writer.status == .unknown {
            writer.startWriting()
            writer.startSession(atSourceTime: timestamp)
            stateLock.lock()
            segmentStartHostTime = CMTimeGetSeconds(timestamp)
            if storedFirstFrame == nil { storedFirstFrame = segmentStartHostTime }
            stateLock.unlock()
        }
        guard writer.status == .writing else { return }

        if isVideo {
            captureIntrinsicsIfNeeded(from: sampleBuffer)
            stateLock.lock()
            let input = videoInput
            stateLock.unlock()
            if input?.isReadyForMoreMediaData == true {
                input?.append(sampleBuffer)
                stateLock.lock(); segmentFrameCount += 1; stateLock.unlock()
            }
        } else {
            stateLock.lock()
            let input = audioInput
            stateLock.unlock()
            if input?.isReadyForMoreMediaData == true {
                input?.append(sampleBuffer)
            }
        }
    }

    private func captureIntrinsicsIfNeeded(from sampleBuffer: CMSampleBuffer) {
        stateLock.lock()
        let already = storedIntrinsics != nil
        stateLock.unlock()
        guard !already else { return }

        guard let attachment = CMGetAttachment(
            sampleBuffer,
            key: kCMSampleBufferAttachmentKey_CameraIntrinsicMatrix,
            attachmentModeOut: nil
        ) as? Data else { return }

        let matrix: matrix_float3x3 = attachment.withUnsafeBytes {
            $0.load(as: matrix_float3x3.self)
        }
        // matrix_float3x3 jest kolumnowa; manifest oczekuje wierszy.
        let rows = (0..<3).map { row in (0..<3).map { column in Double(matrix[column][row]) } }

        stateLock.lock(); storedIntrinsics = rows; stateLock.unlock()
    }
}
```

- [ ] **Step 3: Zbuduj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 4: Weryfikacja na urządzeniu**

Tymczasowo wywołaj z `RigView.onAppear`: `try? rig.configure(profile: .p1080p120); rig.startSession(); let locked = try? rig.lockSettings(); print(locked)`.

1. `configure` nie rzuca `ProfileUnavailable`. Jeśli rzuca na iPhone 15, sprawdź `.p1080p60` i **odnotuj to w README** — profil jest parametrem sesji właśnie po to.
2. W logu `exposureDurationSeconds` wynosi około 0,001, `iso` jest w rozsądnym zakresie, `focusLensPosition` stałe.
3. **Test blokady:** skieruj telefon na zmieniające się oświetlenie (zapal i zgaś lampę). Obraz **nie może** się rozjaśniać ani ściemniać. To jedyny dowód, że blokada faktycznie działa.
4. Zadzwoń na telefon w trakcie podglądu — po odrzuceniu połączenia podgląd wraca sam, a `onEvent` zgłasza `capture-interrupted`, potem `capture-resumed`.

- [ ] **Step 5: Commit**

```bash
git add app-ios
git commit -m "feat(ios): kamera z blokada parametrow, audio PCM i zapis segmentowany"
```

---

### Task 6: Podgląd, poziomica i sondy systemowe

**Files:**
- Create: `app-ios/TennisTrackerRig/UI/PreviewView.swift`
- Create: `app-ios/TennisTrackerRig/Adapters/SystemProbes.swift`
- Create: `app-ios/TennisTrackerRig/Adapters/MotionRecorder.swift`

**Interfaces:**
- Consumes: `StorageProbing`, `ThermalProbing`, `ThermalLevel`, `MotionAnalyzer`, `MotionSample`, `MotionVector`, `SessionEvent` z rdzenia
- Produces: `PreviewView` (UIViewRepresentable), `LevelMeter: ObservableObject` z `rollDegrees`; `SystemStorage: StorageProbing`, `SystemThermal: ThermalProbing`; `MotionRecorder` z `start(directory:onSpike:) throws`, `stop()`

- [ ] **Step 1: Napisz sondy systemowe**

`app-ios/TennisTrackerRig/Adapters/SystemProbes.swift`:

```swift
import Foundation

final class SystemStorage: StorageProbing {
    func freeBytes() -> Int64 {
        let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let values = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        return values?.volumeAvailableCapacityForImportantUsage ?? 0
    }
}

final class SystemThermal: ThermalProbing {
    func thermalLevel() -> ThermalLevel {
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

`app-ios/TennisTrackerRig/Adapters/MotionRecorder.swift`:

```swift
import CoreMotion
import Foundation
import QuartzCore

/// Zapisuje zyroskop do motion.jsonl i zglasza szarpniecia.
///
/// Log sluzy w P1 do oceny, jak bardzo kamera dryfowala na ogrodzeniu,
/// a wykryte skoki sa markerami "tutaj przelicz homografie od nowa".
final class MotionRecorder {

    private let motion = CMMotionManager()
    private var handle: FileHandle?
    private var analyzer = MotionAnalyzer()

    func start(directory: URL, onSpike: @escaping (SessionEvent) -> Void) throws {
        let url = directory.appendingPathComponent("motion.jsonl")
        FileManager.default.createFile(atPath: url.path, contents: nil)
        handle = try FileHandle(forWritingTo: url)

        guard motion.isDeviceMotionAvailable else { return }
        motion.deviceMotionUpdateInterval = 0.01
        motion.startDeviceMotionUpdates(to: .main) { [weak self] data, _ in
            guard let self, let data else { return }
            let sample = MotionSample(
                hostTime: CACurrentMediaTime(),
                rotationRate: MotionVector(
                    x: data.rotationRate.x,
                    y: data.rotationRate.y,
                    z: data.rotationRate.z
                )
            )
            self.append(sample, gravity: data.gravity)
            if let event = self.analyzer.process(sample: sample) {
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

Uwaga: `MotionAnalyzer` z rdzenia wymaga `&mut self`, więc wygenerowany Swift wystawia go jako obiekt. Jeśli `analyzer.process(sample:)` nie kompiluje się na `var`, użyj `let analyzer = MotionAnalyzer()` — UniFFI opakowuje mutowalność wewnątrz.

- [ ] **Step 3: Napisz podgląd z poziomicą**

`app-ios/TennisTrackerRig/UI/PreviewView.swift`:

```swift
import AVFoundation
import CoreMotion
import SwiftUI

/// Podglad kamery. Wskazowka kadrowania: kort z marginesem 1-2 m za liniami
/// ma wypelnic kadr, bez zapasu na niebo — apogeum loba i tak nie zmiesci sie
/// w kadrze, a decyduje kozol, ktory lezy na ziemi.
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

/// Telefon wiszacy krzywo zaniza dokladnosc kalibracji w P1.
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

- [ ] **Step 4: Wstaw podgląd do ekranu głównego**

W `RigView` dodaj `@StateObject private var levelMeter = LevelMeter()` i nad przełącznikiem roli:

```swift
            PreviewView(layer: coordinator.previewLayer)
                .frame(height: 220)
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay(alignment: .center) {
                    Rectangle()
                        .fill(abs(levelMeter.rollDegrees) < 2 ? Color.green : Color.orange)
                        .frame(height: 2)
                        .rotationEffect(.degrees(levelMeter.rollDegrees))
                }
                .overlay(alignment: .bottomLeading) {
                    Text("Court + 1-2 m margin should fill the frame")
                        .font(.caption2)
                        .padding(6)
                }
```

oraz `.onAppear { levelMeter.start() }`.

W `SessionCoordinator` dodaj `import AVFoundation` i:

```swift
    let store = SessionStore()
    lazy var captureRig = CaptureRig(store: store)
    var previewLayer: AVCaptureVideoPreviewLayer { captureRig.previewLayer }

    func prepareCamera(profile: CaptureProfile = .p1080p120) {
        do {
            try captureRig.configure(profile: profile)
            captureRig.startSession()
        } catch {
            // Profil niedostepny nie jest bledem krytycznym — UI pokaze
            // to na ekranie diagnostycznym.
            print("camera configure failed: \(error)")
        }
    }
```

Wywołaj `prepareCamera()` w `connect()`.

- [ ] **Step 5: Zbuduj i zweryfikuj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

Na urządzeniu: podgląd działa, kreska poziomicy jest zielona przy pionowym trzymaniu i pomarańczowa po przechyleniu o więcej niż 2 stopnie.

- [ ] **Step 6: Commit**

```bash
git add app-ios
git commit -m "feat(ios): podglad z poziomica, sondy systemowe i zapis ruchu"
```

---

### Task 7: Lista graczy i ustawienie meczu

**Files:**
- Create: `app-ios/TennisTrackerRig/Session/PlayerRoster.swift`
- Create: `app-ios/TennisTrackerRig/UI/MatchSetupView.swift`
- Modify: `app-ios/TennisTrackerRig/UI/RigView.swift`

**Interfaces:**
- Consumes: `MatchSetup`, `Player`, `CourtEnd`, `MatchFormat`, `defaultMatchSetup()` z rdzenia
- Produces: `RosterEntry: Codable, Identifiable`; `PlayerRoster: ObservableObject` z `entries`, `add(name:genitive:)`, `remove(at:)`; `MatchSetupView` z bindingiem do `MatchSetup`

Ustawienie odbywa się **przed zawieszeniem telefonu**, bo potem nie da się go zdjąć. Imiona nie mogą blokować startu — domyślne `defaultMatchSetup()` pozwala nagrywać od razu.

- [ ] **Step 1: Napisz listę graczy**

`app-ios/TennisTrackerRig/Session/PlayerRoster.swift`:

```swift
import Combine
import Foundation

/// Wpis w trwalej liscie graczy.
///
/// `genitive` to dopelniacz imienia, uzywany przez silnik oglosen w P2:
/// "punkt dla Piotra". Polskiej odmiany nie da sie wyliczyc algorytmicznie
/// — "Marek" daje "Marka", z wypadnieciem e. Pole jest opcjonalne.
struct RosterEntry: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var genitive: String?
}

/// Grasz z tymi samymi kilkoma osobami, wiec imie wpisujesz raz w zyciu.
@MainActor
final class PlayerRoster: ObservableObject {

    @Published private(set) var entries: [RosterEntry] = []

    private let url: URL

    init() {
        url = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("roster.json")
        load()
    }

    func add(name: String, genitive: String?) {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        let entry = RosterEntry(
            id: UUID().uuidString.prefix(8).lowercased(),
            name: trimmed,
            genitive: genitive?.trimmingCharacters(in: .whitespaces).nilIfEmpty
        )
        entries.append(entry)
        save()
    }

    func remove(at offsets: IndexSet) {
        entries.remove(atOffsets: offsets)
        save()
    }

    private func load() {
        guard let data = try? Data(contentsOf: url),
              let decoded = try? JSONDecoder().decode([RosterEntry].self, from: data)
        else { return }
        entries = decoded
    }

    private func save() {
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true
        )
        guard let data = try? JSONEncoder().encode(entries) else { return }
        try? data.write(to: url, options: .atomic)
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}
```

- [ ] **Step 2: Napisz ekran ustawienia meczu**

`app-ios/TennisTrackerRig/UI/MatchSetupView.swift`:

```swift
import SwiftUI

struct MatchSetupView: View {
    @StateObject private var roster = PlayerRoster()
    @Binding var setup: MatchSetup

    @State private var newName = ""
    @State private var newGenitive = ""

    var body: some View {
        List {
            Section("Players") {
                ForEach(Array(setup.players.enumerated()), id: \.offset) { index, player in
                    Picker(endLabel(player.startEnd), selection: binding(for: index)) {
                        Text("Player \(index + 1)").tag(String?.none)
                        ForEach(roster.entries) { entry in
                            Text(entry.name).tag(String?.some(entry.id))
                        }
                    }
                }
            }

            Section("First server") {
                Picker("Serves first", selection: $setup.firstServer) {
                    ForEach(setup.players, id: \.id) { player in
                        Text(player.name).tag(player.id)
                    }
                }
                .pickerStyle(.segmented)
            }

            Section("Roster") {
                ForEach(roster.entries) { entry in
                    VStack(alignment: .leading) {
                        Text(entry.name)
                        if let genitive = entry.genitive {
                            Text(genitive).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                .onDelete { roster.remove(at: $0) }

                HStack {
                    TextField("Name", text: $newName)
                    TextField("Genitive (optional)", text: $newGenitive)
                    Button("Add") {
                        roster.add(name: newName, genitive: newGenitive.nilIfEmpty)
                        newName = ""
                        newGenitive = ""
                    }
                    .disabled(newName.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .navigationTitle("Match")
    }

    private func endLabel(_ end: CourtEnd) -> String {
        end == .north ? "North end" : "South end"
    }

    /// Podmiana gracza zachowuje przypisanie do konca kortu — koniec jest
    /// wlasnosc pozycji, nie osoby.
    private func binding(for index: Int) -> Binding<String?> {
        Binding(
            get: { roster.entries.first { $0.name == setup.players[index].name }?.id },
            set: { newId in
                guard let entry = roster.entries.first(where: { $0.id == newId }) else { return }
                let wasServer = setup.firstServer == setup.players[index].id
                setup.players[index] = Player(
                    id: entry.id,
                    name: entry.name,
                    genitive: entry.genitive,
                    startEnd: setup.players[index].startEnd
                )
                if wasServer { setup.firstServer = entry.id }
            }
        )
    }
}
```

- [ ] **Step 3: Podłącz do ekranu głównego**

W `SessionCoordinator` dodaj `@Published var matchSetup: MatchSetup = defaultMatchSetup()`.

W `RigView` dodaj `NavigationLink("Match setup") { MatchSetupView(setup: $coordinator.matchSetup) }` oraz podpis pod nim:

```swift
            Text("\(coordinator.matchSetup.players[0].name) vs \(coordinator.matchSetup.players[1].name)")
                .font(.caption)
                .foregroundStyle(.secondary)
```

- [ ] **Step 4: Zbuduj i zweryfikuj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

Na urządzeniu:
1. Domyślnie widać `Player 1 vs Player 2` — nagrywanie jest możliwe bez wpisywania czegokolwiek.
2. Dodanie gracza do listy i wybranie go zmienia podpis na ekranie głównym.
3. Lista przeżywa restart aplikacji.
4. Zmiana gracza, który serwuje, aktualizuje `firstServer`.

- [ ] **Step 5: Commit**

```bash
git add app-ios
git commit -m "feat(ios): trwala lista graczy i ekran ustawienia meczu"
```

---

### Task 8: Pełny przebieg sesji

Spina wszystko: master startuje, slave podąża, manifest ląduje na dysku po obu stronach.

**Files:**
- Modify: `app-ios/TennisTrackerRig/Session/SessionCoordinator.swift`, `app-ios/TennisTrackerRig/UI/RigView.swift`

**Interfaces:**
- Consumes: `Recorder`, `SyncEngine`, `CaptureRig`, `MotionRecorder`, `SystemStorage`, `SystemThermal`, `CapabilityProbe`, `SessionStore` z wcześniejszych zadań
- Produces: `SessionCoordinator` rozszerzony o `isRecording`, `elapsedSeconds`, `freeGigabytes`, `thermalState`, `startRecording()`, `stopRecording()`

- [ ] **Step 1: Rozszerz koordynator**

Dodaj do `SessionCoordinator`:

```swift
    @Published private(set) var isRecording = false
    @Published private(set) var elapsedSeconds: Double = 0
    @Published private(set) var freeGigabytes: Double = 0
    @Published private(set) var thermalState = "nominal"

    private let storageProbe = SystemStorage()
    private let thermalProbe = SystemThermal()
    private let motionRecorder = MotionRecorder()
    private var recorder: Recorder?
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

    func startRecording(profile: CaptureProfile = .p1080p120) {
        let sessionId = makeSessionId()
        beginRecording(sessionId: sessionId, profile: profile)
        // Tryb dostarczenia wybiera PeerLink w rdzeniu — komendy ida
        // kanalem niezawodnym, pingi nie.
        try? syncEngine?.send(message: .startRecording(
            sessionId: sessionId, profile: profile, hostTime: clock.now
        ))
    }

    func stopRecording() {
        try? syncEngine?.send(message: .stopRecording(hostTime: clock.now))
        finishRecording()
    }

    private func beginRecording(sessionId: String, profile: CaptureProfile) {
        let recorder = Recorder(
            role: role,
            platform: .ios,
            device: DeviceInfo(
                model: UIDevice.current.modelIdentifier,
                osVersion: UIDevice.current.systemVersion,
                appVersion: Bundle.main.appVersion
            ),
            peerDeviceModel: nil,
            matchSetup: matchSetup,
            capabilities: CapabilityProbe.report(),
            transport: "multipeer",
            capture: captureRig,
            storage: storageProbe,
            thermal: thermalProbe,
            config: RecorderConfig(
                segmentSeconds: 300,
                minimumFreeBytes: 500_000_000,
                peerSilenceTimeout: 120,
                targetBitrateMbps: 40
            )
        )

        do {
            try recorder.arm()
            try recorder.start(sessionId: sessionId, profile: profile, now: clock.now)

            let directory = try store.makeSessionDirectory(sessionId: sessionId)
            sessionDirectory = directory
            try motionRecorder.start(directory: directory) { [weak recorder] event in
                recorder?.noteEvent(event: event)
            }
            captureRig.onEvent = { [weak recorder] event in
                recorder?.noteEvent(event: event)
            }

            self.recorder = recorder
            recordingStarted = clock.now
            isRecording = true
        } catch {
            // Nie da sie uzbroic — najczestsza przyczyna to brak miejsca.
            self.recorder = nil
            isRecording = false
        }
    }

    private func finishRecording() {
        guard let recorder, let sessionDirectory, let syncEngine else { return }
        motionRecorder.stop()

        let snapshot = syncEngine.snapshot()
        if let json = try? recorder.manifestJson(now: clock.now, snapshot: snapshot) {
            try? store.write(manifestJson: json, to: sessionDirectory)
        }
        _ = try? recorder.stop(now: clock.now, snapshot: snapshot)

        self.recorder = nil
        self.sessionDirectory = nil
        isRecording = false
    }
```

Rozszerz `tick()`:

```swift
    private func tick() {
        guard let syncEngine else { return }
        syncEngine.tick(now: clock.now)
        syncQualityMs = syncEngine.residualMs()
        sampleCount = Int(syncEngine.snapshot().samples.count)

        thermalState = String(describing: thermalProbe.thermalLevel())
        freeGigabytes = Double(storageProbe.freeBytes()) / 1_000_000_000

        if let recorder {
            recorder.tick(now: clock.now)
            elapsedSeconds = clock.now - recordingStarted
            // Rdzen mogl zakonczyc sesje sam: przegrzanie, brak miejsca,
            // cisza peera. Wtedy domykamy zapis po stronie aplikacji.
            if case .stopped = recorder.state() {
                finishRecording()
            }
        }
    }
```

Dodaj rozszerzenia pomocnicze na końcu pliku:

```swift
extension Bundle {
    var appVersion: String {
        (infoDictionary?["CFBundleShortVersionString"] as? String) ?? "0.0.0"
    }
}
```

- [ ] **Step 2: Obsłuż komendy przychodzące na slavie**

Zastąp zaślepkę `handle(command:)` z zadania 3 pełną obsługą:

```swift
    private func handle(command: LinkMessage) {
        switch command {
        case let .startRecording(sessionId, profile, _):
            guard role == .slave, !isRecording else { return }
            beginRecording(sessionId: sessionId, profile: profile)

        case .stopRecording:
            guard role == .slave else { return }
            finishRecording()

        case let .heartbeat(hostTime):
            // Slave konczy sesje po 120 s ciszy peera — rdzen liczy ten
            // czas sam, potrzebuje tylko informacji, ze lacze zyje.
            recorder?.notePeerHeartbeat(at: hostTime)

        default:
            break
        }
    }
```

W `tick()` master wysyła heartbeat, żeby slave wiedział, że łącze żyje:

```swift
        if role == .master {
            try? syncEngine.send(message: .heartbeat(hostTime: clock.now))
        }
```

- [ ] **Step 3: Dodaj sterowanie do ekranu**

W `RigView`:

```swift
            Button(coordinator.isRecording ? "Stop" : "Record") {
                coordinator.isRecording ? coordinator.stopRecording() : coordinator.startRecording()
            }
            .buttonStyle(.borderedProminent)
            .tint(coordinator.isRecording ? .red : .accentColor)
            .disabled(!coordinator.isConnected || coordinator.role == .slave)

            if coordinator.isRecording {
                HStack(spacing: 16) {
                    Text(String(format: "%.0f s", coordinator.elapsedSeconds))
                    Text(String(format: "%.1f GB", coordinator.freeGigabytes))
                    Text(coordinator.thermalState)
                }
                .font(.caption.monospaced())
            }
```

- [ ] **Step 4: Zbuduj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

- [ ] **Step 5: Weryfikacja — pełna sesja na dwóch telefonach**

1. Połącz oba telefony, poczekaj na zieloną jakość synchronizacji.
2. Na masterze naciśnij Record. **Slave musi zacząć nagrywać sam** — jego ekran pokazuje licznik czasu.
3. Nagrywaj **6 minut**, żeby wymusić przewinięcie segmentu.
4. Zatrzymaj na masterze. Slave zatrzymuje się sam.
5. W aplikacji Pliki na obu telefonach sprawdź katalog `session-…`: musi zawierać `manifest.json`, co najmniej dwa `video-*.mov` i `motion.jsonl`.
6. Otwórz `manifest.json` i sprawdź:
   - `sessionId` **identyczny na obu telefonach**
   - `role` różny
   - `timing.syncModel.residualStdMs` poniżej 2
   - `camera.locked.exposureDurationSeconds` około 0,001
   - `segments` ma co najmniej dwa wpisy, bez przerw
7. Szarpnij telefonem w trakcie nagrania — w `events` pojawia się `motion-spike`.
8. **Test zerwania łącza:** w trakcie nagrania wyłącz WiFi na slavie na 30 sekund. Nagrywanie **musi trwać dalej na obu**, a po zakończeniu w `syncGaps` musi być wpis.

- [ ] **Step 6: Commit**

```bash
git add app-ios
git commit -m "feat(ios): pelny przebieg sesji z synchronizacja master-slave"
```

---

### Task 9: Eksport sesji

**Files:**
- Create: `app-ios/TennisTrackerRig/UI/ExportView.swift`
- Modify: `app-ios/TennisTrackerRig/UI/RigView.swift`, `Info.plist`

**Interfaces:**
- Consumes: `SessionStore` z zadania 5
- Produces: `ExportView` — lista sesji z rozmiarem i akcją udostępniania

- [ ] **Step 1: Włącz widoczność w aplikacji Pliki**

Dodaj do `Info.plist`:

```xml
<key>UIFileSharingEnabled</key>
<true/>
<key>LSSupportsOpeningDocumentsInPlace</key>
<true/>
```

Dzięki temu katalogi sesji są widoczne w Plikach i można je przeciągnąć na Maca kablem albo wysłać AirDropem, bez pisania własnego eksportera.

- [ ] **Step 2: Napisz listę sesji**

`app-ios/TennisTrackerRig/UI/ExportView.swift`:

```swift
import SwiftUI

struct ExportView: View {
    let store: SessionStore
    @State private var shared: URL?

    var body: some View {
        List(store.sessionDirectories, id: \.self) { directory in
            HStack {
                VStack(alignment: .leading) {
                    Text(directory.lastPathComponent)
                        .font(.system(.body, design: .monospaced))
                    Text(describe(directory))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Share") { shared = directory }
            }
        }
        .navigationTitle("Sessions")
        .sheet(item: $shared) { url in ShareSheet(items: [url]) }
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
        return String(format: "%d segments, %.1f GB", segments, Double(bytes) / 1_000_000_000)
    }
}
```

`ShareSheet` i rozszerzenie `URL: Identifiable` powstały już w zadaniu 4 — nie duplikuj ich.

- [ ] **Step 3: Podłącz do ekranu głównego**

```swift
            NavigationLink("Sessions") {
                ExportView(store: coordinator.store)
            }
```

- [ ] **Step 4: Zbuduj i zweryfikuj**

Run: `xcodebuild -project app-ios/TennisTrackerRig.xcodeproj -scheme TennisTrackerRig -destination 'generic/platform=iOS' build`
Expected: BUILD SUCCEEDED

Na urządzeniu:
1. Ekran Sessions pokazuje nagrane sesje z liczbą segmentów i rozmiarem.
2. Katalogi są widoczne w Plikach → Na moim iPhonie → TennisTrackerRig.
3. Przenieś sesje z obu telefonów na Maca do `~/tennis-sessions/master/` i `~/tennis-sessions/slave/`.

- [ ] **Step 5: Commit**

```bash
git add app-ios
git commit -m "feat(ios): lista sesji i eksport na Maca"
```

---

### Task 10: Narzędzie weryfikujące sesję

**Files:**
- Create: `tools/requirements.txt`, `tools/ms_counter.html`
- Create: `tools/verify_session.py`, `tools/test_verify_session.py`

**Interfaces:**
- Consumes: katalogi sesji z zadań 8-9 (`manifest.json` w schemacie wersji 2)
- Produces: `load_manifest(path)`, `peer_time(sync_model, host_time)`, `check_pair(master, slave)`, `locate_frame(manifest, host_time)`, `extract_pair(...)`; CLI `python tools/verify_session.py <master_dir> <slave_dir> [--extract N --out DIR]`

- [ ] **Step 1: Napisz licznik milisekundowy**

`tools/ms_counter.html`:

```html
<!doctype html>
<meta charset="utf-8">
<title>ms counter</title>
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
    el.textContent = ((performance.now() - start) / 1000).toFixed(3).padStart(8, '0');
    requestAnimationFrame(frame);
  }
  frame();
</script>
```

`tools/requirements.txt`:

```
pytest==8.3.3
```

Dodatkowo w systemie wymagany jest `ffmpeg` (`brew install ffmpeg`).

- [ ] **Step 2: Napisz testy, które mają nie przejść**

`tools/test_verify_session.py`:

```python
import json

import pytest

from verify_session import (
    ManifestError,
    check_pair,
    load_manifest,
    locate_frame,
    peer_time,
)


def make_manifest(role, session_id="s-1", offset=0.0, skew_ppm=0.0, residual=0.9):
    return {
        "schemaVersion": 2,
        "sessionId": session_id,
        "role": role,
        "platform": "ios",
        "match": {
            "players": [
                {"id": "p1", "name": "Player 1", "genitive": None, "startEnd": "north"},
                {"id": "p2", "name": "Player 2", "genitive": None, "startEnd": "south"},
            ],
            "firstServer": "p1",
            "format": "singles-ad-tiebreak",
        },
        "device": {"model": "iPhone17,2", "osVersion": "26.0", "appVersion": "0.1.0"},
        "capabilities": {
            "tier": "full",
            "manualSensor": True,
            "timestampSource": "REALTIME",
            "timestampBaseOffsetMs": None,
            "timestampBaseUncertaintyMs": None,
            "intrinsicsAvailable": True,
            "maxFps": 120,
            "minExposureSeconds": 0.000125,
        },
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
            "transport": "multipeer",
            "syncModel": {
                "referenceHostTime": 1000.0,
                "offsetSeconds": offset,
                "skewPpm": skew_ppm,
                "residualStdMs": residual,
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


def test_load_manifest_rejects_broken_json(tmp_path):
    directory = tmp_path / "broken"
    directory.mkdir()
    (directory / "manifest.json").write_text("{ nope")
    with pytest.raises(ManifestError):
        load_manifest(directory)


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
    a = write_session(tmp_path, "a", make_manifest("master"))
    b = write_session(tmp_path, "b", make_manifest("master"))
    report = check_pair(load_manifest(a), load_manifest(b))
    assert not report.ok
    assert any("role" in problem for problem in report.problems)


def test_check_pair_flags_poor_sync_quality(tmp_path):
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", make_manifest("slave", residual=7.5))
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
    assert any("contiguity" in problem for problem in report.problems)


def test_check_pair_rejects_wrong_schema_version(tmp_path):
    old = make_manifest("slave")
    old["schemaVersion"] = 1
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", old)
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert not report.ok


def test_check_pair_notes_limited_tier(tmp_path):
    limited = make_manifest("slave")
    limited["capabilities"]["tier"] = "limited"
    master = write_session(tmp_path, "master", make_manifest("master"))
    slave = write_session(tmp_path, "slave", limited)
    report = check_pair(load_manifest(master), load_manifest(slave))
    assert any("limited" in note for note in report.notes)


def test_locate_frame_finds_segment_and_offset():
    segment, offset = locate_frame(make_manifest("master"), 1350.5)
    assert segment == "video-001.mov"
    assert offset == pytest.approx(50.5)


def test_locate_frame_returns_none_outside_range():
    assert locate_frame(make_manifest("master"), 5000.0) is None
```

- [ ] **Step 3: Uruchom testy i potwierdź, że nie przechodzą**

Run: `cd tools && python -m pytest test_verify_session.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'verify_session'`

- [ ] **Step 4: Napisz narzędzie**

`tools/verify_session.py`:

```python
#!/usr/bin/env python3
"""Weryfikuje pare sesji nagranych przez rig P0a.

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
#: Jedyna obslugiwana wersja schematu manifestu.
SCHEMA_VERSION = 2


class ManifestError(Exception):
    pass


@dataclass
class Report:
    problems: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.problems


def load_manifest(directory) -> dict:
    path = Path(directory) / "manifest.json"
    if not path.exists():
        raise ManifestError(f"no manifest.json in {directory}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise ManifestError(f"broken manifest in {directory}: {error}") from error


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
        report.problems.append(f"{label}: no segments")
        return
    tolerance = 1.0 / manifest["camera"]["fps"]
    for index in range(1, len(segments)):
        gap = segments[index]["startHostTime"] - segments[index - 1]["endHostTime"]
        if gap > tolerance:
            report.problems.append(
                f"{label}: segment contiguity broken before "
                f"{segments[index]['file']} (gap {gap * 1000:.1f} ms)"
            )


def check_pair(master: dict, slave: dict) -> Report:
    report = Report()

    if master["sessionId"] != slave["sessionId"]:
        report.problems.append(
            f"different sessionId: {master['sessionId']} vs {slave['sessionId']}"
        )

    roles = {master["role"], slave["role"]}
    if roles != {"master", "slave"}:
        report.problems.append(f"invalid role pairing: {sorted(roles)}")

    for manifest, label in ((master, "master"), (slave, "slave")):
        if manifest["schemaVersion"] != SCHEMA_VERSION:
            report.problems.append(
                f"{label}: unsupported schemaVersion {manifest['schemaVersion']}"
            )
        if manifest["timing"]["clockDomain"] != "mach_absolute_time":
            report.problems.append(f"{label}: unexpected clock domain")

        residual = manifest["timing"]["syncModel"]["residualStdMs"]
        if residual > MAX_RESIDUAL_MS:
            report.problems.append(
                f"{label}: residualStdMs {residual:.2f} ms exceeds target {MAX_RESIDUAL_MS} ms"
            )
        else:
            report.notes.append(f"{label}: sync +/-{residual:.2f} ms")

        tier = manifest["capabilities"]["tier"]
        if tier != "full":
            report.notes.append(
                f"{label}: capability tier is {tier} — data usable for training, "
                "not for stereo"
            )

        if not manifest["capabilities"]["intrinsicsAvailable"]:
            report.notes.append(
                f"{label}: no intrinsics — P1 must recover focal length from court geometry"
            )

        _check_segments(manifest, label, report)

        gaps = manifest["timing"]["syncGaps"]
        if gaps:
            total = sum(
                (gap["endHostTime"] or gap["startHostTime"]) - gap["startHostTime"]
                for gap in gaps
            )
            report.notes.append(f"{label}: {len(gaps)} sync gaps, {total:.1f} s total")

        marks = [e for e in manifest["events"] if e.get("type") == "mark"]
        if marks:
            report.notes.append(f"{label}: {len(marks)} marks")

        spikes = [e for e in manifest["events"] if e.get("type") == "motion-spike"]
        if spikes:
            report.notes.append(f"{label}: {len(spikes)} motion spikes")

        segments = manifest["segments"]
        if segments:
            duration = segments[-1]["endHostTime"] - segments[0]["startHostTime"]
            report.notes.append(f"{label}: {len(segments)} segments, {duration:.1f} s")

    return report


def locate_frame(manifest: dict, host_time: float):
    """Zwraca (nazwa pliku, przesuniecie w sekundach) dla podanego czasu lokalnego."""
    for segment in manifest["segments"]:
        if segment["startHostTime"] <= host_time <= segment["endHostTime"]:
            return segment["file"], host_time - segment["startHostTime"]
    return None


def extract_pair(master_dir, master, slave_dir, slave, host_time, out_dir, index) -> bool:
    """Wyciaga dwie klatki, ktore wedlug modelu synchronizacji sa jednoczesne."""
    master_hit = locate_frame(master, host_time)
    slave_hit = locate_frame(slave, peer_time(master["timing"]["syncModel"], host_time))
    if master_hit is None or slave_hit is None:
        return False

    out_dir = Path(out_dir)
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
        help="how many simultaneous frame pairs to extract for comparison",
    )
    parser.add_argument("--out", type=Path, default=Path("out"))
    args = parser.parse_args()

    master = load_manifest(args.master_dir)
    slave = load_manifest(args.slave_dir)
    master_dir, slave_dir = args.master_dir, args.slave_dir
    if master["role"] == "slave":
        master, slave = slave, master
        master_dir, slave_dir = slave_dir, master_dir

    report = check_pair(master, slave)
    for note in report.notes:
        print(f"  {note}")
    for problem in report.problems:
        print(f"ERROR: {problem}")

    if args.extract > 0:
        segments = master["segments"]
        start = segments[0]["startHostTime"] + 2.0
        end = segments[-1]["endHostTime"] - 2.0
        step = (end - start) / max(args.extract - 1, 1)
        extracted = sum(
            extract_pair(
                master_dir, master, slave_dir, slave,
                start + index * step, args.out, index,
            )
            for index in range(args.extract)
        )
        print(f"  extracted {extracted} frame pairs to {args.out}")
        print("  Compare the counter on each pair — it must read the same value.")

    print("OK" if report.ok else "FAILED")
    return 0 if report.ok else 1


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 5: Uruchom testy i potwierdź, że przechodzą**

Run: `cd tools && python -m pytest test_verify_session.py -v`
Expected: PASS, 13 testów

- [ ] **Step 6: Commit**

```bash
git add tools
git commit -m "feat(tools): weryfikacja sesji i wyciaganie klatek jednoczesnych"
```

---

### Task 11: Test akceptacyjny i dokumentacja

Zamyka P0a. Do tej pory nic nie dowiodło, że oś czasu jest naprawdę wspólna — dowodzi tego dopiero ten test.

**Files:**
- Create: `README.md`
- Modify: `docs/superpowers/plans/2026-09-07-p0a-2-aplikacja-ios.md` (odhaczenie)

- [ ] **Step 1: Przeprowadź test akceptacyjny**

1. Otwórz `tools/ms_counter.html` na Macu, tryb pełnoekranowy.
2. Ustaw oba telefony obok siebie, oba skierowane na ekran Maca tak, żeby licznik był czytelny w kadrze.
3. Połącz je, poczekaj na zieloną jakość synchronizacji.
4. Nagrywaj **2 minuty**.
5. Przenieś obie sesje na Maca.
6. Uruchom:

```bash
python tools/verify_session.py ~/tennis-sessions/master ~/tennis-sessions/slave --extract 10 --out out/
```

7. Otwórz pary PNG z `out/` i porównaj odczyt licznika na klatce mastera i slave'a.

**Kryterium zaliczenia:** różnica odczytu licznika w każdej parze nie przekracza **jednego okresu klatki** (8,3 ms przy 120 fps).

Większa różnica oznacza błąd w łańcuchu synchronizacji i musi zostać zdiagnozowana przed zamknięciem P0a. Najbardziej prawdopodobne przyczyny, w kolejności:
- pingi idą kanałem niezawodnym zamiast `.unreliable` (sprawdź mapowanie w `MultipeerTransport.send`)
- znacznik odbioru pobierany po skoku na główny wątek zamiast natychmiast
- użycie `Date()` gdziekolwiek zamiast `CACurrentMediaTime()`

- [ ] **Step 2: Uruchom drugi, niezależny test**

Klaśnięcie przy obu telefonach w trakcie nagrania. Po zgraniu porównaj pozycję transjentu w obu ścieżkach audio:

```bash
ffmpeg -i ~/tennis-sessions/master/video-000.mov -map 0:a -f wav - 2>/dev/null | \
  python -c "import sys,wave,struct; w=wave.open(sys.stdin.buffer); print(w.getframerate(), w.getnframes())"
```

Przy telefonach obok siebie propagacja dźwięku jest pomijalna, więc transjenty muszą wypadać w tym samym momencie wspólnej osi czasu. To potwierdzenie niezależne od zegara sieciowego.

- [ ] **Step 3: Napisz README**

`README.md`:

```markdown
# Tennis Tracker

Automatic scoring for a tennis match, from two phones mounted on the court
fence.

Subprojects: **P0a** core + iOS rig (this code) → P0b Android rig →
P1 ball detector and court calibration → P2 live scoreboard →
P3 3D challenge → P4 doubles.

## Layout

- `src/` — `tracker-core`, all hardware-free logic, in Rust
- `app-ios/` — iOS app, hardware adapters only
- `tools/` — session verification and the synchronisation acceptance test
- `docs/superpowers/specs` — designs
- `docs/superpowers/plans` — implementation plans

## Tests

```bash
cargo test                          # core logic, no device needed
cd tools && python -m pytest        # verification tool
```

## Building

```bash
./scripts/build-xcframework.sh      # rebuild after changing the Rust core
```

Then open `app-ios/TennisTrackerRig.xcodeproj`.

## On court

1. Install on both phones. A free provisioning profile expires after 7 days;
   the expiry date is shown on the main screen.
2. Mount the phones on **opposite sidelines**, each at the middle of its own
   half, about **3 m** high.
3. Aim so the court plus a 1-2 m margin fills the frame, with no headroom for
   the sky. A lob's apex will not fit and does not need to — the bounce is
   what decides the point, and it is always on the ground.
4. Master on the iPhone 16 Pro Max, Slave on the other, then Connect.
5. Wait for green sync quality (below 2 ms).
6. Record. The slave starts and stops on its own.

**Do not tidy the court.** Leave balls where they normally lie. Training
footage of a tidied court teaches the detector a world that does not exist.
A stationary ball is filtered out for free — the detector reads motion.

## Verifying a session

```bash
python tools/verify_session.py <master-dir> <slave-dir>
```

## Notes

- 1080p120 costs about 18 GB per hour. Check free space before a match.
- If the phones cannot see each other, check `NSBonjourServices` in
  `Info.plist` and the local network permission in Settings.
- If sync quality is worse than 2 ms, verify that pings map to
  `.unreliable` — a ping sent over a reliable channel lies about latency.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: README z instrukcja uzycia rigu na korcie"
```

---

## Kryteria ukończenia P0a

Odwzorowanie sekcji 16 specyfikacji na sprawdzalne fakty:

1. `cargo test` przechodzi w całości.
2. `cd tools && python -m pytest` przechodzi w całości.
3. Aplikacja paruje dwa iPhone'y bez internetu i bez konfiguracji sieciowej — zadanie 3, krok 6.
4. Jakość synchronizacji poniżej 2 ms widoczna w UI — zadanie 3, krok 6.
5. Sesja 30-minutowa nagrywa się na obu urządzeniach z poprawnymi manifestami i identycznym `sessionId` — zadanie 8, krok 5.
6. Zerwanie łącza nie przerywa zapisu, luka odnotowana w `syncGaps` — zadanie 8, krok 5, punkt 8.
7. Ustawienie meczu trafia do manifestu, a jego pominięcie nie blokuje nagrywania — zadanie 7, krok 4.
8. Raport zdolności generuje się i trafia do manifestu — zadanie 4, krok 5.
9. Test akceptacyjny z licznikiem milisekundowym zaliczony — zadanie 11, krok 1.
10. Sesja eksportowalna na Maca i wczytywalna narzędziem — zadanie 9, krok 4 oraz zadanie 10.
11. Zweryfikowana dostępność uprawnienia `HotspotConfiguration` na darmowym provisioningu — **przeniesione do P0b**, bo dotyczy wyłącznie mieszanej pary iOS/Android.

Punkty 7 i 9 ze specyfikacji dotyczące **zegarka** realizuje część 3.

## Co dalej

**Część 3:** aplikacja watchOS (start/stop, `MarkMoment`, ekran stanu) oraz transport BLE jako droga awaryjna i przyszły rozruch mieszanej pary.

**P0b:** aplikacja Android, `LocalOnlyHotspot`, gniazda TCP i UDP, mieszana para.

**P1:** detektor piłki i kalibracja kortu — startuje od danych publicznych i syntetycznych, a materiał z pierwszego wyjścia na kort go dostraja. Wtedy też wykonuje się zadanie pomiarowe kadru z sekcji 15 specyfikacji.
