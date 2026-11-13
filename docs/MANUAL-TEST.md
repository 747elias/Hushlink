# Zwei-PC-Abnahme – Windows 11 x64

Status: **nicht durchgeführt in der Linux-Cloud-Umgebung**. GitHub-Windows-Runner besitzen hier keine bestätigte physische Bluetooth-Gegenstelle. Ein In-Memory-Test ist kein Funknachweis. Vor produktiver Nutzung diesen Ablauf auf zwei geeigneten physischen Geräten durchführen und Ergebnisse mit Build-Commit, Windows-Version, Adapter-/Treiber-Version, Bildschirmauflösung und Skalierung festhalten. Nur künstliche Profile und Nachrichten verwenden.

## Vorbereitung

- PC A und PC B mit Windows 11 x64, Bluetooth Classic/RFCOMM, funktionierendem Treiber und normalem Benutzerkonto. Internet/WLAN ausschalten, um den ausschliesslichen Bluetooth-Betrieb sichtbar zu belegen.
- Gleiches Release-EXE auf beide PCs laden, SHA-256 prüfen. ZIP-EXE mit einzelnem Download vergleichen. Start ohne Administratorrechte und ohne installierte Rust-/VC-Redistributable-Werkzeuge auf sauberem System prüfen. SmartScreen-/Unternehmensrichtlinien dokumentieren.
- Einen PC mit 100 %, einen mit 150 % Skalierung verwenden; zusätzlich 125/200 %, 720×540-Fenster, Tastatur-/Tab-Navigation prüfen.

## Grundablauf

1. Erststart: keine Beispielkontakte oder erfolgreiche Verbindung sichtbar. Anzeigenamen „Prüfung Ä“ und „Test B 🦉“, langes Testpasswort plus Bestätigung festlegen. Abweichende/zu kurze Bestätigung abweisen. Verlustwarnung sichtbar.
2. App beenden/neustarten: bestehender Tresor verlangt Passwort. Falsches Passwort ändert keine Datei; korrektes öffnet Profil. Zweite Instanz mit demselben Profil darf nicht gleichzeitig schreiben.
3. Bluetooth deaktivieren, dann aktivieren. Verständliche Meldungen für fehlenden Adapter/Classic-Unterstützung prüfen. Keine fiktiven Geräte zeigen. Auf A Empfang starten, auf B Suche bewusst starten. Beobachten, dass Hushlink beim Stoppen keine eigene SDP-Ankündigung hinterlässt und Windows-Sichtbarkeit nicht heimlich dauerhaft ändert.
4. Gegebenenfalls Windows-Pairing durchführen. B verbindet A über den Hushlink-Dienst. A lehnt Anfrage zunächst ab; B muss einen Unterbruch und keinen Chat sehen. Erneut verbinden, A nimmt an.
5. Vor Bestätigung auf beiden Seiten keinen Text senden können; auch nach nur einseitiger Bestätigung gesperrt. Vollständiger Sicherheitscode stimmt auf beiden Geräten überein. Beide Personen vergleichen unabhängig und bestätigen. Kontakt erst dann gespeichert; Verbindung korrekt angezeigt.
6. Beidseitig „Synthetisch-ÄÖÜ-東京-🦉-2026“ sowie mehrere Zeilen senden. Enter einmal sendet genau eine Nachricht; Shift+Enter fügt Zeilenumbruch ein. Leerer Text wird abgewiesen; 16-KiB-Grenze sinnvoll angezeigt. Statusfolge prüfen: ausstehend → gesendet → empfangen nach Gegenbestätigung. „Empfangen“ ist keine Lesebestätigung.
7. Nachricht senden, Funk/Empfang sofort unterbrechen. Nicht bestätigte Nachrichten müssen fehlgeschlagen werden; keine automatische Doppelübermittlung. Wiederverbinden führt neuen Handshake durch. Identität stimmt mit Kontakt überein; beide Apps dürfen ihre bereits gespeicherten, bestätigten Schlüssel automatisch erneut bestätigen. Beide Richtungen funktionieren wieder.
8. Apps neu starten/entsperren: Kontakt und Verlauf unverändert vorhanden. Dateien unter lokalem App-Datenordner mit Hex-/Textsuche auf eindeutigen Testtext, Anzeigenamen und Bluetooth-Adressen prüfen; keine Klartexttreffer. Keine Chat-Logs/temporären Klartextdateien erzeugt.
9. Manuell sperren (Button und Strg+L), während Nachricht/Discovery läuft: Verbindung endet, sichtbare Inhalte und Eingabe verschwinden. Entsperren baut keine alte Sitzung wieder auf. Keine verspäteten Worker-Ereignisse zeigen alte Inhalte. Passwortdialog-Undo darf alte Passwörter nicht wiederherstellen.
10. Inaktivität auf 1 Minute stellen, ohne Eingabe abwarten. App sperrt und schliesst Sitzung. Windows mit Win+L sperren, entsperren: Hushlink bleibt gesperrt. Auch während Verbindungsaufbau, Handshake, Kontaktprüfung und Passwortarbeit prüfen. Zeitanpassung des Systems darf die monotone Inaktivitätsmessung nicht umgehen.
11. Passwort mit falschem bisherigem Passwort ändern: abweisen. Mit korrektem Passwort ändern: Kontakte und Verlauf bleiben. Nach Neustart altes Passwort abweisen, neues akzeptieren. Keine unverschlüsselte Zwischenkopie.
12. Profil auf B bewusst neu anlegen (Testtresor zuvor bei geschlossener App sichern), gleiche Bluetooth-Adresse. A muss Schlüsselwechsel blockieren. Abbrechen erhält alten Kontakt. Bewusste erneute Prüfung erfordert neuen vollständigen Code auf beiden Seiten; frühere Bestätigung darf nicht gelten.
13. Verlauf auf A löschen und neu starten: lokal entfernt, auf B unverändert. Keine physische SSD-Löschung versprechen.

