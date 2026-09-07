# Tennis Tracker — P0a: rdzeń Rust + rig iOS (design)

Data: 2026-09-07
Status: zatwierdzony do planowania implementacji
Zastępuje: `2026-09-02-tennis-tracker-p0-rig-danych-design.md`

## 1. Kontekst

Celem całego przedsięwzięcia jest system, który **w trakcie gry** automatycznie prowadzi punktację meczu singlowego. Dwa telefony zawieszone na ogrodzeniu kortu obserwują po jednej połówce, wykrywają piłkę i kozły, a jeden z nich ogłasza stan głosem.

Sprzęt autora:

- iPhone 16 Pro Max (A18 Pro) — **master**
- iPhone 15 (A16, telefon kolegi) — **slave**
- Apple Watch — pilot i wyświetlacz stanu
- Mac do developmentu, konto Apple Developer w wersji darmowej (apka wygasa co 7 dni, akceptowane)
- Dostęp do urządzenia z Androidem: **okazjonalny, z pożyczenia**

Ustawienie na korcie: telefony na **przeciwnych liniach bocznych**, każdy w połowie długości swojej połówki kortu, na wysokości **około 3 m**, na sztywnym ogrodzeniu siatkowym. Nawierzchnia: głównie **mączka**.

Format meczu: **singiel**, pełne sety, tiebreak przy 6:6, przewaga (ad).

### Decyzje architektoniczne

**Aplikacje natywne, nie webowe.** Dostęp do akceleratorów ML (z przeglądarki nieosiągalny), pełna kontrola kamery (blokada ekspozycji, ISO, ostrości, wysokie fps), łączność bez serwera sygnalizacyjnego oraz sprzętowe znaczniki czasu klatek.

**Wspólny rdzeń w Ruście, spięty przez UniFFI.** Około 60% kodu projektu to czysta logika: synchronizacja zegarów, protokół łącza, manifest, geometria, punktacja. Pisana raz, konsumowana przez Swifta i Kotlina z automatycznie generowanych bindingów. Testowana przez `cargo test` na Macu — bez symulatora i bez telefonu.

**Klatka nigdy nie przekracza granicy FFI.** Warstwa natywna przechwytuje obraz i wykonuje inferencję; do rdzenia wchodzą wyłącznie liczby — pozycja piłki, znacznik czasu, pewność. Kopiowanie bufora 1080p przez FFI byłoby zabójcze, kilkanaście bajtów na klatkę jest bezkosztowe.

**Fuzja na poziomie zdarzeń, nie klatek.** Każde urządzenie prowadzi własny pipeline i wysyła do mastera chude zdarzenia we **współrzędnych kortu w metrach**, nigdy w pikselach. Granicą między urządzeniami są metry. Master nie musi wiedzieć nic o optyce ani kalibracji peera. Rozwiązanie skaluje się na debla, na trzecią kamerę i na wymianę telefonu na dedykowany sprzęt.

**Kort jest wzorcem kalibracyjnym.** Wymiary są znormalizowane (23,77 na 10,97 m, linie serwisowe 6,40 m od siatki), więc detekcja linii plus `solvePnP` daje pełną pozę kamery w 3D. Ponieważ obie kamery wyznaczają pozę względem tego samego kortu, **wzajemna poza kamer wychodzi za darmo** — bez szachownicy i bez wspólnej procedury kalibracyjnej.

**Do liczenia punktów stereo nie jest potrzebne.** W chwili kozła piłka dotyka płaszczyzny kortu, więc homografia przenosi punkt z obrazu wprost we współrzędne kortu. Jedna skalibrowana kamera wystarcza do orzeczenia, gdzie piłka odbiła. Stereo jest potrzebne dopiero do pełnej trajektorii 3D.

**Kamera musi widzieć powierzchnię kortu, nie całą przestrzeń gry.** Wszystko, co decyduje o punkcie — kozioł, przekroczenie siatki — dzieje się na ziemi lub tuż nad nią. Apogeum loba nie decyduje o niczym, więc jego utrata z kadru jest akceptowana (patrz sekcja 15).

### Dekompozycja na podprojekty

| | Podprojekt | Zależność |
|---|---|---|
| **P0a** | **Rdzeń Rust + rig iOS — ten dokument** | brak |
| P0b | Rig Android, mieszana para | P0a |
| P1 | Detektor piłki i kalibracja kortu (Python, offline) | dane z P0a; P0b poszerza |
| P2 | Tablica wyników na żywo, obie platformy | P1 |
| P3 | Challenge i powtórka 3D | P2 |
| P4 | Debel | P2 |

Kolejność jest odwrócona względem intuicji celowo: detektor piłki jest sercem systemu, a nie da się go wytrenować bez nagrań z tych telefonów, z tego kortu i z tej wysokości. P0a nie zawiera ani grama uczenia maszynowego, więc powstaje niezależnie od dostępu do kortu.

## 2. Zakres P0a

### Czym P0a jest

Crate Rust `tracker-core` plus aplikacja iOS (jedna binarka, rola wybierana przy starcie) plus aplikacja watchOS. Zadanie: nagrać z dwóch telefonów materiał, z którego da się wytrenować detektor i odtworzyć geometrię. To znaczy wideo o **stałych, znanych parametrach**, ze **wspólną osią czasu** i z **pełnymi metadanymi**.

Sedno: kluczowym artefaktem nie jest wideo, tylko *wideo z wiarygodną osią czasu i znaną optyką*. Nagranie bez tego jest w P1 warte tyle, co film znaleziony w internecie.

P0a jest użyteczny sam w sobie — daje nagrania meczów z dwóch ujęć.

### Czym P0a nie jest (świadome wyłączenia)

- Nie wykrywa piłki, kortu ani linii
- Nie liczy punktów
- Nie zawiera modelu ML
- Nie obsługuje Androida (to P0b) ani debla
- Nie przetwarza wideo w czasie rzeczywistym

Każdy z tych elementów wymaga danych, których jeszcze nie ma.

### Co P0a przygotowuje pod P0b

