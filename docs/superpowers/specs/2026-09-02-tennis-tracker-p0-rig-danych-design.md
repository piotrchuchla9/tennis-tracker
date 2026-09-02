# Tennis Tracker — P0: Rig danych (design)

Data: 2026-09-02
Status: zatwierdzony do planowania implementacji

## 1. Kontekst

Celem całego przedsięwzięcia jest system, który **w trakcie gry** automatycznie prowadzi punktację meczu singlowego. Dwa iPhone'y zawieszone na ogrodzeniu kortu obserwują po jednej połówce, wykrywają piłkę i kozły, a jeden z nich ogłasza stan głosem.

Sprzęt jest ustalony i niezmienny:

- iPhone 16 Pro Max (A18 Pro) — **master**
- iPhone 15 (A16) — **slave**
- Apple Watch — opcjonalny pilot, nie wymagany
- Mac do developmentu, konto Apple Developer w wersji darmowej (apka wygasa co 7 dni, akceptowane)

Ustawienie na korcie: telefony na **przeciwnych liniach bocznych**, każdy w połowie długości swojej połówki kortu, na wysokości **około 3 m**, na sztywnym ogrodzeniu siatkowym. Nawierzchnia: głównie **mączka**.

Format meczu: **singiel**, pełne sety, tiebreak przy 6:6, przewaga (ad).

### Decyzje architektoniczne podjęte przed tym dokumentem

**Natywne iOS (Swift + CoreML), nie aplikacja webowa.** Powody: dostęp do Neural Engine, z przeglądarki nieosiągalny (WebGPU daje kilkukrotnie mniej); pełna kontrola kamery — blokada ekspozycji, ISO, ostrości oraz 120 fps, czego Safari nie udostępnia; łączność peer-to-peer bez internetu i bez serwera sygnalizacyjnego przez MultipeerConnectivity, podczas gdy WebRTC wymagałby serwera na korcie; oraz sprzętowe znaczniki czasu klatek z `CMSampleBuffer.presentationTimeStamp`.

**Fuzja na poziomie zdarzeń, nie klatek.** Każdy telefon prowadzi własny pełny pipeline i wysyła do mastera wyłącznie chude zdarzenia we **współrzędnych kortu w metrach**, nigdy w pikselach. Granicą między urządzeniami są metry. Master nie musi wiedzieć nic o optyce ani kalibracji slave'a. Rozwiązanie skaluje się na debla i na trzecią kamerę.

**Kort jest wzorcem kalibracyjnym.** Wymiary kortu są znormalizowane (23,77 na 10,97 m, linie serwisowe 6,40 m od siatki), więc detekcja linii plus `solvePnP` daje pełną pozę kamery w 3D. Ponieważ obie kamery wyznaczają pozę względem tego samego kortu, **wzajemna poza kamer wychodzi za darmo** — bez szachownicy i bez wspólnej procedury kalibracyjnej.

**Do liczenia punktów stereo nie jest potrzebne.** W chwili kozła piłka dotyka płaszczyzny kortu, więc homografia przenosi punkt z obrazu wprost we współrzędne kortu. Jedna skalibrowana kamera wystarcza do orzeczenia, gdzie piłka odbiła. Stereo jest potrzebne dopiero do pełnej trajektorii 3D.

### Dekompozycja na podprojekty

| | Podprojekt | Zależność |
|---|---|---|
| **P0** | **Rig danych — ten dokument** | brak |
| P1 | Detektor piłki i kalibracja kortu (Python, offline) | dane z P0 |
| P2 | Tablica wyników na żywo (realtime na telefonach) | P1 |
| P3 | Challenge i powtórka 3D | P2 |
| P4 | Debel | P2 |

Kolejność jest odwrócona względem intuicji celowo: detektor piłki jest sercem systemu, a nie da się go wytrenować bez nagrań z tych telefonów, z tego kortu i z tej wysokości. P0 nie zawiera ani grama uczenia maszynowego, więc powstaje niezależnie od dostępu do kortu.

## 2. Zakres P0

### Czym P0 jest

