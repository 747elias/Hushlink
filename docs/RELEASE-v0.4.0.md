# Hushlink v0.4.0 – unsignierte Vorabversion

Alle vorgesehenen Erweiterungen sind im Entwicklungsbranch implementiert:

- Neue dunkle Messenger-Oberfläche mit Mint-Akzenten, eigener Navigation,
  Chatvorschauen/Suchfiltern, kompakten Blasen und gegliederten Einstellungen.
  Inter und eigene Icons sind lokal verfügbar; keine UI-Netzwerkanfragen.
- Antworten mit Referenz, Emoji-Reaktionen und gekennzeichnete Bearbeitungen;
  verschlüsselte dauerhafte Bestätigungen und sichere explizite Wiederholung.
- Optionale Schreibanzeige, lokale Suche über Kontakte, gemerkte Nachrichten,
  zeitweilige/dauerhafte Stummschaltung und Begrenzung unbekannter Anfragen.
- Bestätigter PNG-Versand aus Zwischenablage/Strg+V, Rate und Restzeit bei Dateien.
- Vollständiger QR-Sicherheitsvergleich mit bewusst gestarteter Kamera.
- Optionaler echter Windows-Hello-Schlüsselplatz mit verpflichtender Benutzer-
  bestätigung und Passwort-Fallback; keine schwächere Ersatzimplementierung.
- Verschlüsselte automatische Sicherungen mit Aufbewahrung, Wiederherstellung
  an einen neuen freien Pfad und Speicherübersicht/gezieltem Löschen.
- Vorbereitete vertrauenswürdige App-/Installer-Signierung und Signaturprüfung
  im Updater sowie zwingende Schranken vor Release-Veröffentlichung.

Beide PCs brauchen Protokoll 4. Vorhandene v0.1–v0.3-Tresore bleiben lesbar und
werden beim erfolgreichen Speichern auf Profilschema 4 erweitert. Hello-Einrichtung
verwendet Hülle 2. Vor der Migration eine verschlüsselte Kopie behalten;
ältere Apps können neu gespeicherte Profile nicht öffnen.

Prüfstatus des Implementierungsabschlusses `9cbf888`: **102 Linux- und 105
Windows-Tests** sowie Format, Clippy, RFCOMM-Probe, MSVC-Release-Build, echter
EXE-Start, Installer/Tray/Autostart/Update-Übergabe und Deinstallation erfolgreich.
Verzögerte optionale Kamera-DLLs, kein separater VC++-Runtime-Import, Schutz neuer
Backups bei zurückgestellter Uhr und vollständiger Restore-Akteurtest geprüft.
Heruntergeladene CI-EXE/Setup/ZIP auf PE, SHA-256, Commit und Identität geprüft;
aktuelle Nachweise: [VALIDATION.md](VALIDATION.md).

**[v0.4.0 ist öffentlich veröffentlicht](https://github.com/747elias/Hushlink/releases/tag/v0.4.0), ohne Herausgebersignatur.**
Der Maintainer hat diese einzelne unsignierte Vorabversion am 15.11.2026
ausdrücklich freigegeben. Die Ausnahme hebt die Signaturpflicht für zukünftige
Releases und die Herausgeberprüfung im Updater nicht auf.

Release-Commit: `79723dcee7b5bd329e88a88e383bf4435e87e477`.
[Erfolgreiche Windows-CI](https://github.com/747elias/Hushlink/actions/runs/43707154440).
Die öffentlichen Downloads wurden nach Veröffentlichung vollständig erneut
heruntergeladen und auf Tag-Zielcommit, SHA-256, PE-Format und identische
ZIP-Binärdateien geprüft. [Maschinenlesbarer Nachweis](releases/v0.4.0-verification.json).

Noch nicht durchgeführt: zwei physische Bluetooth-PCs, reale Kamera, Hello-
Biometrie/PIN samt Providerunterstützung, vollständige Windows-UI-/Skalierungs-
und Win+L-Abnahme sowie Upgrade zwischen signierten Versionen. Keine externe
Sicherheitsprüfung. [Konkreter manueller Testplan](MANUAL-TEST.md).

## Signierlauf der ausdrücklich unsignierten Ausnahme

Die tagbasierte Signierpipeline stoppt bei dieser ausdrücklich freigegebenen unsignierten Ausnahme an der fehlenden Signierkonfiguration ([Signierlauf](https://github.com/747elias/Hushlink/actions/runs/43709817861)). Das ist kein fehlgeschlagener App-Test. Veröffentlicht wurden die unveränderten Dateien aus dem erfolgreichen CI-Lauf des exakten Release-Commits; die tatsächlichen öffentlichen Downloads wurden separat geprüft.