Interfejsy portów projektowane są od początku pod **ograniczenia Camera2**, nie pod możliwości AVFoundation. Konkretnie: brak gwarancji manualnej ekspozycji, brak gwarancji intrinsics, znaczniki czasu na nieznanej bazie, wybór między wysokim fps a kontrolą manualną. Implementacja iOS wypełnia je komfortowo, ale kształt interfejsu jest podyktowany słabszą platformą.

## 3. Moduły

### Rdzeń Rust (`core/`)

| Moduł | Odpowiedzialność |
|---|---|
| `clock::ClockSample` | próbka ping-pong, offset i delay |
| `clock::ClockSyncEstimator` | filtr best-delay i dopasowanie liniowe |
| `clock::ClockModel` | konwersja czasu w obie strony |
| `clock::SyncSession` | harmonogram pingów, luki w ciągłości |
| `link::LinkMessage`, `link::LinkCodec` | protokół i koperta |
| `link::PeerLink` | numeracja, deduplikacja, stan łącza |
| `session::SessionManifest` | model i walidacja sidecara |
| `session::MatchSetup` | gracze, wiązanie ze stronami kortu, kolejność serwisu |
| `session::MotionAnalyzer` | detekcja szarpnięć kamery |
| `session::SessionRecorder` | maszyna stanów sesji |
| `device::CapabilityReport` | zdolności urządzenia i poziom pracy |
| `remote::RemoteCommand`, `remote::RemoteFeedback` | zestaw komend pilota |

Wszystkie te moduły są wolne od typów platformowych. Zależności sprzętowe wyrażone są traitami implementowanymi po stronie natywnej.

### Aplikacja iOS (`app-ios/`)

| Moduł | Odpowiedzialność |
|---|---|
| `MonotonicClock` | jedyne źródło czasu, `CACurrentMediaTime()` |
| `CaptureRig` | `AVCaptureSession` z blokadą parametrów, zapis segmentowany z audio |
| `MultipeerTransport` | transport podstawowy dla pary iOS |
| `BleTransport` | droga awaryjna i przyszły rozruch mieszanej pary |
| `MotionRecorder` | CoreMotion, zapis `motion.jsonl` |
| `SystemProbes` | wolne miejsce, stan termiczny |
| `CapabilityProbe` | odpytanie możliwości urządzenia |
| `SessionStore` | katalogi sesji, zapis manifestu |
| `PlayerRoster` | trwała lista znanych graczy |
| `SessionCoordinator` | spina rdzeń z adapterami |
| `WatchBridge` | WatchConnectivity, przekaz komend |
| `RigUI` | podgląd, poziomica, stan, eksport, diagnostyka |

### Aplikacja watchOS (`app-watch/`)

Trzy funkcje: start/stop nagrywania, **oznacz moment**, ekran stanu (czas, wolne miejsce, termika, jakość synchronizacji, ostrzeżenia).

## 4. Produkt sesji

Katalog na każdym z telefonów:

```
session-2026-09-14-1732/
  manifest.json
  video-000.mov, video-001.mov, ...   segmenty po 5 min, wideo + audio PCM
  motion.jsonl                        IMU 100 Hz
```

### Schemat `manifest.json`

```json
{
  "schemaVersion": 2,
  "sessionId": "2026-09-14T17:32:10Z-a3f9",
  "role": "master",
  "platform": "ios",
  "peerDeviceModel": "iPhone15,4",
  "match": {
    "players": [
      { "id": "p1", "name": "Piotr", "genitive": "Piotra", "startEnd": "north" },
      { "id": "p2", "name": "Marek", "genitive": "Marka", "startEnd": "south" }
    ],
    "firstServer": "p1",
    "format": "singles-ad-tiebreak"
  },
  "device": {
    "model": "iPhone17,2",
    "osVersion": "26.0",
    "appVersion": "0.1.0"
  },
  "capabilities": {
    "tier": "full",
    "manualSensor": true,
    "timestampSource": "REALTIME",
    "timestampBaseOffsetMs": null,
    "timestampBaseUncertaintyMs": null,
    "intrinsicsAvailable": true,
    "maxFps": 120,
    "minExposureSeconds": 0.000125
  },
  "camera": {
    "lens": "builtInWideAngleCamera",
    "profile": "1080p120",
    "width": 1920,
    "height": 1080,
    "fps": 120,
    "codec": "hevc",
    "targetBitrateMbps": 40,
    "audio": { "sampleRateHz": 48000, "channels": 1, "codec": "pcm" },
    "locked": {
      "exposureDurationSeconds": 0.001,
      "iso": 64,
      "focusLensPosition": 0.82,
      "whiteBalanceGains": { "r": 1.9, "g": 1.0, "b": 1.6 }
    },
    "intrinsicMatrix": [[1580.0, 0.0, 960.0], [0.0, 1580.0, 540.0], [0.0, 0.0, 1.0]]
  },
  "timing": {
    "clockDomain": "mach_absolute_time",
    "firstFrameHostTime": 123456.789012,
    "transport": "multipeer",
    "syncModel": {
      "referenceHostTime": 123456.0,
      "offsetSeconds": 0.0142,
      "skewPpm": 7.3,
      "residualStdMs": 0.9
    },
    "syncSamples": [
      { "hostTime": 123456.0, "offsetSeconds": 0.0142, "delaySeconds": 0.0031 }
    ],
    "syncGaps": [
      { "startHostTime": 124000.0, "endHostTime": 124035.0, "reason": "link-lost" }
    ]
  },
  "segments": [
    { "file": "video-000.mov", "startHostTime": 123456.789,
      "endHostTime": 123756.789, "frameCount": 36000 }
  ],
  "events": [
    { "hostTime": 123500.1, "type": "motion-spike", "magnitude": 0.42 },
    { "hostTime": 123610.0, "type": "capture-interrupted", "reason": "system-pressure" },
    { "hostTime": 123700.0, "type": "thermal", "state": "serious" },
    { "hostTime": 123812.5, "type": "mark", "tag": "interesting" },
    { "hostTime": 124455.0, "type": "mark", "tag": "changeover" }
  ],
  "motionLog": "motion.jsonl"
}
```

