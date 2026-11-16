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
