# Hushlink v1.1.0 – direkte Gruppenchats

Unsignierter Windows-11-x64-Release, vom Maintainer angefordert. SmartScreen
kann warnen; Prüfsummen ersetzen keine Herausgebersignatur.

- Gruppen mit bis zu acht unabhängig bestätigten Kontakten, Einladungen und
  ausdrücklichem Beitritt; Besitzer verwaltet die Mitgliederliste.
- Direkter verschlüsselter Textversand an jedes Mitglied, dauerhaftes Outbox-
  und Receipt-Verfahren. Offline-Mitglieder bleiben ausstehend; Bestätigungen
  je Empfänger belegen Speicherung durch die App, keine Lesebestätigung.
- Sichtbare Gruppensynchronisierung verbindet bekannte Mitglieder nacheinander.
  Bluetooth bleibt eine aktive RFCOMM-Verbindung. Kein Relay, Server, Internet-
  Ersatz oder eigener Gruppenverschlüsselungsalgorithmus.
- Vor dem Beitritt alle anderen Mitglieder paarweise als Kontakte prüfen.
  Neue Mitgliederlisten erneut annehmen. Entfernen wird bei unerreichbaren
  Geräten erst nach Zustellung der neuen Liste wirksam; vorhandene Kopien bleiben.
- Gruppentext; Dateien/Antworten/Reaktionen bleiben im bestehenden 1:1-Chat.

Alle PCs benötigen v1.1 mit Protokoll 5. Tresorschema 5 liest ältere Profile;
vor dem Update eine geschlossene verschlüsselte Sicherung behalten, ältere
Apps können das neu gespeicherte Schema nicht öffnen. Sperren stoppt auch
Gruppensynchronisierung. Der Updater behält seine Herausgeberprüfung, diese
unsignierte Version manuell installieren.

Automatische Tests, Windows-Build und native Installer-Abnahme sind kein
Bluetooth-Hardwaretest. Reale Gruppentests mit mindestens drei Windows-11-PCs,
Kamera/Hello, Win+L und DPI bleiben manuell erforderlich. Keine externe
Sicherheitsprüfung, keine zugesicherte Anonymität.

## Prüfstand

113 automatische Tests auf Linux, Format, Clippy mit `-D warnings`, Abhängigkeits-
notizen und Workflow-Lint geprüft. Dazu reale Linux-UI-Bedienung mit getrennten
künstlichen Profilen bei normalem und kleinem Fenster: Anlegen/Beitreten, Enter/
Shift+Enter, Sperren, Neustart und genau eine dauerhaft gespeicherte mehrzeilige
Nachricht; kein Klartext in Tresor/Logs. Negative Tests prüfen insbesondere
unbekannte Kontakte, fehlende Zustimmung, falsche Besitzer/Revisionen,
Manipulation, verlorene ACKs, frische Noise-Verbindungen, Wiederholung nach
Verlaufslöschung und fehlgeschlagene Speicherung ohne Bestätigung.

Der Release-Workflow muss zusätzlich sämtliche Windows-Tests, den nativen
MSVC-Release-Build, Start-/PE-/Paketprüfung und Installation/Tray/Autostart/
Update-Übergabe/Deinstallation für genau den Tag-Commit bestehen. Veröffentlicht
werden nur die bereits nativ geprüften Installerbytes. Anschliessend werden
EXE, Setup, ZIP und SHA256SUMS öffentlich heruntergeladen und gegen Tag-Commit,
Prüfsummen, PE-Format und ZIP-Inhalt geprüft. Diese Schranken ersetzen keinen
physischen Bluetooth-Test. [Actions](https://github.com/747elias/Hushlink/actions).