Wszystkie znaczniki czasu są w domenie monotonicznego zegara **lokalnego** urządzenia, w sekundach, typu `f64`. Przeliczenie na wspólną oś czasu wykonuje się w P1 przy użyciu `timing.syncModel`. Manifest slave'a i mastera opisują tę samą sesję pod wspólnym `sessionId`.

`schemaVersion` wynosi **2** — wersja 1 opisywała rig wyłącznie iOS bez pól `platform` i `capabilities`.

Zdarzenia typu `mark` pochodzą z zegarka i niosą znacznik czasu momentu wskazanego przez gracza.

## 5. Synchronizacja czasu

### Algorytm

Handshake w stylu NTP. Master wysyła ping ze swoim `t1`, peer odbiera w `t2`, odsyła `t2` i `t3`, master odbiera w `t4`:

```
offset = ((t2 - t1) + (t3 - t4)) / 2
delay  = (t4 - t1) - (t3 - t2)
```

Zachowywane są **wyłącznie próbki o najniższym `delay`** — pakiet, który przeszedł najszybciej, przeszedł najmniej zaburzony, więc jego `offset` jest najbliższy prawdy. Filtr ten zastępuje odporną statystykę i sam usuwa wartości odstające. Rozrzut wśród zachowanych próbek trafia do manifestu jako `residualStdMs`.

### Trzy warunki poprawności

**Zegar monotoniczny.** Nigdy czas ścienny — systemowa korekta NTP przesunęłaby oś czasu w środku nagrania. Na iOS `CACurrentMediaTime()` opiera się na `mach_absolute_time`, czyli tej samej podstawie co `CMSampleBuffer.presentationTimeStamp` (`CMClockGetHostTimeClock()`), więc znaczniki klatek są bezpośrednio porównywalne z wynikiem synchronizacji bez konwersji.

**Model liniowy: offset oraz skew.** Kwarce dwóch urządzeń chodzą z różną prędkością, typowo 1–20 ppm, co przez godzinę daje rozjazd 4–72 ms, czyli do dziewięciu klatek przy 120 fps. Pojedyncza synchronizacja na starcie jest bezużyteczna. Stosowany jest model liniowy — punkt odniesienia, offset, dryf — dopasowywany na bieżąco. Do manifestu trafia model i surowe próbki.

**Kadencja.** Seria około 50 pingów przy uzbrajaniu sesji, następnie ping co 1 s przez cały czas nagrania.

### Pingi idą po UDP, nigdy po TCP

Pomiar zakłada, że opóźnienie pakietu odzwierciedla rzeczywistą drogę. TCP z retransmisjami, blokowaniem czoła kolejki i algorytmem Nagle'a **kłamie o opóźnieniu** — jeden zgubiony pakiet i próbka pokazuje 200 ms zamiast 5. Filtr best-delay to odrzuci, ale kosztem połowy próbek.

Zatem: **UDP dla pingów, TCP dla komend.** W MultipeerConnectivity odpowiednio `.unreliable` i `.reliable`.

### Cel dokładności

**Poniżej 2 ms.** Przy 120 fps okres klatki wynosi 8,3 ms, więc 2 ms to ćwierć klatki.

### Walidacja

**Test podstawowy, wykonalny w domu:** oba telefony obok siebie, skierowane na ekran Maca wyświetlający licznik z rozdzielczością milisekundową. Po nagraniu porównuje się czas widoczny na klatkach, które według modelu synchronizacji są jednoczesne. Weryfikuje cały łańcuch: zegar, synchronizację i znaczniki klatek.

**Test niezależny:** klaśnięcie i korelacja skrośna ścieżek audio. Przy telefonach obok siebie propagacja dźwięku jest pomijalna. Na korcie, przy 13 m, wnosi do 38 ms — wielkość obliczalna z kalibracji, więc możliwa do odjęcia (istotne w P3).

## 6. Transport i parowanie

Trzy ścieżki, **wszystkie za jednym traitem `PeerTransport` w rdzeniu**. Protokół, numeracja, deduplikacja, estymator i maszyna stanów są wspólne; różni się wyłącznie sposób przesyłania bajtów.

| Para | Odkrycie | Dane | Konfiguracja od użytkownika |
|---|---|---|---|
| **iOS ↔ iOS** | MultipeerConnectivity (AWDL) | MPC | żadna |
| Android ↔ Android | BLE → `LocalOnlyHotspot` | TCP + UDP | żadna |
| iOS ↔ Android | BLE → hotspot z Androida | TCP + UDP | jedno dotknięcie |

Powód rozdzielenia: **iOS nie potrafi programowo stworzyć hotspotu** — Personal Hotspot to ręczny przełącznik wymagający planu komórkowego. Dla pary iPhone'ów zostaje więc MPC, który jest zresztą lepszy: AWDL to bezpośredni link peer-to-peer bez punktu dostępowego.

Android ma `WifiManager.startLocalOnlyHotspot()` (API 26+) — hotspot bez udostępniania internetu, zaprojektowany do komunikacji między urządzeniami, przekazujący aplikacji wygenerowane SSID i hasło.

### BLE jest kanałem rozruchowym, nie kanałem danych

1. Oba urządzenia rozgłaszają się i skanują po BLE, znajdują się, wymieniają role i **raporty możliwości**
2. Android tworzy `LocalOnlyHotspot`, wysyła poświadczenia po BLE
3. iPhone dołącza przez `NEHotspotConfiguration` — systemowe okienko, jedno dotknięcie
4. Odkrycie usługi przez mDNS/NSD, otwarcie gniazd TCP i UDP
5. BLE zostaje podłączone jako watchdog i droga awaryjna

**W P0a implementowane są MPC i BLE.** Ścieżka gniazdowa (TCP + UDP) oraz tworzenie hotspotu powstają w P0b, ale trait i protokół są na nie gotowe od początku.

