Erste Hushlink-Vorabversion für Windows 11 x64: direkter Bluetooth-Textchat, beidseitige Sicherheitscodeprüfung, verschlüsseltes lokales Profil, Kontakte und Verlauf, Passwortwechsel und automatische Sperre. Kein Internet, kein Konto, keine Telemetrie.

Downloads:

- `Hushlink-v0.1.0-windows-x64.exe`: eigenständige Windows-x64-App mit statischer MSVC-Laufzeit.
- `Hushlink-v0.1.0-windows-x64.zip`: dieselbe EXE mit Kurzstart, Lizenz und Dokumentation.
- `SHA256SUMS.txt`: SHA-256 beider Downloads.

Voraussetzungen: Windows 11 x64, Bluetooth Classic/RFCOMM mit geeignetem Adapter und Windows-Treiber sowie eine funktionierende Desktop-Grafikumgebung. Bei Bedarf Geräte in Windows koppeln und System-Sichtbarkeit einschalten. Eine Paketregistrierung oder Rust-Installation wird nicht benötigt. Dateien sind unsigniert; Windows SmartScreen kann warnen.

Der Release-Workflow führt vor der Veröffentlichung für genau den Tag-Commit `cargo fmt --check`, strenges Clippy, Tests und einen Windows-MSVC-Release-Build aus. Die Verpackung prüft Commit-Zuordnung, Windows-PE-Format sowie SHA-256 und die Gleichheit von ZIP- und Einzel-EXE. Diese Vorabversion wird nur nach erfolgreichen automatischen Checks veröffentlicht.

**Nicht durchgeführt:** echte Funk-Ende-zu-Ende-Tests auf zwei physischen Windows-11-PCs sowie interaktive Windows-UI-/Skalierungsabnahme. Ein erfolgreiches Kompilieren, ein Testtransport oder ein Windows-Runner beweist keinen funktionierenden Bluetooth-Hardwarezugriff. Der konkrete Ablauf steht in `docs/MANUAL-TEST.md` im Repository.

Keine externe Sicherheitsprüfung. Vergleicht den vollständigen Sicherheitscode unabhängig und bestätigt auf beiden Geräten. Bluetooth-Metadaten bleiben sichtbar; keine zugesicherte Anonymität. Ohne Wiederherstellung führt ein vergessenes Tresorpasswort zu Datenverlust. Forward Secrecy schützt keine bei späterem Tresorzugriff lesbaren lokalen Verläufe. Siehe SECURITY.md.
