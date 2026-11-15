# Hushlink v0.4.1 – Onboarding (noch nicht veröffentlicht)

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

Die CI erzeugt Windows-Testbuilds; dies ist kein veröffentlichter Release. Die
Ausnahme für den unsignierten v0.4.0-Release gilt weiterhin nur für v0.4.0.
Native Windows-Start-/Installer-Tests und lokale automatische Tests sind keine
Bluetooth-Hardwareabnahme. Reale zwei-PC-, Hello-/Kamera-, Win+L- und DPI-Tests
bleiben erforderlich. Keine externe Sicherheitsprüfung.