BLE w P0a nie jest przedwczesną optymalizacją, mimo że para dwóch iPhone'ów obywa się bez niego. Pełni dwie role: jest **drugą implementacją tego samego traitu**, co dowodzi, że abstrakcja transportu nie przyjęła kształtu MultipeerConnectivity — a to jest dokładnie ryzyko, przed którym broni się cały podział na P0a i P0b — oraz stanowi **testowalną drogę awaryjną**, gdy MPC zawiedzie. Bez tego trait miałby jedną implementację i nie byłoby wiadomo, czy jest abstrakcją, czy tylko opakowaniem.

### Drabina degradacji

| Poziom | Sync | Co działa |
|---|---|---|
| MPC albo hotspot + UDP | < 2 ms | wszystko, łącznie z 3D w P3 |
| tylko BLE | ~±15 ms | punktacja tak, triangulacja nie |
| audio jako sync | < 1 ms | ratuje BLE, dokładane w P3 |

BLE nie osiągnie celu poniżej 2 ms, ponieważ iOS wymusza minimalny interwał połączenia 15 ms (w praktyce negocjuje 30 ms), co kwantyzuje pomiar czasu obiegu. Na komendy i zdarzenia (rzędu 2 KB/s) jest w zupełności wystarczające.

### Ryzyko do zweryfikowania na starcie

Uprawnienie `com.apple.developer.networking.HotspotConfiguration` może nie być dostępne na darmowym provisioningu. Weryfikacja jest **osobnym zadaniem na początku P0a**, nie odkryciem w połowie. Jeśli uprawnienia nie ma, ścieżka awaryjna to ręczne dołączenie do sieci przez Ustawienia; dotyczy wyłącznie mieszanej pary, czyli P0b.

## 7. Parametry nagrywania

### Blokada parametrów kamery

Przed startem następuje krótki pomiar sceny, po czym **ekspozycja, ISO, ostrość i balans bieli zostają zablokowane**. Autoekspozycja polująca w trakcie wymiany oznacza zmienną jasność między klatkami, co psuje detekcję i późniejsze różnicowanie klatek. Zablokowane wartości trafiają do manifestu.

### Czas naświetlania ważniejszy niż liczba klatek

Piłka lecąca 40 m/s przy 1/1000 s przesuwa się 4 cm — mniej niż własna średnica, więc pozostaje kropką. Przy 1/250 s przesuwa się 16 cm, czyli rozmazuje się w kreskę długości dwóch i pół piłki. Na mączce w świetle dziennym 1/1000 s jest osiągalne bez trudu.

Zasada ta rozstrzyga konflikt, który pojawi się w P0b: sesja `CONSTRAINED_HIGH_SPEED_VIDEO` na Androidzie odbiera część kontroli manualnej. **Zwykła sesja 60 fps z pełną kontrolą bije sesję 120 fps z autoekspozycją.**

### Profile

| Platforma | Domyślny | Pozostałe |
|---|---|---|
| iOS | `1080p120` | `4K60`, `1080p60` |
| Android (P0b) | `1080p60` | `1080p30` |

Wybór „rozdzielczość kontra liczba klatek" **nie jest rozstrzygany w tym dokumencie**. Profil jest konfigurowalny i zapisywany w manifeście; porównanie wykona się w P1 na rzeczywistych nagraniach, na podstawie zmierzonej skuteczności detektora. Dostarczenie danych do tej decyzji jest jednym z zadań P0a.

Uzasadnienie wartości domyślnej dla iOS: wysoka liczba klatek sama wymusza krótką ekspozycję, a moment kozła lokalizuje się do ±4 ms zamiast ±8 ms — przy piłce 40 m/s to różnica między ±16 cm a ±33 cm niepewności miejsca odbicia. Piłka zajmuje szacunkowo około 7 px przy dalekiej linii i około 15 px blisko kamery, co mieści się w zakresie, dla którego architektura TrackNet jest projektowana.

### Audio

Każdy segment zawiera **nieskompresowaną ścieżkę audio** (48 kHz, mono, PCM), zapisywaną w tym samym pliku co wideo, dzięki czemu dzieli z nim oś czasu. W P0a audio jest niezależnym kanałem walidacji synchronizacji; w P2 staje się podstawą detekcji uderzeń rakiety i taśmy siatki. Kompresja jest wykluczona — interesujące zdarzenia to krótkie transjenty, a te giną w kodekach stratnych.

### Bitrate

Ustawiany wysoko, bez oszczędzania. Kompresja HEVC traktuje szybką, siedmiopikselową plamkę jak szum i usuwa ją jako pierwszą. Piłka jest dokładnie tym, co kodek wyrzuca najchętniej. Utrata miejsca na dysku jest tańsza niż utrata piłki.

### Segmentacja i zasoby

1080p120 zajmuje około **18 GB na godzinę**. Stąd segmenty po **300 sekund**: chronią przed utratą całej sesji przy zapchanym dysku lub awarii i ułatwiają transfer. `SessionRecorder` monitoruje wolne miejsce oraz stan termiczny, ostrzega zawczasu, a przy przegrzaniu proponuje zejście na niższy profil zamiast po cichu gubić klatki.

## 8. Zdolności urządzenia i poziomy pracy

P0a nie wykonuje inferencji, więc bramki dotyczą **wyłącznie kontroli kamery i czasu**. Wymagania obliczeniowe wchodzą dopiero w P2.

| Bramka | Klucz (Android, dla P0b) | Wymóg |
|---|---|---|
| Manualny sensor | `REQUEST_AVAILABLE_CAPABILITIES` ⊃ `MANUAL_SENSOR` | ekspozycja, ISO, czas trwania klatki |
| Ekspozycja | `SENSOR_INFO_EXPOSURE_TIME_RANGE` | minimum ≤ 1 ms |
| Ostrość | `CONTROL_AF_MODE_OFF` | blokada ostrości |
| Klatki | `getHighSpeedVideoFpsRangesFor()` lub zwykła sesja | ≥ 60 fps |
| Znaczniki czasu | `SENSOR_INFO_TIMESTAMP_SOURCE` | `REALTIME` |

