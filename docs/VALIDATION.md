# Prüfstand v0.2.0

## Aktueller Ausbau

- Lokal auf Linux mit Rust 1.91.1: 77 Tests erfolgreich, strenges Clippy und Formatprüfung. Zusätzlich Actionlint, PowerShell-Syntaxprüfung und Abgleich sämtlicher 246 Windows-Abhängigkeits-/Lizenzhinweise erfolgreich.
- Neue negative Protokolltests prüfen Dateiangebote vor Vertrauen, ausdrückliche Annahme, Blockgrößen/Reihenfolge, Manipulation, Wiederholung, kreuzenden Abbruch und gemeinsame Sitzungsgrenzen. Akteurtests prüfen Speicherung vor Empfangsbestätigung, Speicherfehler, Sperr-Generationen, Export und Verlaufslöschung samt Anhängen.
- Tresortests prüfen verschlüsselte Anhänge, Klartextfreiheit, Migration vorhandener Profile, Passwortwechsel, Grenzen, Pfadwechsel und Erhalt beider Kopien bei nicht eindeutigem Commit. Desktop-Einstellungstests benutzen einen isolierten Startup-Zustand; sie verändern keine reale Autostart-Registrierung.
- cargo-audit 0.23.0 mit dem unten genannten RustSec-Datenbankstand: keine klassifizierten Schwachstellen und keine Unsoundness-Meldungen. Die bereits dokumentierte Wartungswarnung `ttf-parser 0.25.1` bleibt.
- Reale Linux-X11-Oberfläche mit künstlichem Profil: Erststart, Einstellungen bei gesperrtem Tresor, öffentliche HTTPS-Update-Prüfung, leere Übertragungsansicht und Strg+L geprüft. Synthetischer Profilname und Passwort fehlen in den erzeugten Dateien/Logs; stdout/stderr leer. Kein Nachweis nativer Windows-Dateidialoge.
- Windows-Lauf [43602210229](https://github.com/747elias/Hushlink/actions/runs/43602210229) für `8e1778bbff04d06611d7cdb7abaf309ec7278d85`: Format, strenges Clippy, 78 Tests, MSVC-Release-Build und Start der echten EXE erfolgreich. Die nachfolgende Verpackung fand eine falsche Versionsannahme über Inno-PE-Metadaten; der Abgleich nutzt jetzt die offizielle Compiler-Engine-Ausgabe `--version`. Vollständiger Installerlauf wird erneut ausgeführt.
- Der verpflichtende Windows-Check baut MSVC, startet die echte EXE, baut/prüft den Inno-Installer und installiert, aktualisiert und deinstalliert ihn im wegwerfbaren Runner-Konto. Veröffentlicht werden nur genau diese geprüften Bytes. Native Ergebnisse werden nach abgeschlossenem Lauf ergänzt; ein grüner Vorgänger-Commit reicht nicht für einen Release.

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