Aplikacja iOS instalowana na oba telefony — jedna binarka, rola wybierana przy starcie. Zadanie: nagrać z dwóch telefonów materiał, z którego da się wytrenować detektor i odtworzyć geometrię. To znaczy wideo o **stałych, znanych parametrach**, ze **wspólną osią czasu** i z **pełnymi metadanymi**.

Sedno: kluczowym artefaktem P0 nie jest wideo, tylko *wideo z wiarygodną osią czasu i znaną optyką*. Nagranie bez tego jest w P1 warte tyle, co film znaleziony w internecie.

P0 jest użyteczny sam w sobie — daje nagrania meczów z dwóch ujęć.

### Czym P0 nie jest (świadome wyłączenia)

- Nie wykrywa piłki
- Nie wykrywa kortu ani linii
- Nie liczy punktów
- Nie zawiera żadnego modelu ML
- Nie obsługuje debla
- Nie przetwarza wideo w czasie rzeczywistym

Każdy z tych elementów wymaga danych, których jeszcze nie ma.

## 3. Moduły

| Moduł | Odpowiedzialność |
|---|---|
| `PeerLink` | Parowanie i transport przez MultipeerConnectivity: wykrycie drugiego telefonu, negocjacja ról, kanał komend, wznawianie po zerwaniu. |
| `ClockSync` | Ciągła estymacja różnicy zegarów metodą NTP-style po `PeerLink`. Wystawia offset **oraz skew, oraz niepewność**. |
| `CaptureRig` | `AVCaptureSession` z zablokowaną ekspozycją, ISO, ostrością i balansem bieli. Zapis wideo wraz z nieskompresowaną ścieżką audio przez `AVAssetWriter`, w segmentach. Włączona dostawa macierzy intrinsics per klatka. |
| `MotionLog` | Zapis żyroskopu i akcelerometru w trakcie nagrania (100 Hz) oraz wykrywanie skoków. |
| `SessionRecorder` | Spina powyższe w sesję: start i stop, segmentacja, nadzór nad stanem termicznym i wolnym miejscem. |
| `SessionManifest` | Model i serializacja sidecara JSON opisującego sesję. |
| `Exporter` | Wyciągnięcie sesji z telefonu na Maca (Files, share sheet, AirDrop). |
| `RigUI` | Podgląd kamery, poziomica z IMU, stan sparowania, jakość synchronizacji, przycisk nagrywania, data wygaśnięcia apki. |

Podział jest tak dobrany, żeby cała logika warta testowania mieszkała w modułach niezależnych od sprzętu (`ClockSync`, `PeerLink`, `SessionRecorder`, `SessionManifest`), a warstwa dotykająca AVFoundation (`CaptureRig`) pozostała możliwie cienka i pozbawiona decyzji.

## 4. Produkt sesji

Katalog na każdym z telefonów:

```
session-2026-09-14-1732/
  manifest.json
  video-000.mov, video-001.mov, ...   segmenty po 5 min
  motion.jsonl                        IMU 100 Hz
```

### Schemat `manifest.json`

```json
{
  "schemaVersion": 1,
  "sessionId": "2026-09-14T17:32:10Z-a3f9",
  "role": "master",
  "peerDeviceModel": "iPhone15,4",
  "device": {
    "model": "iPhone17,2",
    "osVersion": "26.0",
    "appVersion": "0.1.0"
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
    { "file": "video-000.mov", "startHostTime": 123456.789, "endHostTime": 123756.789, "frameCount": 36000 }
  ],
  "events": [
    { "hostTime": 123500.1, "type": "motion-spike", "magnitude": 0.42 },
    { "hostTime": 123610.0, "type": "capture-interrupted", "reason": "system-pressure" },
    { "hostTime": 123700.0, "type": "thermal", "state": "serious" }
  ],
  "motionLog": "motion.jsonl"
}
```

Wszystkie znaczniki czasu są w domenie `mach_absolute_time` **lokalnego** urządzenia, wyrażone w sekundach. Przeliczenie na wspólną oś czasu wykonuje się w P1 przy użyciu `timing.syncModel`. Manifest slave'a i mastera opisują tę samą sesję pod wspólnym `sessionId`.

## 5. Synchronizacja czasu

### Algorytm