Sprawdzana jest zdolność `MANUAL_SENSOR` bezpośrednio, nie poziom sprzętowy — poziom FULL ją implikuje, ale odpytanie o zdolność jest precyzyjniejsze.

### Poziomy

| Poziom | Warunek | Skutek |
|---|---|---|
| **full** | wszystkie bramki | dane w pełni użyteczne, także do stereo |
| **limited** | brak `MANUAL_SENSOR` (zostaje blokada AE) albo `timestampSource` = `UNKNOWN` | nagrywa, braki w manifeście, dane do treningu tak, do stereo nie |
| **rejected** | brak blokady AE albo < 30 fps | apka odmawia i wypisuje, czego brakuje |

### Znaczniki czasu przy źródle `UNKNOWN`

Nie jest to blokada, tylko degradacja, i ratuje ją ta sama sztuczka co synchronizację. Dla wielu klatek mierzy się parę (znacznik klatki, odczyt zegara monotonicznego w chwili dostarczenia) i bierze **minimum różnicy** — najszybciej dostarczona klatka przeszła najkrótszą drogą, więc jej różnica najlepiej przybliża przesunięcie baz. Wynik trafia do `capabilities.timestampBaseOffsetMs` razem z niepewnością.

### Intrinsics

iOS dostarcza macierz intrinsics per klatka. Android w praktyce prawie nigdy. Manifest jawnie odnotowuje `intrinsicsAvailable`, żeby pipeline P1 wiedział, czy odzyskiwać ogniskową z geometrii kortu.

### Raport możliwości

Osobny ekran diagnostyczny odpytuje wszystkie bramki naraz i zrzuca JSON do pliku; ten sam raport wchodzi do manifestu każdej sesji.

Uzasadnienie: dostęp autora do Androida jest okazjonalny. Jedna sesja z pożyczonym telefonem ma dać komplet twardych danych zamiast wrażenia. Zbierane raporty tworzą realną macierz zgodności.

### Słaby sprzęt jest dla rigu zaletą

P1 potrzebuje różnorodności sensorów, obiektywów i charakterystyk kompresji, żeby detektor nie przeuczył się na jeden aparat. Nagranie z budżetowego telefonu to materiał, którego inaczej nie byłoby. Dlatego P0 nagrywa na wszystkim powyżej progu `rejected` i **uczciwie dokumentuje** warunki, zamiast odrzucać sprzęt.

## 9. Zegarek

Telefony wiszą 3 m nad ziemią, więc **żadne ostrzeżenie nie jest widoczne**: kończące się miejsce, przegrzanie, zerwane łącze, spadła jakość synchronizacji. Bez zegarka informacja o przerwanym nagrywaniu dociera dopiero po meczu. To argument mocniejszy niż wygoda sterowania.

### Zestaw komend

Definiowany raz w rdzeniu, wspólny dla każdego urządzenia sterującego.

| Podprojekt | Komendy |
|---|---|
| **P0a** | `StartRecording`, `StopRecording`, `MarkMoment { tag }` |
| P2 | punkt dla gracza, cofnij punkt, powtórz punkt, ustaw wynik, powtórz ogłoszenie, wstrzymaj liczenie, odrzuć ostatnią wymianę |
| P3 | poproś o powtórkę |

`MarkMoment` jest w P0a nieprzypadkowo. Podczas nagrywania materiału treningowego stuknięcie w zegarek przy nietypowym zdarzeniu — dziwny kozioł, piłka z sąsiedniego kortu, dobra wymiana — zapisuje znacznik czasu do manifestu i **oszczędza przeglądania godzin materiału w P1**. To najtańsza forma etykietowania: zbierana mimochodem, w trakcie gry.

### Kanał zwrotny

`RemoteFeedback` niesie stan nagrywania, jakość synchronizacji i ostrzeżenia, z haptycznym potwierdzeniem. Haptyka ma przewagę nad głosem przy wietrze i nie przeszkadza na sąsiednim korcie.

### Trasowanie

Apple Watch rozmawia wyłącznie ze **swoim** sparowanym iPhone'em — tak działa WatchConnectivity. Jeśli to slave, komenda idzie dalej po `PeerLink` do mastera. Efekt uboczny jest korzystny: wystarczy być w zasięgu bliższego telefonu, nie mastera.

### Poza zakresem P0a

**Garmin** to trzeci ekosystem — osobny język (Monkey C), osobne SDK, zależność od Garmin Connect Mobile jako pośrednika. Odłożony do P2, traktowany jak Android: najpierw sprawdzenie, czy jest realnie potrzebny. Zestaw `RemoteCommand` jest na niego gotowy.

**Żyroskop nadgarstkowy jako detektor uderzeń** — zegarek na ręce z rakietą daje bardzo pewny sygnał „ten gracz właśnie uderzył", mocniejszy niż obraz przy zasłonięciu piłki ciałem. Rozwiązuje realny problem P2 z przypisaniem uderzenia. Wymaga zegarka na obu graczach, więc nie może być wymogiem; zapisane jako sygnał wspomagający w P2, obok audio.

## 10. Tożsamość graczy i ustawienie meczu

### Imię wiąże się ze stroną kortu, a strony się zmieniają

Każda kamera pokrywa jedną połowę kortu. Zdanie „kamera A widzi Piotra" jest zatem prawdziwe **tylko do zmiany stron** — po nieparzystych gemach gracze się zamieniają i to samo urządzenie obserwuje już przeciwnika.

Model nie może więc być parą imion. Musi być **wiązaniem gracz ↔ koniec kortu, zmiennym w czasie**. Bez tego punktacja w P2 zacznie przyznawać punkty odwrotnie po pierwszej zmianie stron.

### Zawartość ustawienia meczu

| Pole | Po co |
|---|---|
| Gracze | ogłoszenia głosowe, statystyki, komendy z zegarka |
| Koniec kortu na starcie | wiązanie kamer z graczami |
| **Kto serwuje pierwszy** | wymuszone przez zasady gry, patrz niżej |
| Format | pełne sety, tiebreak przy 6:6, przewaga |

