# Changelog

## 0.1.0 – erste Vorabversion

- Deutsche Rust-Desktopoberfläche für lokalen 1:1-Textchat mit Bluetooth Classic/RFCOMM auf Windows 11 x64.
- Eigener SDP-Dienst, bewusste Gerätesuche und Empfang, eingehende Anfragen und Wiederverbindung.
- Ende-zu-Ende-verschlüsselte Sitzungen, vollständiger Sicherheitscode, beidseitige Bestätigung und Sperre bei bekanntem Identitätswechsel.
- Passwortgeschützter authentifiziert verschlüsselter lokaler Tresor mit Kontakten, Verlauf, atomarem Speichern und Passwortwechsel.
- Unicode, mehrzeilige Nachrichten und authentifizierte Empfangsbestätigungen; manuelle, Inaktivitäts- und Windows-Sitzungssperre.
- Gated GitHub-Actions-Prüfungen und native Windows-MSVC-Auslieferung als EXE und ZIP mit SHA-256-Werten.

Prüfstatus: Automatische Ergebnisse sind im zugehörigen Release-Actions-Lauf nachprüfbar. Physische Zwei-PC-Bluetooth-Tests und eine interaktive Windows-UI-Abnahme wurden in der Linux-Cloud-Umgebung nicht durchgeführt. Keine externe Sicherheitsprüfung, kein Authenticode-Herausgeberzertifikat. Siehe SECURITY.md und docs/MANUAL-TEST.md.