Handshake w stylu NTP po `PeerLink`. Master wysyła ping ze swoim `t1`, slave odbiera w `t2`, odsyła `t2` i `t3`, master odbiera w `t4`:

```
offset = ((t2 - t1) + (t3 - t4)) / 2
delay  = (t4 - t1) - (t3 - t2)
```

Zbierana jest seria próbek; zachowywane są **wyłącznie te o najniższym `delay`** — pakiety, które przeszły najszybciej, przeszły najmniej zaburzone. Rozrzut wśród zachowanych próbek stanowi miarę niepewności i trafia do manifestu jako `residualStdMs`.

### Trzy warunki poprawności

**Zegar monotoniczny.** Wyłącznie `mach_absolute_time` albo `CLOCK_MONOTONIC_RAW`, nigdy czas ścienny — systemowa korekta NTP w trakcie nagrania przesunęłaby oś czasu w jego środku. `CMSampleBuffer.presentationTimeStamp` z kamery leży na `CMClockGetHostTimeClock()`, czyli na tej samej podstawie, więc znaczniki klatek są bezpośrednio porównywalne z wynikiem synchronizacji bez żadnej konwersji.

**Model liniowy: offset oraz skew.** Kwarce dwóch urządzeń chodzą z różną prędkością, typowo 1–20 ppm, co przez godzinę daje rozjazd 4–72 ms, czyli do dziewięciu klatek przy 120 fps. Pojedyncza synchronizacja na starcie jest zatem bezużyteczna. Stosowany jest model liniowy — punkt odniesienia, offset i dryf — dopasowywany na bieżąco. Do manifestu trafia zarówno model, jak i surowe próbki.

**Kadencja.** Seria około 50 pingów przy uzbrajaniu sesji (poniżej sekundy), następnie ping co 1 s przez cały czas nagrania.

### Cel dokładności

**Poniżej 2 ms.** Przy 120 fps okres klatki wynosi 8,3 ms, więc 2 ms to ćwierć klatki.

### Walidacja

**Test podstawowy, wykonalny w domu:** oba telefony obok siebie, skierowane na ekran Maca wyświetlający licznik z rozdzielczością milisekundową. Po nagraniu porównuje się czas widoczny na klatkach, które według modelu synchronizacji są jednoczesne. Weryfikuje to cały łańcuch end-to-end: zegar, synchronizację i znaczniki klatek.

**Test niezależny:** pojedyncze klaśnięcie i korelacja skrośna ścieżek audio. Przy telefonach obok siebie różnica czasu propagacji dźwięku jest pomijalna. Na korcie, przy odległości około 13 m, propagacja wnosi do 38 ms — wielkość znana i obliczalna z kalibracji, więc możliwa do odjęcia, co jest istotne dopiero w P1.

## 6. Parametry nagrywania

### Blokada parametrów kamery

Przed startem nagrania następuje krótki pomiar sceny, po czym **ekspozycja, ISO, ostrość i balans bieli zostają zablokowane** na stałe. Autoekspozycja polująca w trakcie wymiany oznacza zmienną jasność między klatkami, co psuje zarówno detekcję, jak i późniejsze różnicowanie klatek. Zablokowane wartości trafiają do manifestu.

### Czas naświetlania ważniejszy niż liczba klatek

Piłka lecąca 40 m/s przy 1/1000 s przesuwa się 4 cm — mniej niż własna średnica, więc pozostaje kropką. Przy 1/250 s przesuwa się 16 cm, czyli rozmazuje się w kreskę długości dwóch i pół piłki. Na mączce w świetle dziennym 1/1000 s jest osiągalne bez trudu.

### Profil domyślny: 1080p przy 120 fps

Uzasadnienie: wysoka liczba klatek sama wymusza krótką ekspozycję, a moment kozła lokalizuje się do plus minus 4 ms zamiast plus minus 8 ms — przy piłce 40 m/s to różnica między 16 cm a 33 cm niepewności miejsca odbicia. Piłka zajmuje szacunkowo około 7 px przy dalekiej linii i około 15 px blisko kamery, co mieści się w zakresie, dla którego architektura TrackNet jest projektowana.

### Profil jako parametr, nie założenie