### Kolejność serwisu nie jest opcjonalna

W tenisie **ogłasza się najpierw wynik serwującego**. „Trzydzieści piętnaście" i „piętnaście trzydzieści" opisują ten sam stan gry z perspektywy różnych graczy. Bez wiedzy o tym, kto serwuje, poprawne ogłoszenie wyniku jest niemożliwe — niezależnie od tego, czy imiona zostały podane.

Kolejność serwisu determinuje ponadto rotację przez cały mecz oraz to, do którego pola serwisowego leci piłka. Pole jest zatem obowiązkowe w modelu, nawet gdy gracze pozostają anonimowi.

### Wprowadzanie: lista, nie klawiatura

Gra się z tymi samymi kilkoma osobami, więc aplikacja utrzymuje **trwałą listę znanych graczy**; wybór dwóch to dwa stuknięcia. Imię wpisuje się raz.

Ustawienie odbywa się **przed zawieszeniem telefonu**, bo potem nie da się go zdjąć. Wartości domyślne to „Gracz 1" i „Gracz 2" — **imiona nie mogą blokować startu nagrania**.

### Zegarek zna swojego właściciela

Skoro zegarek ma jednego właściciela, komendy w P2 stają się jednoznaczne bez znajomości stron: **„punkt dla mnie" i „punkt dla przeciwnika"**. Dwa przyciski, bez zastanawiania się, która kamera co obserwuje.

### Zmiana stron

