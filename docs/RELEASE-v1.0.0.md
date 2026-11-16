# Hushlink v1.0.0 – geführter Erststart und freie Passwortwahl

- Geführter Erststart in drei Schritten: Willkommen, Profil und Speicherort,
  danach frei wählbares Passwort mit Wiederholung gegen Tippfehler.
- Keine Mindestlänge, Maximallänge oder Zeichenvorgaben für Passwörter in der
  App. Auch leere Passwörter sind möglich. Gilt auch für Passwortwechsel und
  Öffnen vorhandener Tresore. Keine Änderung am verschlüsselten Dateiformat.
- Hinweise statt Stärkeregeln: ohne Passwort kann jede Person mit Zugriff auf
  die Tresordatei deren Inhalte öffnen. Vergessene Passwörter sind nicht
  wiederherstellbar. Lange einzigartige Passphrasen schützen besser.
- Bestehende Profile werden direkt entsperrt; der Assistent läuft nur vor dem
  Anlegen eines neuen Tresors. Zurück verwirft Passwortkopien.
- Such-/Empfangsvorgänge bleiben bewusst ausgelöst, die Kontaktprüfung bleibt
  vor jeder Chatfreigabe erforderlich.

Vom Maintainer als v1.0.0 und ausdrücklich unsignierte Veröffentlichung freigegeben.
Windows SmartScreen kann warnen; Prüfsummen ersetzen keine Herausgebersignatur.
Der Updater behält seine Herausgeberprüfung; diese unsignierte Version manuell
installieren. Vor dem Update den geschlossenen Tresor sichern.

Enthält weiterhin verschlüsselten 1:1-Bluetooth-Chat, Dateien/Fortsetzung,
Kontaktprüfung, Verlauf, Tray/Autostart, QR/Kamera und optionales Windows Hello.
Protokoll 4 und Tresorformat bleiben unverändert.

Die Downloads umfassen den empfohlenen Benutzer-Installer, eine direkt startbare
EXE, ein ZIP mit Begleitdateien und SHA256SUMS.txt. Vor Veröffentlichung laufen
Format, strenges Clippy, sämtliche Tests, Windows-MSVC-Build, nativer Start und
Installer-/Tray-/Autostart-/Update-Übergabe-/Deinstallationstests für exakt den
Release-Commit. Danach werden die öffentlichen Downloads erneut geprüft.

Reale zwei-PC-, Hello-/Kamera-, Win+L- und DPI-Tests bleiben erforderlich. Die
Versionsnummer ist kein Hardware-Nachweis. Keine externe Sicherheitsprüfung.