## Fehler, Abbruch und Dateisicherheit

- Gegenstelle während Passwortänderung, Speicherung, Verbindung oder Empfang schliessen. Keine eingefrorene Oberfläche; Abbruch führt zu verständlichem Zustand. Prozess während Tresorspeicherung mit ausschliesslich Testdaten beenden; danach muss ein vollständig gültiger alter oder neuer Tresor vorliegen, keine automatisch überschriebene beschädigte Datei.
- Geschlossenen künstlichen Tresor kürzen und einzelne Bytes ändern. Öffnen verständlich abweisen, Datei bitgenau erhalten. Vor jedem Fall neue Testkopie verwenden. Vorhandenen defekten Tresor niemals als Erststart überschreiben.
- Schreibgeschützten Datenordner, vollen Datenträger und geöffnete zweite Instanz prüfen: keine falschen Zustellbestätigungen oder stillen Datenverluste. Dateifehler müssen sichtbar werden.
- Nach unerwartetem Adapterverlust erneut verfügbar machen und neu verbinden. Empfang starten/stoppen mehrfach; keine hängenden Dienste/Threads. Zwei gleichzeitige Anfragen sinnvoll begrenzen.
- 500 Kontakte/10.000 Nachrichten/16-MiB-Grenze mit ausschliesslich generierten Testdaten prüfen; Fehler erlauben Löschen alter Verläufe. Langes Gespräch bis Sitzungsgrenze erfordert neue Sitzung.

## Ergebnisprotokoll

| Prüffall | PC/Adapter/Build | Ergebnis | Beobachtung |
|---|---|---|---|
| Start ohne Zusatzinstallation | offen | nicht durchgeführt | |
| Classic-SDP anbieten/finden/verbinden/annehmen | offen | nicht durchgeführt | |
| Beidseitiger Chat ohne IP | offen | nicht durchgeführt | |
| Code-/Identitätsprüfung | offen | nicht durchgeführt | |
| Windows-Sperre und Abbruch | offen | nicht durchgeführt | |
| Neustart/Passwortwechsel/Löschen | offen | nicht durchgeführt | |
| Skalierung/Tastatur/Emojis | offen | nicht durchgeführt | |
