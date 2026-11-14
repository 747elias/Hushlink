# Changelog

## 0.2.0 – Dateien und Windows-Installation

- Authentifizierter Bluetooth-Dateiversand bis 20 MiB mit ausdrücklicher Annahme, Fortschritt, Abbruch und bestätigter verschlüsselter Speicherung; bewusster Export statt automatischem Öffnen.
- Alle privaten Daten einschließlich Anhängen in einem verschlüsselten Tresor; Speicherortwahl, sichere Verlagerung und Migration von v0.1-Profilen.
- Per-user Windows-Installer, Startmenü und Deinstallation mit Erhalt der Tresore; Systemtray, Einzelinstanz und wählbarer gesperrter Autostart.
- Abschaltbare automatische GitHub-Update-Prüfung und geprüfte Downloads; Installation und Neustart nach bewusster Auswahl, mit sicherer Prozessübergabe.
- Protokollversion 2, erweiterte negative Tests und verpflichtende native Windows-Installer-/Update-Prüfungen vor Veröffentlichung.

Prüfstatus und verbleibende Hardwaretests: [Release-Hinweise](docs/RELEASE-v0.2.0.md), [Prüfstand](docs/VALIDATION.md). Keine externe Sicherheitsprüfung oder Authenticode-Signatur. Beide Chatpartner benötigen v0.2. Vor der Tresormigration eine geschlossene Sicherung behalten.

## 0.1.0 – erste Vorabversion

- Deutsche Rust-Desktopoberfläche für lokalen 1:1-Textchat mit Bluetooth Classic/RFCOMM auf Windows 11 x64.
- Eigener SDP-Dienst, bewusste Gerätesuche und Empfang, eingehende Anfragen und Wiederverbindung.
- Ende-zu-Ende-verschlüsselte Sitzungen, vollständiger Sicherheitscode, beidseitige Bestätigung und Sperre bei bekanntem Identitätswechsel.
- Passwortgeschützter authentifiziert verschlüsselter lokaler Tresor mit Kontakten, Verlauf, atomarem Speichern und Passwortwechsel.
- Unicode, mehrzeilige Nachrichten und authentifizierte Empfangsbestätigungen; manuelle, Inaktivitäts- und Windows-Sitzungssperre.
- Gated GitHub-Actions-Prüfungen und native Windows-MSVC-Auslieferung als EXE und ZIP mit SHA-256-Werten.

Prüfstatus: Automatische Ergebnisse sind im zugehörigen Release-Actions-Lauf nachprüfbar. Physische Zwei-PC-Bluetooth-Tests und eine interaktive Windows-UI-Abnahme wurden in der Linux-Cloud-Umgebung nicht durchgeführt. Keine externe Sicherheitsprüfung, kein Authenticode-Herausgeberzertifikat. Siehe SECURITY.md und docs/MANUAL-TEST.md.