W P2 system wie, kiedy kończy się gem, więc **przewraca wiązanie automatycznie** po nieparzystych gemach. Uzupełnia to komenda korekcyjna z zegarka („zamień strony") na wypadek rozjazdu.

W P0a punktacji nie ma, więc zmiana stron jest po prostu kolejnym tagiem `MarkMoment`. Stuknięcie w zegarek przy zmianie zapisuje znacznik do manifestu, dzięki czemu P1 wie, od którego momentu połówki zamieniły graczy.

### Model od razu pod debla

`MatchSetup` przechowuje **listę graczy z przypisaniem do stron**, nie dwa pola. Debel w P4 dokłada wtedy graczy do listy, zamiast wymuszać przepisanie modelu i wszystkiego, co go używa. Ta sama dyscyplina obowiązuje w module punktacji.

### Podział na podprojekty

| P0a | `MatchSetup` w rdzeniu, lista graczy w UI, pole `match` w manifeście, tag `changeover` |
| P2 | wynik wiązany z imionami, automatyczna zmiana stron, komendy „dla mnie / dla przeciwnika", ogłoszenia z imieniem |

Dane pozostają lokalnie na urządzeniu — bez konta i bez chmury.

## 11. Lokalizacja

### Trzy rodzaje tekstu, trzy różne reguły

| Rodzaj | Gdzie żyje | Tłumaczony? |
|---|---|---|
| Teksty interfejsu | String Catalog w aplikacjach | **tak** |
| Ogłoszenia głosowe | silnik ogłoszeń (P2) | **tak, według reguł języka** |
| Format danych — klucze JSON, wartości enumów | rdzeń | **nigdy** |

Trzeci wiersz jest twardy. Wartości takie jak `"tier": "full"`, `"reason": "link-lost"` czy `"1080p120"` stanowią **słownik protokołu**, nie tekst dla człowieka. Ich przetłumaczenie zepsułoby manifesty i pipeline P1.

### Angielski jako język bazowy

Development language aplikacji to **angielski**; polski jest pierwszą dodaną lokalizacją.

Przy tym ustawieniu brakujące tłumaczenie degraduje się do angielskiego, czyli do języka zrozumiałego dla najszerszego grona. Odwrotny wybór oznaczałby, że użytkownik z angielskim telefonem widzi polskie napisy wszędzie tam, gdzie tłumaczenia zabrakło.

### Błędy rdzenia są typowane, nie napisowe

Komunikaty `Display` typów błędów w rdzeniu pisane są **po angielsku** i przeznaczone wyłącznie dla logów i deweloperów. Interfejs użytkownika **nigdy ich nie wyświetla** — mapuje wariant enuma na zlokalizowany komunikat.

Powód jest konkretny: napisy z rdzenia przechodzą przez granicę FFI. Gdyby były po polsku, użytkownik z angielskim telefonem zobaczyłby polski tekst niezależnie od ustawień. Pola typu `reason: String` w wariantach błędów służą logom, nie prezentacji.

Komentarze w kodzie pozostają po polsku — nie są wyjściem runtime.

### Ogłoszenia to nie interpolacja napisów

Trudność językowa siedzi w ogłoszeniach głosowych, nie w interfejsie, i dlatego należy do P2.

**Odmiana imion.** Angielskie „point for Piotr" to po polsku „punkt dla **Piotra**" — dopełniacz. Imię „Marek" daje „**Marka**", z wypadnięciem *e*. Odmiany nie da się wyliczyć algorytmicznie w sposób pewny.

Rozwiązanie dwutorowe:

1. **Konstrukcje unikające przypadków zależnych** jako domyślne. Tenis ogłasza najpierw wynik serwującego, więc imię często jest zbędne: „trzydzieści piętnaście", „gem — Piotr" (mianownik). Zawsze poprawne.
2. **Opcjonalne pole dopełniacza** w `Player`, wypełniane raz przy dodawaniu gracza do listy. Gdy jest obecne, ogłoszenia mogą brzmieć naturalniej.

**Terminologia bez odpowiednika.** Angielskie „love" to po prostu „zero"; „deuce" to „równowaga"; „advantage" to „przewaga". To nie jest tłumaczenie słowo w słowo, tylko osobny zestaw reguł na język.

Stąd wymaganie architektoniczne dla P2: silnik ogłoszeń **nie interpoluje napisów**, tylko zamienia stan gry na zdanie według reguł danego języka. Wejściem jest struktura opisująca stan, nie tekst.

### Język ogłoszeń niezależny od języka interfejsu

To dwa osobne ustawienia. Gra z polskimi znajomymi przy telefonie ustawionym na angielski jest sytuacją typową, a `AVSpeechSynthesisVoice` i tak wymaga jawnego podania locale.

### Zakres w P0a

- Development language **angielski**, polski jako dodana lokalizacja, String Catalog (`.xcstrings`)
- Komunikaty `Display` w rdzeniu po angielsku; interfejs mapuje warianty błędów
- Format danych nigdy nie lokalizowany
- Opcjonalne pole `genitive` w `Player` — tanie teraz, kosztowne później, bo zmienia schemat manifestu

Silnik ogłoszeń z regułami językowymi powstaje w P2.

## 12. Przebieg sesji

1. Aplikacja uruchomiona na obu telefonach; obie strony ogłaszają się i szukają, łączą się automatycznie.
2. Wybór roli — jawny przełącznik, domyślnie iPhone 16 Pro Max jako master.
3. **Ustawienie meczu na masterze**: wybór dwóch graczy z listy, przypisanie do końców kortu, wskazanie serwującego. Pomijalne — wartości domyślne pozwalają nagrywać od razu.
4. Zawieszenie telefonów i wycelowanie. Podgląd z **poziomicą z IMU** oraz wskazówką kadrowania: kort z marginesem ma wypełnić kadr, bez zapasu na niebo.
5. Uzbrojenie: pomiar sceny, blokada parametrów kamery, raport możliwości.
6. Seria synchronizacyjna. UI pokazuje jakość jedną liczbą (`±1,2 ms`) z sygnalizacją zielony / żółty / czerwony.
7. Master rozpoczyna nagrywanie — z telefonu albo z zegarka; komenda idzie po łączu; oba urządzenia zapisują lokalnie.
8. W trakcie widoczne na telefonie i na zegarku: czas, wolne miejsce, stan termiczny, jakość synchronizacji, stan łącza.
9. Stop — oba urządzenia finalizują zapis i wymieniają końcowy log synchronizacji, tak aby **oba manifesty zawierały komplet**.
10. Eksport na Maca.

## 13. Obsługa awarii

**Zasada nadrzędna: utrata łączności nigdy nie zatrzymuje nagrywania.** Każde urządzenie zapisuje lokalnie i samodzielnie. Łącze służy wyłącznie do komend i synchronizacji.

| Awaria | Zachowanie |
|---|---|
| Zerwanie łącza w trakcie nagrania | Oba urządzenia nagrywają dalej. Transport wznawia w tle. `SyncSession` zapisuje lukę do `syncGaps`. Offset w luce odtwarzany w P1 z modelu liniowego — po to jest skew. UI i zegarek ostrzegają, nic nie przerywają. |
| Slave nie otrzymał komendy stop | Brak pingów przez **120 s** kończy sesję po jego stronie z poprawnie zapisanym manifestem. |
| Brak miejsca na dysku | Kontrolowane zamknięcie z zapisanym manifestem. Kontrola przed startem, czy starczy na planowaną długość. |
| Przegrzanie | Ostrzeżenie przy `serious`; przy `critical` czyste domknięcie segmentu i sesji. |
| Telefon szarpnięty na ogrodzeniu | `MotionAnalyzer` wykrywa skok powyżej progu i zapisuje `motion-spike`. W P1 to marker „tutaj przelicz homografię od nowa". |
| Przerwanie sesji kamery | Obsługa przerwania `AVCaptureSession`, zdarzenie `capture-interrupted`, automatyczne wznowienie. |
| Utrata łączności z zegarkiem | Nagrywanie trwa. Sterowanie wraca na telefon. |
| Wygaśnięcie apki | Data wygaśnięcia provisioningu widoczna na ekranie głównym. |

## 14. Testy

Praca prowadzona metodą TDD. Podział modułów jest podporządkowany testowalności: cała logika mieszka w rdzeniu Rust, poza warstwą sprzętową.

**`ClockSyncEstimator`** — czysta matematyka, w pełni testowalna bez sprzętu. Syntetyczne serie `(t1, t2, t3, t4)` o zadanym offsecie, skew i jitterze; weryfikacja odzyskanych parametrów. Przypadki brzegowe: wartości odstające, gubione pakiety, skokowa zmiana opóźnienia, luka w serii. Najważniejszy moduł P0a i zarazem najłatwiejszy do przetestowania.

**`ClockModel`** — konwersja w obie strony, dokładność przy dużym skew, zamknięcie pętli.

**`PeerLink`** — protokół komend na atrapie transportu: zerwanie, wznowienie, duplikaty, pakiety poza kolejnością, uszkodzone pakiety.

**`SyncSession`** — harmonogram pingów, kojarzenie odpowiedzi, luki przy zerwaniu łącza.

**`SessionRecorder`** — maszyna stanów `idle → armed → recording → finalizing → stopped` wraz ze ścieżkami błędów, na atrapach kamery i systemu plików. Każda awaria z sekcji 11 jako scenariusz testowy.

**`SessionManifest`** — round-trip serializacji i walidacja kompletności.

**`CapabilityReport`** — mapowanie surowych odczytów na poziom pracy, przypadki brzegowe każdej bramki.

**`MotionAnalyzer`** — próg, okres refrakcji, norma wektora.

**Warstwa natywna** (`CaptureRig`, transporty, `WatchBridge`) — świadomie pozbawiona logiki, nietestowana jednostkowo; weryfikowana ręcznie i testem akceptacyjnym.

**Test akceptacyjny P0a** — dwa telefony na biurku, ekran Maca z licznikiem milisekundowym, dwie minuty nagrania, skrypt w Pythonie sprawdzający, czy klatki uznane za jednoczesne pokazują ten sam czas. Wykonalny bez wychodzenia z domu.

Ten sam skrypt wczytuje sesję z obu telefonów i sprawdza spójność manifestów, wspólną oś czasu oraz ciągłość segmentów. Stanowi zalążek pipeline'u P1.

## 15. Warunki nagrywania i zadanie pomiarowe kadru

Odłożone do pierwszego wyjścia na kort. Zapisane tutaj, żeby nie zniknęło.

### Nie sprzątaj kortu

Piłki mają leżeć tam, gdzie normalnie leżą — pod siatką, w narożnikach. Materiał treningowy pokazujący wysprzątany kort nauczy detektor świata, który nie istnieje.

Uzasadnienie techniczne: nieruchoma piłka jest **z natury odfiltrowana**. Detektor typu TrackNet dostaje stos trzech kolejnych klatek właśnie dlatego, że ruch jest sygnałem; leżąca piłka daje identyczne piksele i słabą odpowiedź. Różnicowanie klatek widzi ją jako tło. Problemem są dopiero piłki **w ruchu**: toczące się, odbijane przed serwisem, przylatujące z sąsiedniego kortu — i te rozróżnia test paraboli (składowa pionowa i przyspieszenie bliskie −9,81 m/s²).

Podczas nagrywania warto używać `MarkMoment`, gdy obca piłka wejdzie w kadr — powstaje gotowa lista trudnych przypadków do sprawdzenia w P1. Ten sam mechanizm z tagiem `changeover` odnotowuje zmiany stron, bez których wiązanie graczy z kamerami rozjeżdża się w połowie materiału.

### Kadrowanie

Kamera 3 m, obiektyw szeroki, kadr poziomy: pole widzenia w pionie około 40°, oś pochylona około 15° w dół. Górna krawędź kadru sięga wtedy na drugiej stronie kortu **około 4 m wysokości**, podczas gdy lob leci 8–12 m.

Kadr pionowy zwiększa pion do około 70° (zasięg ~7 m), ale zmniejsza poziom do 40°, co przy 11 m daje 8 m pokrycia przy połówce długiej na 11,89 m — przestaje obejmować własną połowę kortu. Obiektyw ultraszeroki obejmie wszystko, ale piłka spadnie z ~7 px do ~3 px.

**Wniosek: żadne ustawienie nie obejmie apogeum loba. Kadr poziomy jest poprawny, a wypadanie piłki górą jest wpisane w konstrukcję.** Widoczne pozostaje wznoszenie, opadanie i kozioł; ciągłość toru przez przerwę odtwarza ekstrapolacja balistyczna i podtrzymanie toru w filtrze. Kosztem jest kompletność trajektorii (P3), nie werdykt (P2).

Kadr ma obejmować kort **plus 1–2 m marginesu** za liniami, bez zapasu na niebo.

### Co zmierzyć na pierwszej sesji

Kilka minut nagrania z celowymi lobami i głębokimi piłkami, a następnie pomiar na materiale:

1. Długość przerw, gdy piłka wychodzi górą (w klatkach)
2. Czy kozioł zawsze mieści się w kadrze wraz z marginesem
3. Ile pikseli ma piłka przy dalekiej linii dla każdego profilu
4. Skala dryfu kamery na ogrodzeniu (z `motion.jsonl`)

Odpowiada to empirycznie na pytanie o kadrowanie, pochylenie i wybór profilu, zamiast rozstrzygać je przy biurku.

## 16. Kryteria ukończenia P0a

1. `cargo test` przechodzi w całości.
2. `pytest` narzędzia weryfikującego przechodzi w całości.
3. Aplikacja paruje dwa iPhone'y bez internetu i bez konfiguracji sieciowej.
4. Jakość synchronizacji poniżej 2 ms widoczna w UI i na zegarku.
5. Sesja 30-minutowa nagrywa się na obu urządzeniach bez utraty klatek, z poprawnymi manifestami po obu stronach i identycznym `sessionId`.
6. Wymuszone zerwanie łącza nie przerywa zapisu, luka odnotowana w `syncGaps`.
7. Zegarek startuje i zatrzymuje nagrywanie oraz zapisuje `mark` do manifestu.
8. Raport możliwości generuje się i trafia do manifestu.
9. Ustawienie meczu (dwaj gracze z listy, końce kortu, serwujący) trafia do manifestu obu urządzeń; pominięcie go nie blokuje nagrywania.
10. Test akceptacyjny z licznikiem milisekundowym zaliczony: różnica odczytu w każdej parze klatek nie przekracza jednego okresu klatki.
11. Sesja eksportowalna na Maca i wczytywalna narzędziem weryfikującym.
12. Zweryfikowana dostępność uprawnienia `HotspotConfiguration` na darmowym provisioningu (wynik odnotowany, niezależnie od tego, jaki jest).

## 17. Co P0a przekazuje dalej

**Do P0b:** rdzeń Rust z kompletem testów, protokół łącza, model manifestu, `MatchSetup`, zestaw komend pilota, interfejsy portów zaprojektowane pod ograniczenia Camera2.

**Do P1:**
- Nagrania z dwóch ujęć ze wspólną osią czasu i znaną optyką
- Macierz intrinsics obu kamer — kalibracja wewnętrzna gotowa
- Dane do rozstrzygnięcia wyboru profilu i kadrowania
- Log IMU pozwalający ocenić rzeczywistą skalę dryfu kamery
- Znaczniki `mark` wskazujące trudne fragmenty oraz zmiany stron
- Ustawienie meczu wiążące graczy z końcami kortu, czyli z konkretnymi kamerami
- Skrypt weryfikujący sesję — pierwszy element pipeline'u P1

Do czasu pierwszego wyjścia na kort P1 startuje z dwóch niezależnych źródeł danych: **publicznych zbiorów** do trackingu piłki tenisowej (ujęcie telewizyjne, przydatne jako pretraining) oraz **danych syntetycznych** — renderowana piłka z poprawnym rozmyciem ruchu, komponowana na statycznym zdjęciu docelowego kortu wzdłuż fizycznie poprawnych trajektorii, co daje nieograniczoną liczbę klatek z idealnymi etykietami. Etykietowanie rzeczywistych klatek prowadzone półautomatycznie: ręczne oznaczenie co dziesiątej klatki, interpolacja wzdłuż trajektorii, przegląd i korekta.
