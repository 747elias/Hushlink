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

Prüfstatus: für `7336f17` 97 Linux- und 100 Windows-Tests sowie Format, Clippy,
MSVC-Release-Build, echter EXE-Start, Installer/Tray/Autostart/Update-Übergabe und
Deinstallation erfolgreich. Heruntergeladene CI-EXE/Setup/ZIP lokal auf PE,
Prüfsummen und Commit geprüft. Weitere Regressionen und verzögert geladene
optionale Kamera-DLLs sind im nachfolgenden Prüflauf enthalten; massgeblich ist
[VALIDATION.md](VALIDATION.md) mit dem aktuellen Branch-CI-Lauf.

**Kein veröffentlichter v0.4-Release und keine Herausgebersignatur.** Der Nutzer
hat noch kein Code-Signing-Zertifikat oder verifizierten Signierdienst.
[SIGNING.md](SIGNING.md) enthält den konkreten eingerichteten Ablauf und die
noch nötige externe Konto-/Identitätsfreigabe. Unsigned CI-Artefakte werden
nicht als signierter Release ausgegeben. Es wurde kein Tag erstellt.

Noch nicht durchgeführt: zwei physische Bluetooth-PCs, reale Kamera, Hello-
Biometrie/PIN samt Providerunterstützung, vollständige Windows-UI-/Skalierungs-
und Win+L-Abnahme sowie Upgrade zwischen signierten Versionen. Keine externe
Sicherheitsprüfung. [Konkreter manueller Testplan](MANUAL-TEST.md).