Wybór "rozdzielczość kontra liczba klatek" **nie jest rozstrzygany w tym dokumencie**. Profil nagrywania jest konfigurowalny i zapisywany w manifeście:

- `1080p120` — domyślny
- `4K60`
- `1080p60`

Porównanie zostanie wykonane w P1 na rzeczywistych nagraniach z docelowego kortu, na podstawie zmierzonej skuteczności detektora. Dostarczenie danych do tej decyzji jest jednym z zadań P0.

### Audio

Każdy segment zawiera **nieskompresowaną ścieżkę audio** (48 kHz, mono, PCM), zapisywaną wraz z wideo w tym samym pliku, dzięki czemu dzieli z nim oś czasu. Audio pełni w P0 rolę niezależnego kanału walidacji synchronizacji (korelacja skrośna klaśnięcia), a w P2 stanie się podstawą detekcji uderzeń rakiety i taśmy siatki. Kompresja jest tu wykluczona, ponieważ interesujące zdarzenia są krótkimi transjentami, a to właśnie one giną w kodekach stratnych.

### Bitrate

Bitrate ustawiany wysoko, bez oszczędzania. Kompresja HEVC traktuje szybką, siedmiopikselową plamkę jak szum i usuwa ją jako pierwszą. Piłka jest dokładnie tym, co kodek wyrzuca najchętniej. Utrata miejsca na dysku jest tańsza niż utrata piłki.

### Segmentacja i zasoby

1080p120 zajmuje około **18 GB na godzinę**. Stąd segmenty po 5 minut: chronią przed utratą całej sesji przy zapchanym dysku lub awarii i ułatwiają transfer na Maca. `SessionRecorder` monitoruje wolne miejsce oraz `ProcessInfo.thermalState`, ostrzega zawczasu, a przy przegrzaniu proponuje zejście na niższy profil zamiast po cichu gubić klatki.

## 7. Przebieg sesji

1. Aplikacja uruchomiona na obu telefonach; obie strony ogłaszają się i szukają przez MultipeerConnectivity, łączą się automatycznie.
2. Wybór roli — jawny przełącznik, domyślnie iPhone 16 Pro Max jako master.
3. Zawieszenie telefonów i wycelowanie. Podgląd z **poziomicą z IMU**.
4. Uzbrojenie: pomiar sceny, blokada parametrów kamery.
5. Seria synchronizacyjna. UI pokazuje jakość jedną liczbą, na przykład `±1,2 ms`, z sygnalizacją zielony, żółty, czerwony — żeby przed startem było wiadomo, czy nagranie będzie użyteczne.
6. Master rozpoczyna nagrywanie; komenda idzie po `PeerLink`; oba urządzenia zapisują lokalnie.
7. W trakcie widoczne: czas nagrania, wolne miejsce, stan termiczny, jakość synchronizacji, stan łącza.
8. Stop — oba urządzenia finalizują zapis i wymieniają końcowy log synchronizacji, tak aby **oba manifesty zawierały komplet danych**.
9. Eksport na Maca.

## 8. Obsługa awarii

**Zasada nadrzędna: utrata łączności nigdy nie zatrzymuje nagrywania.** Każdy telefon zapisuje lokalnie i samodzielnie. Łącze służy wyłącznie do komend i synchronizacji.

| Awaria | Zachowanie |
|---|---|
| Zerwanie łącza w trakcie nagrania | Oba urządzenia nagrywają dalej. `PeerLink` wznawia połączenie w tle. `ClockSync` po powrocie zapisuje lukę do `syncGaps`. Offset w luce odtwarzany w P1 z modelu liniowego — po to jest skew. UI ostrzega, nie przerywa. |
| Slave nie otrzymał komendy stop | Brak pingów przez 2 minuty kończy sesję po jego stronie z poprawnie zapisanym manifestem. |
| Brak miejsca na dysku | Kontrolowane zamknięcie sesji z zapisanym manifestem. Dodatkowo kontrola przed startem, czy starczy miejsca na planowaną długość. |
| Przegrzanie | Ostrzeżenie przy `.serious`; przy `.critical` czyste domknięcie bieżącego segmentu i sesji. |
| Telefon szarpnięty na ogrodzeniu | `MotionLog` wykrywa skok powyżej progu i zapisuje zdarzenie `motion-spike` do manifestu. W P1 stanowi to znacznik "tutaj przelicz homografię od nowa". |
| Przerwanie sesji kamery (połączenie, powiadomienie) | Obsługa przerwania `AVCaptureSession`, zdarzenie `capture-interrupted` w manifeście, automatyczne wznowienie. |
| Wygaśnięcie apki (provisioning 7-dniowy) | Data wygaśnięcia widoczna na ekranie głównym, żeby nie okazało się to na korcie. |

