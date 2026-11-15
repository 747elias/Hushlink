# Prüfstand v0.4.0 – noch kein veröffentlichter Release


Aktueller UI-Ausbau: `feature/messenger-redesign`. Neue lokale Inter-/Icon-
Oberfläche, responsive Navigation/Chats/Formulare/Einstellungen und kompakte
Nachrichtenaktionen. Echte Linux-Live-Prüfung mit getrenntem künstlichem Tresor,
kleinem Fenster, Suche, Sperre und Klartextscan dokumentiert in
[PLAN-DESIGN.md](PLAN-DESIGN.md). Der
[exakte aktuelle Branch-CI-Lauf](https://github.com/747elias/Hushlink/actions/workflows/ci.yml?query=branch%3Afeature%2Fmessenger-redesign)
und dessen Artefakt-Commit bestimmen den jeweiligen Windows-Testbuild. Der
folgende Abschlussstand beschreibt die bereits geprüfte Funktionsbasis vor
diesem UI-Ausbau; Hardware-/Signiergrenzen gelten weiterhin.

Abschlussstand der Implementierung: `9cbf888e3f14156ba52c138a0588aa8a584a9308`.
**102 lokale Tests und 105 Windows-Tests**, Format, strenges Clippy, RFCOMM-Probe,
MSVC-Release-Build, PE-/Paketprüfung und sämtliche Start-/Installer-Smokes grün.
[Exakter Windows-Lauf 43638437534](https://github.com/747elias/Hushlink/actions/runs/43638437534).
App 15'707'136 Byte, Installer 7'798'353 Byte. Kamera-DLLs verzögert geladen,
keine zusätzliche VC++-Laufzeit. Artefakte sind unsignierte Testbuilds.

PowerShell-Signierschranke mit ausdrücklich leerer künstlicher Konfiguration
negativ geprüft: stoppt vor dem Signieren. Veröffentlichung hängt von signierten
Checks und Packaging ab; `contents: write` nur im Publish-Job. Kein v0.4-Tag,
kein Release und keine legitime Signatur vorhanden. Dafür fehlt weiterhin die
externe Dienst-/Identitätsfreigabe des Maintainers, siehe [SIGNING.md](SIGNING.md).
Die folgenden Zwischenstände dokumentieren die Entwicklung; bei späteren reinen
Dokumentationscommits bleibt deren aktueller Branch-CI-Lauf massgeblich.

- Implementiert: Antworten, Bearbeitungen, Reaktionen, optionale Schreibanzeige,
  Suche/Pins, Stummschaltung/Anfragebegrenzung, Rate/Restzeit, Zwischenablage-PNG,
  QR/Kamera, optionaler kryptografisch gebundener Hello-Zugang, verschlüsselte
  Sicherungen/Wiederherstellung und Speicherverwaltung. Protokoll 4, Schema 4,
  optionale authentifizierte Hülle 2.
- Lokaler vollständiger Lauf für `7336f17`: **97 Tests erfolgreich**, Format und
  strenges Clippy. Zusätzliche Regressionen für Besitz/Revisionsbindung, spätes
  Retry nach Löschung, Bitmap-Leeren und unabhängige Native-Lock-Abbrüche sowie
  Einstellungen/Rate-Limit ergänzen den aktuellen Satz auf **101 Linux-Tests**; ein zusätzlicher Akteurtest prüft vollständige Wiederherstellung, falsches Passwort, belegtes Ziel und beschädigte Sicherung.
  Der vollständige lokale Folge-Lauf besteht mit **102 Tests**. Die endgültigen aktuellen Ergebnisse sind im Branch-CI-Lauf massgeblich.
- Windows-Lauf [43634633664](https://github.com/747elias/Hushlink/actions/runs/43634633664)
  für `7336f17bebf2222eb6b03552418edf38f2e62ea1`: **100 Tests erfolgreich**, Format,
  strenges Clippy, RFCOMM-Probe, echter x64-MSVC-Release-Build, sichtbarer EXE-Start,
  Paketprüfung und tatsächliche Installation/Tray/Autostart/Update-Übergabe/
  Deinstallation. Windows prüft zusätzlich Renderer, native Tray-Schleife und
  Ablehnung eines unsignierten Update-Binaries.
- Tatsächliche CI-Artefakte dieses Laufs heruntergeladen: App 15'698'944 Byte,
  Installer 7'787'407 Byte, beide x64 PE32+ GUI. SHA-256 und identische EXEs im ZIP
  samt Commit-Marker lokal geprüft. Dies sind **unsignierte Entwicklungsartefakte**,
  keine veröffentlichten Assets. Nur Windows-Systembibliotheken, keine separate
  VC++-Laufzeit. Nachfolgende Verbesserung lädt Kamera-DLLs verzögert; die CI
  kontrolliert normale und verzögerte PE-Importtabellen.
- Reale Linux-X11-App mit ausdrücklich künstlichem separatem Tresor:
  Entsperren, Antwortverweise, Bearbeitungsmarkierung, Reaktionen, Suche/Pins,
  Speicherübersicht und automatische verschlüsselte Sicherung sichtbar geprüft.
  Kontakt-Stummschaltung, erneutes Entsperren sowie Inaktivitäts- und Strg+L-Sperre mit geschlossenen privaten Dialogen geprüft. Profil, Backup und Logs enthalten keine eindeutigen künstlichen Klartextmarker; stdout/stderr sind leer. Keine erfundene Funkverbindung.
- Windows-Lauf [43636751840](https://github.com/747elias/Hushlink/actions/runs/43636751840)
  für `3df3d1bf3ef4967e44a208a9780797308bebc8b7`: **104 Tests**, MSVC-Release-
  Build und vollständige Start-/Installer-Smokes erfolgreich. Die PE-Prüfung
  bestätigt verzögerte Kamera-DLLs und keine separate VC++-Laufzeit. Native
  Job-Objekte und vom UI unabhängige Sperrflags kompilieren/prüfen auf Windows.
  Kamera- und Hello-Hardware wird dadurch weiterhin nicht nachgewiesen.
- Zusätzliche Regression: neue Sicherung muss bei zurückgestellter Systemuhr erhalten bleiben. Fehler zuerst reproduziert, Bereinigung um expliziten Schutz der neuen Kopie erweitert. CNG behandelt ausschliesslich `ERROR_SUCCESS (0)` als Erfolg.
- SignPath-Ablauf, App-/Setup-Signaturprüfung, Zertifikatsbindung im Updater und
  Windows-Prüfung tatsächlich veröffentlichter Downloads vorbereitet. Actionlint
  und PowerShell-Syntax prüfen die Konfiguration. **Keine echte Signatur:**
  Der Nutzer hat noch kein Zertifikat/verifizierten Dienst. Kein v0.4-Tag oder
  Release veröffentlicht. Konkreter externer Schritt: [SIGNING.md](SIGNING.md).
- cargo-audit 0.23.0 mit lokalem RustSec-Stand 13.11.2026: null klassifizierte
  Schwachstellen/Unsoundness, Wartungswarnungen für `ttf-parser 0.25.1` und die
  durch nokhwa genutzte Build-Makrobibliothek `paste 1.0.15`. Keine externe Prüfung.

Nicht durchgeführt: echter Bluetooth-Chat/Dateifortsetzung mit zwei physischen
Windows-11-PCs, Kameraaufnahme/QR-Lesen mit realem Gerät, Hello-PIN/Biometrie und
Provider-Unterstützung, echte Win+L-/Skalierungs-/Benachrichtigungsabnahme sowie
ein Versionsupgrade zwischen zwei signierten installierten Versionen. Ein
synthetischer Schlüsselplatztest beweist keine Hello-Hardwarefunktion.
[Manueller Ablauf](MANUAL-TEST.md).

---

# Archivierter Prüfstand v0.3.0

- Lokal mit Rust 1.91.1: 87 vollständige Tests erfolgreich einschließlich der Regression für abgebrochene Discovery. Strenges Clippy und Format erfolgreich. Die endgültigen Ergebnisse für den Release-Commit sind im Actions-Lauf massgeblich.
- Neue Prüfungen: fortgesetzter Dateiversand in neuer Noise-Sitzung, erneute Zustimmung, ungültige/replayte Resume-Kontrollen, Präfixabweichung, Empfängeridentität, verlorene Speicherbestätigung ohne Duplikate, verschlüsselte Checkpoints vor Sperre und nach abruptem Verlust, v0.2-Migration mit Anhängen/Passwortwechsel, Kontakte/Favoriten und inhaltsfreie Benachrichtigungen über Sperrgenerationen hinweg.
- Drag-&-Drop-Regression: ohne Vertrauen und bei gesperrter App keine Vormerkliste, kein automatisches Senden, Begrenzung, Ablehnung fehlender lokaler Pfade, Leeren bei Kontaktwechsel und Sperre. Discovery-Regression verhindert die Anzeige verspäteter Ergebnisse abgebrochener Suchen.
- Reale Linux-X11-Oberfläche mit ausdrücklich künstlichem separatem Testtresor: Entsperren, Kontakt auswählen und Zähler löschen, lokalen Alias/Favoriten speichern, Geräteübersicht, Ansicht eines verschlüsselt gespeicherten Teilstands, Assistent mit ehrlichem fehlendem Windows-Transport und Sperren per Strg+L geprüft. Keine erfundene Funkverbindung. Stdout/stderr leer.
- Windows-Prüfschranke enthält weiterhin Format, Clippy, vollständige Tests, RFCOMM-API-Probe, MSVC-Release-Build, tatsächlichen EXE-Start, Paketprüfung und Installation, Tray-Start, Autostart, Update-Übergabe sowie Deinstallation des wirklichen Installers. Der zusätzliche native Nachrichtenschleifentest verarbeitet Benachrichtigungssignale ohne egui-Repaints und synthetische Klickereignisse; er beweist keine sichtbare Windows-Mitteilung auf Benutzerhardware.

- Vollständiger Windows-Lauf [43627895522](https://github.com/747elias/Hushlink/actions/runs/43627895522) für `75bae04d3c7489387f47ccf1aeaebcd4111fd6ec`: **89 Tests**, Format, strenges Clippy, MSVC-Release-Build, EXE-Start, Paketprüfung und sämtliche echten Installer-/Tray-/Autostart-/Update-Übergabe-/Deinstallationsprüfungen erfolgreich. Der native Benachrichtigungsschleifentest bestand ohne egui-Repaints.
- Heruntergeladene Windows-Artefakte lokal erneut geprüft: x64-PE32+-GUI-App und x64-Installer, SHA-256, Commit-Marker und identische EXEs im ZIP. Die Importtabelle enthält Windows-Systembibliotheken und keine separate VC-Runtime; das Binary ist ausdrücklich unsigniert.
- Nach erneutem Entsperren des künstlichen UI-Testtresors blieben Alias und Favorit erhalten, alle sensiblen Fenster waren zuvor durch Strg+L geschlossen. Künstliche Profil-/Kontakt-/Passwort-/Anhangstexte fehlen in den 106 erzeugten App-Dateien/Logs; stdout und stderr sind leer.
- Der Tag-Workflow prüft seinen genauen Commit erneut und veröffentlicht ausschließlich die dabei erfolgreich geprüften Installerbytes. Der nachfolgende Job lädt die tatsächlichen veröffentlichten Downloads separat herunter und prüft Commit, Größe, SHA-256, PE-Format und ZIP-Gleichheit. Maßgeblich bleibt der konkrete [Release-Lauf](https://github.com/747elias/Hushlink/actions/workflows/release.yml).

Offen bleiben die Abnahme auf zwei physischen Windows-11-PCs (Chat, Dateifortsetzung, Funkunterbruch, Pairing, Adapterverlust), tatsächliche Win+L-/Explorer-/Skalierungs-/Benachrichtigungsabnahme und ein Upgrade zwischen verschiedenen installierten Versionen auf Benutzerhardware. [Manueller Testplan](MANUAL-TEST.md). Keine externe Sicherheitsprüfung.

---

# Archivierter Prüfstand v0.2.0

## Aktueller Ausbau

- Lokal auf Linux mit Rust 1.91.1: 77 Tests erfolgreich, strenges Clippy und Formatprüfung. Zusätzlich Actionlint, PowerShell-Syntaxprüfung und Abgleich sämtlicher 246 Windows-Abhängigkeits-/Lizenzhinweise erfolgreich.
- Neue negative Protokolltests prüfen Dateiangebote vor Vertrauen, ausdrückliche Annahme, Blockgrößen/Reihenfolge, Manipulation, Wiederholung, kreuzenden Abbruch und gemeinsame Sitzungsgrenzen. Akteurtests prüfen Speicherung vor Empfangsbestätigung, Speicherfehler, Sperr-Generationen, Export und Verlaufslöschung samt Anhängen.
- Tresortests prüfen verschlüsselte Anhänge, Klartextfreiheit, Migration vorhandener Profile, Passwortwechsel, Grenzen, Pfadwechsel und Erhalt beider Kopien bei nicht eindeutigem Commit. Desktop-Einstellungstests benutzen einen isolierten Startup-Zustand; sie verändern keine reale Autostart-Registrierung.
- cargo-audit 0.23.0 mit dem unten genannten RustSec-Datenbankstand: keine klassifizierten Schwachstellen und keine Unsoundness-Meldungen. Die bereits dokumentierte Wartungswarnung `ttf-parser 0.25.1` bleibt.
- Reale Linux-X11-Oberfläche mit künstlichem Profil: Erststart, Einstellungen bei gesperrtem Tresor, öffentliche HTTPS-Update-Prüfung, leere Übertragungsansicht und Strg+L geprüft. Synthetischer Profilname und Passwort fehlen in den erzeugten Dateien/Logs; stdout/stderr leer. Kein Nachweis nativer Windows-Dateidialoge.
- Windows-Lauf [43602210229](https://github.com/747elias/Hushlink/actions/runs/43602210229) für `8e1778bbff04d06611d7cdb7abaf309ec7278d85`: Format, strenges Clippy, 78 Tests, MSVC-Release-Build und Start der echten EXE erfolgreich. Die nachfolgende Verpackung fand eine falsche Versionsannahme über Inno-PE-Metadaten; der Abgleich nutzt jetzt die offizielle Compiler-Engine-Ausgabe `--version`. Vollständiger Installerlauf wird erneut ausgeführt.
- Vollständiger nativer Lauf [43604813268](https://github.com/747elias/Hushlink/actions/runs/43604813268) für `ab2fe0553e9721625edbc7d35a666ff43e3b9abe` erfolgreich: **79 Tests**, Format, Clippy, MSVC-Release-Build, echter EXE-Start, Inno-Setup-Build und Paketprüfung. Der tatsächliche Installer wurde im Runner-Konto installiert; EXE-Hash, Version, Startmenü und Autostart geprüft. Normaler Start zeigte das Hauptfenster, `--tray` verbarg es. Wiederinstallation bewahrte deaktivierten Autostart; die Update-Übergabe wartete auf den noch laufenden Elternprozess; stille Installation startete anschließend gesperrt im Tray. Deinstallation entfernte Programm/Registrierung/Shortcuts/Autostart und erhielt sämtliche künstlichen Benutzerdaten.
- Die native Fensterliste belegt ein verborgenes `Hushlink`-Hauptfenster beim Tray-Start. Windows' sichtbares Hilfsfenster `Winit Thread Event Target` hatte zuvor den ungenauen .NET-Selektor irregeführt; die Regression prüft jetzt ausdrücklich Titel und Prozesskennung. Zusätzlich ist ein möglicher erster eframe-Sichtbarkeitswechsel beim Start berücksichtigt.
- Die heruntergeladenen Vorab-Artefakte wurden lokal erneut auf SHA-256, Windows-PE-Format und identische EXEs im ZIP sowie den Build-Commit geprüft. Das App-Binary ist x64-PE32+-GUI, der Installer ist ein x64-Windows-Programm.
- Der v0.2.0-Tag-Workflow führt dieselben Prüfungen für seinen genauen Commit erneut aus. Er veröffentlicht ausschließlich die Installerbytes aus diesem erfolgreichen Lauf. Anschließend lädt ein unabhängiger Job alle tatsächlich veröffentlichten Assets erneut herunter und prüft Commit, Größen, Hashes und ZIP-Gleichheit. Maßgeblich ist der [Release-Workflow](https://github.com/747elias/Hushlink/actions/workflows/release.yml); diese vorgelagerte Prüfung ersetzt ihn nicht.

Noch offen als reale Hardwareabnahme: Bluetooth-Datei-/Textchat zwischen zwei physischen Windows-11-PCs, Win+L/Adapterverlust, vollständige interaktive Windows-UI-/Skalierungs-/Explorer-Tray-Abnahme und vollständiges Upgrade zwischen zwei verschiedenen installierten Update-Versionen. Der In-Memory-Test ersetzt keinen Funknachweis. [Manueller Ablauf](MANUAL-TEST.md).

---

# Archivierter Prüfstand v0.1.0

Der validierte Funktionsstand ist unten mit Commit und Lauf verlinkt. Der Versionstag führt die automatischen Release-Prüfungen für genau seinen Commit erneut aus. Die Ergebnisse des zugehörigen Release-Workflows sind für die ausgelieferten Dateien massgeblich; ein anderer erfolgreicher Commit reicht nicht aus.

## Lokal durchgeführt

- Linux x86_64, gepinnte Rust-1.91.1-Toolchain: Formatprüfung, strenges Clippy und vollständige gesperrte Tests; 38 Tests bestanden einschliesslich der zusätzlichen Sperr- und Hintergrund-Inaktivitätstests.
- Reale laufende egui-Oberfläche auf X11: Erststart mit künstlichem Profil, verständliche Formularfehler, Neustart/Entsperren, falsches Passwort, manuelle Sperre, Einstellungen und Passwortwechsel. Falsches bisheriges Passwort liess die Testdatei bitgenau unverändert; altes Passwort nach Wechsel abgewiesen, neues akzeptiert.
- Inaktivitätssperre auf eine Minute eingestellt; bei anschliessender Beobachtung nach 83 Sekunden war der Tresor gesperrt und die Anzeige geleert. Kein Ersatz für Win+L-Abnahme.
- Reale Fenstergrössen bis 720×540: Erststart ist scrollbar, Schaltfläche zur Profilerstellung bleibt erreichbar. Keine Behauptung einer Windows-Skalierungsabnahme.
- Künstliche eindeutige Anzeigenamen und Passwörter waren in den erzeugten Tresor-/Sperrdateien sowie Testlogs nicht im Klartext enthalten. Die Protokoll- und Tresortests prüfen zusätzliche eindeutige Nachrichtentexte.
- Eine gezielte Regression prüft exaktes Enter gegenüber Shift+Enter/Alt+Enter und Tastenwiederholung. Shift+Enter löst kein Senden aus.
- Internes Code-/Abhängigkeitsreview mit Korrektur der gemeldeten Unsoundness-Probleme. Verbleibende ttf-parser-Wartungswarnung ist dokumentiert. Keine externe Sicherheitsprüfung.

## Windows und Auslieferung

- Früher isolierter RFCOMM-Durchstich: echter MSVC-Build und Link des Dienst-, Listener-, Annahme- und Initiierungscodes auf Windows2025 erfolgreich ([Run 43435018711](https://github.com/747elias/Hushlink/actions/runs/43435018711)).
- Ein vollständiger Windows-Lauf deckte eine plattformspezifische Testannahme auf: die Klartextprüfung versuchte eine unter Windows verbindlich gesperrte Sidecar-Datei zu lesen. Der Test gibt den Tresor jetzt vor dem Scan frei und prüft weiterhin sämtliche Dateien; kein Test wurde übersprungen.
- Der vollständige Windows-Lauf [43496556227](https://github.com/747elias/Hushlink/actions/runs/43496556227) für `8b283ccc32543c8a53330cea109717ecbaaad95d` bestand Format, strenges Clippy, **39 Tests**, MSVC-Release-Build, Start der tatsächlichen EXE über acht Sekunden, PE-Prüfung und vollständige Probeverpackung. Ein zusätzlicher Regressionstest prüft die tatsächliche Aktivierung des Direct3D-Backends und die dunkle Oberfläche bei einem hellen Windows-Systemthema.
- Der Tag-Workflow baut/prüft erneut, vergleicht den Commit-Marker, überprüft Einzel-EXE und ZIP sowie SHA-256 und veröffentlicht erst dann. Nach Veröffentlichung werden alle Assets separat heruntergeladen und erneut geprüft.

## Nicht durchgeführt

Keine zwei geeigneten physischen Windows-11-PCs verfügbar: kein Nachweis von Funkbetrieb, Pairing, SDP-Erreichbarkeit, Nachrichten über reale Bluetooth-Hardware, Reichweite, Adapterverlust oder Win+L an echter Benutzerhardware. Keine vollständige interaktive Windows-UI-/Skalierungsabnahme. Ein Windows-Runner und ein nur für Tests gebauter In-Memory-Transport sind keine Hardwarebeweise. Der verbindliche manuelle Ablauf steht in MANUAL-TEST.md.

## Unabhängige Artefaktprüfung vor dem Tag

Die tatsächliche Windows-Aufnahme aus dem oben verlinkten grünen Lauf zeigt die
dunkle, lesbare Erststartoberfläche auf einem 1024×768-Desktop. Die EXE blieb
über acht Sekunden aktiv; Standardausgabe und Fehlerausgabe waren leer. Der
separat heruntergeladene Build enthält den korrekten Commit-Marker. Das Binary
ist eine x64-PE32+-GUI-Datei; die Importtabelle enthält Windows-Systembibliotheken
und keine zusätzliche VC-Runtime-Abhängigkeit. Der Zertifikateintrag ist leer,
entsprechend der dokumentierten unsignierten Auslieferung.

Die endgültigen EXE-/ZIP-Prüfsummen werden pro Release neu berechnet. Der
Release-Workflow veröffentlicht nach seinen Prüfungen und lädt die tatsächlichen
Release-Assets anschliessend unabhängig erneut herunter. Der letzte Job speichert
Commit-, Hash-, PE- und ZIP-Gleichheitsnachweise als JSON-Artefakt.
