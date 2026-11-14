# Hushlink v0.4.0 – Implementierung, Veröffentlichung blockiert

Alle vorgesehenen Erweiterungen sind im Entwicklungsbranch implementiert:

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

**Kein veröffentlichter v0.4-Release und keine Herausgebersignatur.** Der Nutzer
hat noch kein Code-Signing-Zertifikat oder verifizierten Signierdienst.
[SIGNING.md](SIGNING.md) enthält den konkreten eingerichteten Ablauf und die
noch nötige externe Konto-/Identitätsfreigabe. Unsigned CI-Artefakte werden
nicht als signierter Release ausgegeben. Es wurde kein Tag erstellt.

Noch nicht durchgeführt: zwei physische Bluetooth-PCs, reale Kamera, Hello-
Biometrie/PIN samt Providerunterstützung, vollständige Windows-UI-/Skalierungs-
und Win+L-Abnahme sowie Upgrade zwischen signierten Versionen. Keine externe
Sicherheitsprüfung. [Konkreter manueller Testplan](MANUAL-TEST.md).