## 9. Testy

Praca prowadzona metodą TDD. Podział modułów jest podporządkowany testowalności: logika mieszka poza warstwą sprzętową.

**`ClockSync`** — czysta matematyka, w pełni testowalna bez sprzętu. Syntetyczne serie `(t1, t2, t3, t4)` o zadanym offsecie, skew i jitterze; weryfikacja odzyskanych parametrów. Przypadki brzegowe: wartości odstające, gubione pakiety, skokowa zmiana opóźnienia łącza, luka w seriach. Najważniejszy moduł P0 i zarazem najłatwiejszy do przetestowania.

**`PeerLink`** — protokół komend na atrapie transportu w pamięci: zerwanie, wznowienie, duplikaty, pakiety poza kolejnością.

**`SessionRecorder`** — maszyna stanów `idle → armed → recording → finalizing` wraz ze ścieżkami błędów, na atrapach kamery i systemu plików. Każda awaria z sekcji 8 jako scenariusz testowy.

**`SessionManifest`** — round-trip serializacji i walidacja kompletności.

**`CaptureRig`** — warstwa świadomie pozbawiona logiki, nietestowana jednostkowo; weryfikowana testem akceptacyjnym.

**Test akceptacyjny P0** — dwa telefony na biurku, ekran Maca z licznikiem milisekundowym, dwie minuty nagrania, skrypt w Pythonie sprawdzający, czy klatki uznane za jednoczesne pokazują ten sam czas. Wykonalny bez wychodzenia z domu.

Ten sam skrypt wczytuje sesję z obu telefonów i sprawdza spójność manifestów, wspólną oś czasu oraz ciągłość segmentów. Stanowi zalążek pipeline'u P1.

## 10. Kryteria ukończenia P0

1. Aplikacja uruchamia się na obu telefonach i paruje je bez konfiguracji sieciowej, poza zasięgiem internetu.
2. Jakość synchronizacji raportowana w UI, wartość poniżej 2 ms osiągana w warunkach normalnych.
3. Sesja 30-minutowa nagrywa się na obu urządzeniach bez utraty klatek, z poprawnymi manifestami po obu stronach.
4. Wymuszone zerwanie łącza w trakcie nagrania nie przerywa zapisu, a luka jest odnotowana w `syncGaps`.
5. Test akceptacyjny z licznikiem milisekundowym potwierdza zgodność osi czasu.
6. Sesja daje się wyeksportować na Maca i wczytać skryptem weryfikującym.

## 11. Co P0 przekazuje do P1

- Nagrania z dwóch ujęć, ze wspólną osią czasu i znaną optyką
- Macierz intrinsics obu kamer — kalibracja wewnętrzna gotowa
- Dane do rozstrzygnięcia wyboru profilu nagrywania
- Log IMU pozwalający ocenić rzeczywistą skalę dryfu kamery na ogrodzeniu
- Skrypt weryfikujący sesję — pierwszy element pipeline'u P1

Do czasu pierwszego wyjścia na kort P1 startuje z dwóch niezależnych źródeł danych: **publicznych zbiorów** do trackingu piłki tenisowej, w ujęciu telewizyjnym, przydatnych jako pretraining, oraz **danych syntetycznych** — renderowana piłka z poprawnym rozmyciem ruchu, komponowana na statycznym zdjęciu docelowego kortu wzdłuż fizycznie poprawnych trajektorii, co daje nieograniczoną liczbę klatek z idealnymi etykietami. Etykietowanie rzeczywistych klatek będzie prowadzone półautomatycznie: ręczne oznaczenie co dziesiątej klatki, interpolacja wzdłuż trajektorii, przegląd i korekta.
