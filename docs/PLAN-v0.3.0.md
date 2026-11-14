# Hushlink v0.3.0

1. Bestand prüfen, bestehende Releases und Änderungen bewahren.
2. Verschlüsselte Fortsetzungspunkte und Kontakt-Eigennamen/Favoriten im Tresor ergänzen; sichere Migration und Grenzen prüfen.
3. Dateifortsetzung nach neuem Handshake mit erneuter Annahme und Prüfung des vorhandenen Präfixes implementieren. Kein Fortsetzen alter Sitzungsschlüssel.
4. Private Windows-Benachrichtigungen unabhängig vom sichtbaren Fenster, Drag & Drop mit bewusster Dateiauswahl, Kontaktverwaltung und Verbindungsassistent umsetzen.
5. Negative Tests, Linux-Oberflächenprüfung und native Windows-/Installer-Abnahme; abgeschlossene Schritte committen und pushen.
6. v0.3.0 mit der unveränderten exakten Release-Prüfschranke veröffentlichen und tatsächliche Downloads samt Update-Client prüfen.

Benachrichtigungen enthalten keine Nachrichtentexte, Dateinamen oder Kontaktnamen. Empfangene Dateien werden weiterhin nie automatisch geöffnet. Fortsetzen startet bewusst auf der Senderseite und verlangt eine neue bestätigte Verbindung und erneute Annahme. Bluetooth-Hardwaretests bleiben ohne zwei physische Windows-PCs offen.

## Fortschritt

- Implementiert und gepusht: verschlüsselte Checkpoints/Präfixprüfung, explizite Dateifortsetzung, Kontakte/Favoriten, private native Hinweise, Drag & Drop und Verbindungsassistent.
- Lokal bestanden: 87 vollständige Tests einschließlich der Regression für abgebrochene Gerätesuche, strenges Clippy und Format. Linux-Oberfläche mit künstlichem Testtresor geprüft: Alias/Favorit speichern, Zähler beim Lesen, gespeicherte Teilstände, Assistent und Strg+L.
- Erledigt: vollständiger nativer Windows-Lauf 43627895522 mit 89 Tests und realer Installer-/Tray-/Autostart-/Update-Übergabe-/Deinstallationsabnahme. Vorab-Downloads lokal nochmals auf Hashes, PE und ZIP-Gleichheit geprüft.
- Auslieferung: v0.3.0 wird nach erneut erfolgreichen Prüfungen des genauen Tag-Commits veröffentlicht; danach unabhängige Kontrolle der tatsächlichen öffentlichen Downloads.
- Hardwaregrenze unverändert: keine zwei physischen Windows-11-PCs mit Bluetooth in dieser Umgebung.
