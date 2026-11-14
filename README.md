# Hushlink

Hushlink ist ein lokaler, Ende-zu-Ende-verschlüsselter 1:1-Chat mit Dateiversand für zwei Windows-11-PCs in Bluetooth-Reichweite. Nachrichten und Dateien benötigen weder Internet noch Konto oder zentralen Server. Nur die abschaltbare Update-Funktion nutzt Internet. Keine Telemetrie. Die deutsche Oberfläche und die App-Logik sind in Rust geschrieben.

**Erste Vorabversion:** Die automatischen Prüfungen und Windows-Builds stehen im jeweiligen [Actions-Lauf](https://github.com/747elias/Hushlink/actions). Ein erfolgreicher Build ist kein Bluetooth-Hardwaretest. Der konkrete Zwei-PC-Testplan ist [hier](docs/MANUAL-TEST.md) beschrieben. Es besteht keine externe Sicherheitsprüfung.

## Starten und benutzen

1. Unter [Releases](https://github.com/747elias/Hushlink/releases) **Hushlink-v0.2.0-windows-x64-setup.exe** herunterladen, SHA-256 mit `Get-FileHash` gegen `SHA256SUMS.txt` vergleichen und als normaler Benutzer installieren. Installation unter `%LOCALAPPDATA%\Programs\Hushlink`, mit Startmenü und Deinstallation. Alternativ die separate EXE direkt starten oder das ZIP vollständig entpacken. Öffentliche Downloads benötigen kein GitHub-Konto.
2. EXE als normaler Benutzer starten. Windows 11 x64, eine funktionierende Direct3D-12-Grafikumgebung (gegebenenfalls Windows-Softwareadapter) und ein Bluetooth-Classic-Adapter mit Microsoft-RFCOMM-Unterstützung werden benötigt. Reine BLE-Adapter reichen nicht. Die EXE verwendet die statische MSVC-Laufzeit; weder Rust noch eine zusätzliche App-Registrierung oder ein MSIX-Paket sind erforderlich. Es gibt keinen TCP- oder WLAN-Ersatztransport.
3. Einen Anzeigenamen und ein langes, einzigartiges Passwort (mindestens 12 Zeichen) festlegen. Das Passwort verschlüsselt tatsächlich den lokalen Tresor. **Es gibt keine Passwortwiederherstellung. Ein vergessenes Passwort bedeutet Datenverlust.**
4. Bluetooth auf beiden PCs in den Windows-Einstellungen einschalten. Bei Bedarf beide PCs dort koppeln. Zum erstmaligen Finden Windows-Bluetooth-Einstellungen geöffnet halten; Windows steuert die Sichtbarkeit. Hushlink aktiviert selbst keine dauerhafte Auffindbarkeit.
5. Auf einem PC **Verbindung empfangen** wählen. Auf dem anderen **Geräte suchen**, den richtigen PC und **Verbinden** wählen. Die Suche zeigt Geräte-Kandidaten; erst der Verbindungsaufbau prüft den Hushlink-Dienst. Eingehende Anfragen bewusst annehmen oder ablehnen. Nur eine Verbindung gleichzeitig.
6. Auf beiden PCs den vollständigen Sicherheitscode über einen unabhängigen Weg vergleichen, zum Beispiel persönlich. Ein Gerätename genügt nicht. Auf beiden Seiten bestätigen. Erst dann wird das Schreiben freigegeben. Eine geänderte bekannte Identität blockiert die Verbindung; eine neue Prüfung verlangt eine bewusste Entscheidung.
7. Text schreiben: **Enter** sendet, **Shift+Enter** fügt eine neue Zeile ein. Unicode, Umlaute, Emojis und Zeilenumbrüche werden als UTF-8 übertragen. Ausstehend bedeutet lokal vorgemerkt, gesendet bedeutet in die Verbindung geschrieben und **empfangen** bedeutet durch die andere App nach verschlüsselter Speicherung bestätigt. Das ist keine Lesebestätigung.
8. Kontakte und Verlauf bleiben im verschlüsselten lokalen Tresor erhalten. Kontakt auswählen und **Erneut verbinden** nutzen. Jeder Verbindungsaufbau verwendet eine neue Sitzung. Bei bereits bestätigten Kontakten prüft jede App den gespeicherten Schlüssel und bestätigt die neue Sitzung automatisch. Neue oder geänderte Identitäten benötigen wieder eine bewusste Prüfung.
9. **Sperren** oder **Strg+L** beendet Sitzungen und leert die Anzeige. Die automatische Sperre nach 1–120 Minuten ist in den Einstellungen konfigurierbar (Standard 5 Minuten); das Sperren der Windows-Sitzung sperrt Hushlink ebenfalls. In den Einstellungen lässt sich das Passwort ändern, der Verlauf bleibt dabei erhalten.
10. **Verlauf löschen** entfernt den ausgewählten Verlauf aus dem lokalen Tresor. Kopien auf dem anderen PC bleiben bestehen. Sichere physische Löschung auf SSDs wird nicht versprochen.
11. **Datei senden** wählt eine Datei bis 20 MiB. Die andere Person muss das Angebot ausdrücklich annehmen. Fortschritt und Abbruch stehen unter **Dateiübertragungen**. Erst nach bestätigter Speicherung erscheint die Datei als empfangen. Anhänge bleiben verschlüsselt im Tresor; **Speichern unter** erzeugt bewusst eine entschlüsselte Kopie. Dateien werden nie automatisch geöffnet. Vorsicht bei unbekannten Dateien.
12. In **Einstellungen** den Tresorpfad wählen: gesperrt einen vorhandenen Tresor oder einen Speicherort für ein neues Profil auswählen, entsperrt den Tresor nach dem Trennen sicher verschieben. Alle privaten Daten bleiben zusammen in einer Datei. Bestehende Zieldateien werden nicht überschrieben.
13. Dort auch Windows-Autostart, Schliessen ins Tray und automatische Updates einstellen. Autostart öffnet einen gesperrten Tresor. Schliessen ins Tray sperrt zuerst und beendet Verbindungen; über das Tray-Menü **Beenden** vollständig schliessen. Updates werden beim Start und alle sechs Stunden geprüft/heruntergeladen, installiert erst nach **Update installieren und neu starten**. Dies benötigt Internet; ausgeschaltet bleibt der Chat vollständig offline nutzbar.

Die Anwendung ist nicht mit einem vertrauenswürdigen Herausgeberzertifikat signiert. Windows SmartScreen kann warnen. Prüfsummen belegen Dateikonsistenz, keine Herausgeberidentität. Keine unnötigen Administratorrechte erteilen.

## Lokale Daten und Grenzen

Der Standardpfad liegt im lokalen Windows-App-Datenordner (`%LOCALAPPDATA%\Hushlink\Hushlink\data\profile.vault`), nicht neben der EXE. In den Einstellungen lässt sich der Speicherort wählen. Profil, private Schlüssel, Kontakte, Nachrichten und Anhänge liegen gemeinsam in dieser verschlüsselten Datei. `hushlink-settings.json` im Standardordner enthält ausschließlich den Startverweis auf den Pfad und nicht geheime Desktop-Einstellungen. Eine leere `.lock`-Datei enthält keine Chatdaten. Nur ausdrücklich exportierte Anhänge erzeugen Klartextdateien. Keine automatischen Klartext-Backups, Chat-Logs oder Crash-Reports. Windows kann unabhängig davon Speicherabbilder, Auslagerungsdateien und Systembackups erzeugen.

Den verschlüsselten Tresor nur bei geschlossener App sichern. Backups benötigen dasselbe Passwort. Eine beschädigte oder von aussen geänderte Datei wird nicht automatisch ersetzt. Betriebssystemverschlüsselung wie BitLocker ergänzt den App-Tresor. Ein entsperrtes oder kompromittiertes Gerät bleibt ein Risiko. [SECURITY.md](SECURITY.md) beschreibt das Bedrohungsmodell und die Grenzen einschliesslich Metadaten und lokal gespeicherter Nachrichten.

Vor dem Wechsel von v0.1 auf v0.2 den geschlossenen Tresor sichern. v0.2 liest vorhandene Profile, speichert jedoch ein erweitertes Format, das v0.1 sicher abweist. Beide Chatpartner müssen v0.2 verwenden; der versionierte Protokollkontext verlangt einen neuen Codevergleich. Deinstallation entfernt die App, bewahrt Tresore und Einstellungen.

## Bauen

Voraussetzung: Rust über rustup sowie unter Windows Visual Studio 2022 Build Tools mit **Desktopentwicklung mit C++** und Windows SDK. `rust-toolchain.toml` fixiert Rust; `Cargo.lock` fixiert die vollständige Abhängigkeitsauflösung. Die GitHub Actions bauen das endgültige Binary mit MSVC auf einem echten Windows-Runner.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo fmt --check
cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo test --locked --target x86_64-pc-windows-msvc
cargo build --locked --release --target x86_64-pc-windows-msvc
.\target\x86_64-pc-windows-msvc\release\hushlink.exe
```

Unter Linux lassen sich die plattformunabhängigen Protokoll-, Tresor- und Akteurtests ausführen. Die Desktop-App zeigt dort ausdrücklich fehlende Windows-Bluetooth-Unterstützung an. Der Testtransport ist strikt auf Tests beschränkt. Ein Linux-Build ersetzt weder MSVC noch Hardwaretests.

## Fehlerbehebung

- **Kein Adapter/Funk deaktiviert:** Windows-Bluetooth-Einstellungen und Gerätemanager prüfen, Herstellertreiber aktualisieren, sicherstellen, dass Classic/RFCOMM unterstützt wird. Ein USB-BLE-Dongle ist nicht automatisch geeignet.
- **Kein Gerät sichtbar:** Beide Geräte in Reichweite bringen, Windows-Sichtbarkeit/Pairing prüfen, Empfang auf dem anderen Hushlink aktivieren und Suche bewusst wiederholen. Funkumgebungen können die Reichweite stark reduzieren.
- **Dienst fehlt/Verbindung scheitert:** Auf der Gegenstelle Hushlink-Empfang aktivieren. Windows-Pairing abschliessen. Der stabile eigene Dienst wird per SDP registriert; keine fremden seriellen COM-Dienste verwenden.
- **Windows-Zugriff verweigert:** Windows-Bluetooth-Datenschutz-, Geräte- und Unternehmensrichtlinien prüfen. Hushlink umgeht keine Richtlinien. Details zur nativen API und benötigten Voraussetzungen stehen in [BLUETOOTH.md](docs/BLUETOOTH.md).
- **Zeitlimit/Unterbruch:** Empfang stoppen, trennen und neu verbinden. Ein neuer Handshake ist erforderlich; Nachrichten ohne Empfangsbestätigung können nach einem Abbruch als fehlgeschlagen erscheinen, obwohl die andere Seite sie bereits erhalten hat. Kein automatisches erneutes Senden.
- **Sicherheitscode verschieden/Identität geändert:** Nicht bestätigen. Verbindung trennen und unabhängig klären. Erst nach bewusster Entscheidung neu prüfen.
- **Falsches Passwort/beschädigter Tresor:** Datei unverändert lassen, Passwort prüfen oder eine eigene gesicherte Kopie wiederherstellen. Keine automatische Neuanlage über eine bestehende Datei. Keine Wiederherstellung durch den Entwickler möglich.
- **Windows-Sperrüberwachung noch nicht bereit:** Kurz warten und erneut versuchen. Wenn die Meldung bleibt, Windows-Sitzungsdienste und Richtlinien prüfen und die App neu starten. Das Profil wird ohne aktive Sperrüberwachung nicht entsperrt.
- **Profil bereits geöffnet:** Andere Hushlink-Instanz schliessen. Eine verbleibende leere Sperrdatei ist normal; die Betriebssystem-Dateisperre wird beim Prozessende freigegeben.
- **Tresor voll:** Alte lokale Verläufe samt Anhängen löschen. Grenzen: 128 MiB verschlüsselter Tresor, insgesamt 80 MiB Anhangsdaten, 1.000 Anhänge, 10.000 Nachrichten, 500 Kontakte, 16 KiB Text pro Nachricht. Sitzungen sind ebenfalls begrenzt; bei einer langen Unterhaltung neu verbinden.
- **Update nicht möglich:** Internet/GitHub-Erreichbarkeit prüfen oder den neuen Installer manuell aus Releases laden. Updates benötigen keine Zugangsdaten. Bei Fehlern bleibt die bisherige App nutzbar. Für manuelle Installation eine noch laufende Instanz über das Tray beenden. Keine Sicherheitswarnung durch Eingabe eines GitHub-Tokens umgehen.

## Architektur und Mitwirkung

- `src/ui.rs`: egui/eframe, deutsche Oberfläche und UI-Sperrlifecycle.
- `src/runtime.rs`: Hintergrundakteur, Tresorlebenszyklus, Verbindungen und Nachrichtenstatus.
- `src/transport/`: natives Windows-Bluetooth-Classic/RFCOMM; kein IP-Fallback.
- `src/protocol.rs`: Noise-Sitzung, gegenseitige Vertrauensbestätigung und begrenzte Frames.
- `src/vault.rs`: authentifizierte Verschlüsselung, Passwortwechsel und atomare Speicherung.
- `src/storage_location.rs`: minimaler Startverweis und sichere Tresorverlagerung.
- `src/desktop/`, `src/updater.rs`: Windows-Tray, Autostart und begrenzte HTTPS-Update-Downloads.

[Prüfstand](docs/VALIDATION.md) · [Interne Prüfung](docs/SECURITY-REVIEW.md) · [Protokoll](docs/PROTOCOL.md) · [Tresorformat](docs/VAULT.md) · [Zwei-PC-Test](docs/MANUAL-TEST.md) · [Änderungen](CHANGELOG.md) · [Projektregeln](AGENTS.md).

Versionstags führen vor der Veröffentlichung dieselben Prüfungen und einen neuen Windows-Release-Build samt Installer-Abnahme für genau den Tag-Commit aus. Erst erfolgreiche Checks erlauben Veröffentlichung der exakt geprüften Dateien. Release-Artefakte enthalten Setup-EXE, direkt startbare EXE, ZIP und geprüfte SHA-256-Werte. [Installation](installer/README.md) · [Update-Vertrauen und Grenzen](docs/UPDATES.md).
