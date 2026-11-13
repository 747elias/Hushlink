# Hushlink

Hushlink ist ein lokaler, Ende-zu-Ende-verschlüsselter 1:1-Textchat für zwei Windows-11-PCs in Bluetooth-Reichweite. Kein Internet, kein Konto, kein zentraler Server, keine Telemetrie. Die deutsche Oberfläche und die gesamte App-Logik sind in Rust geschrieben.

**Erste Vorabversion:** Die automatischen Prüfungen und Windows-Builds stehen im jeweiligen [Actions-Lauf](https://github.com/747elias/Hushlink/actions). Ein erfolgreicher Build ist kein Bluetooth-Hardwaretest. Der konkrete Zwei-PC-Testplan ist [hier](docs/MANUAL-TEST.md) beschrieben. Es besteht keine externe Sicherheitsprüfung.

## Starten und benutzen

1. Unter [Releases](https://github.com/747elias/Hushlink/releases) die Windows-x64-EXE oder das ZIP herunterladen. Das Repository ist privat; Downloads benötigen einen berechtigten GitHub-Zugang. ZIP zuerst vollständig entpacken. Prüfsumme mit `Get-FileHash .\Hushlink-v0.1.0-windows-x64.exe -Algorithm SHA256` gegen `SHA256SUMS.txt` vergleichen.
2. EXE als normaler Benutzer starten. Windows 11 x64, eine funktionierende Grafikumgebung und ein Bluetooth-Classic-Adapter mit Microsoft-RFCOMM-Unterstützung werden benötigt. Reine BLE-Adapter reichen nicht. Die EXE verwendet die statische MSVC-Laufzeit; weder Rust noch eine zusätzliche App-Registrierung oder ein MSIX-Paket sind erforderlich. Es gibt keinen TCP- oder WLAN-Ersatztransport.
3. Einen Anzeigenamen und ein langes, einzigartiges Passwort (mindestens 12 Zeichen) festlegen. Das Passwort verschlüsselt tatsächlich den lokalen Tresor. **Es gibt keine Passwortwiederherstellung. Ein vergessenes Passwort bedeutet Datenverlust.**
4. Bluetooth auf beiden PCs in den Windows-Einstellungen einschalten. Bei Bedarf beide PCs dort koppeln. Zum erstmaligen Finden Windows-Bluetooth-Einstellungen geöffnet halten; Windows steuert die Sichtbarkeit. Hushlink aktiviert selbst keine dauerhafte Auffindbarkeit.
5. Auf einem PC **Verbindung empfangen** wählen. Auf dem anderen **Geräte suchen**, den richtigen PC und **Verbinden** wählen. Die Suche zeigt Geräte-Kandidaten; erst der Verbindungsaufbau prüft den Hushlink-Dienst. Eingehende Anfragen bewusst annehmen oder ablehnen. Nur eine Verbindung gleichzeitig.
6. Auf beiden PCs den vollständigen Sicherheitscode über einen unabhängigen Weg vergleichen, zum Beispiel persönlich. Ein Gerätename genügt nicht. Auf beiden Seiten bestätigen. Erst dann wird das Schreiben freigegeben. Eine geänderte bekannte Identität blockiert die Verbindung; eine neue Prüfung verlangt eine bewusste Entscheidung.
7. Text schreiben: **Enter** sendet, **Shift+Enter** fügt eine neue Zeile ein. Unicode, Umlaute, Emojis und Zeilenumbrüche werden als UTF-8 übertragen. Ausstehend bedeutet lokal vorgemerkt, gesendet bedeutet in die Verbindung geschrieben und **empfangen** bedeutet durch die andere App nach verschlüsselter Speicherung bestätigt. Das ist keine Lesebestätigung.
8. Kontakte und Verlauf bleiben im verschlüsselten lokalen Tresor erhalten. Kontakt auswählen und **Erneut verbinden** nutzen. Jeder Verbindungsaufbau verwendet eine neue Sitzung. Auch bei bekannten Kontakten muss die aktuelle Verbindung auf beiden Geräten bestätigt werden.
9. **Sperren** oder **Strg+L** beendet Sitzungen und leert die Anzeige. Die automatische Sperre nach 1–120 Minuten ist in den Einstellungen konfigurierbar (Standard 5 Minuten); das Sperren der Windows-Sitzung sperrt Hushlink ebenfalls. In den Einstellungen lässt sich das Passwort ändern, der Verlauf bleibt dabei erhalten.
10. **Verlauf löschen** entfernt den ausgewählten Verlauf aus dem lokalen Tresor. Kopien auf dem anderen PC bleiben bestehen. Sichere physische Löschung auf SSDs wird nicht versprochen.

Die Anwendung ist nicht mit einem vertrauenswürdigen Herausgeberzertifikat signiert. Windows SmartScreen kann warnen. Prüfsummen belegen Dateikonsistenz, keine Herausgeberidentität. Keine unnötigen Administratorrechte erteilen.

## Lokale Daten und Grenzen

Profile liegen im lokalen Windows-App-Datenordner, nicht neben der portablen EXE. Der genaue Pfad wird durch `directories::ProjectDirs` für Hushlink bestimmt (`%LOCALAPPDATA%\Hushlink\Hushlink\data\profile.vault`). Eine lokale Sperrdatei enthält keine Geheimnisse. Keine unverschlüsselten Backups, Chat-Logs oder Crash-Reports werden durch Hushlink erzeugt. Windows kann unabhängig davon Speicherabbilder, Auslagerungsdateien und Systembackups erzeugen.

Den verschlüsselten Tresor nur bei geschlossener App sichern. Backups benötigen dasselbe Passwort. Eine beschädigte oder von aussen geänderte Datei wird nicht automatisch ersetzt. Betriebssystemverschlüsselung wie BitLocker ergänzt den App-Tresor. Ein entsperrtes oder kompromittiertes Gerät bleibt ein Risiko. [SECURITY.md](SECURITY.md) beschreibt das Bedrohungsmodell und die Grenzen einschliesslich Metadaten und lokal gespeicherter Nachrichten.

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
- **Profil bereits geöffnet:** Andere Hushlink-Instanz schliessen. Eine verbleibende leere Sperrdatei ist normal; die Betriebssystem-Dateisperre wird beim Prozessende freigegeben.
- **Tresor voll:** Alte lokale Verläufe löschen. Grenzen: 16 MiB verschlüsselter Tresor, 10.000 Nachrichten, 500 Kontakte, 16 KiB Text pro Nachricht. Sitzungen sind ebenfalls begrenzt; bei einer langen Unterhaltung neu verbinden.

## Architektur und Mitwirkung

- `src/ui.rs`: egui/eframe, deutsche Oberfläche und UI-Sperrlifecycle.
- `src/runtime.rs`: Hintergrundakteur, Tresorlebenszyklus, Verbindungen und Nachrichtenstatus.
- `src/transport/`: natives Windows-Bluetooth-Classic/RFCOMM; kein IP-Fallback.
- `src/protocol.rs`: Noise-Sitzung, gegenseitige Vertrauensbestätigung und begrenzte Frames.
- `src/vault.rs`: authentifizierte Verschlüsselung, Passwortwechsel und atomare Speicherung.

[Protokoll](docs/PROTOCOL.md) · [Tresorformat](docs/VAULT.md) · [Zwei-PC-Test](docs/MANUAL-TEST.md) · [Änderungen](CHANGELOG.md) · [Projektregeln](AGENTS.md).

Versionstags führen vor der Veröffentlichung dieselben Prüfungen und einen neuen Windows-Release-Build für genau den Tag-Commit aus. Erst erfolgreiche Checks erlauben Verpackung und Veröffentlichung. Release-Artefakte enthalten EXE, ZIP und geprüfte SHA-256-Werte.
