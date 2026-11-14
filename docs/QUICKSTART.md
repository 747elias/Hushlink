# Hushlink – Kurzstart für Windows 11 x64

1. **Hushlink-v0.3.0-windows-x64-setup.exe** herunterladen und im eigenen Benutzerkonto installieren. Keine Administratorrechte nötig. Alternativ die separate EXE direkt starten oder die ZIP vollständig entpacken. Der Tresor liegt zunächst im lokalen Windows-App-Datenordner, nicht neben der EXE; in Einstellungen lässt sich sein Speicherort wählen.
2. Falls Windows SmartScreen warnt: Diese erste Version ist nicht mit einem vertrauenswürdigen Herausgeberzertifikat signiert. Quelle und SHA-256-Prüfsumme vor dem Start prüfen. Keine Sicherheitsfunktion pauschal deaktivieren.
3. Auf beiden PCs Bluetooth einschalten. Die Adapter müssen Bluetooth Classic/RFCOMM unterstützen; reine BLE-Adapter reichen nicht. Wenn nötig die PCs über die Windows-Bluetooth-Einstellungen koppeln.
4. Beim Erststart Anzeigename und ein starkes, einzigartiges Passwort festlegen. Ein vergessenes Passwort kann nicht zurückgesetzt werden; ohne das Passwort ist der lokale Verlauf verloren.
5. Auf einem PC bewusst den Hushlink-Dienst anbieten. Auf dem anderen PC Geräte suchen und eine Verbindung anfordern. Die eingehende Anfrage auf dem ersten PC annehmen.
6. Den vollständigen Sicherheitsfingerabdruck auf beiden PCs über einen unabhängigen, vertrauenswürdigen Weg vergleichen, zum Beispiel direkt nebeneinander. Beide Personen müssen ihn auf ihrem Gerät bestätigen. Ein Bluetooth-Gerätename beweist keine Identität.
7. Danach schreiben: Enter sendet, Umschalt+Enter fügt einen Zeilenumbruch ein. „Empfangen“ bedeutet, dass die Gegenstelle eine geschützte Bestätigung geschickt hat.
8. Vor dem Verlassen des PCs die App sperren. Die Inaktivitätssperre lässt sich in der App einstellen. Sperren beendet laufende Verbindungen.
9. **Datei senden**: Datei bis 20 MiB auswählen. Die andere Person muss annehmen. Unter **Dateiübertragungen** Fortschritt sehen oder abbrechen. Anhänge bleiben im verschlüsselten Tresor; **Speichern unter** erzeugt eine unverschlüsselte Kopie. Keine automatische Ausführung.
10. **Einstellungen**: vorhandenen Tresor wählen oder Speicherort ändern, Windows-Autostart, Schliessen ins Tray und automatische Updates einstellen. Schliessen ins Tray sperrt zuerst. Zum vollständigen Beenden das Tray-Menü nutzen. Nur Updates benötigen Internet; Installation erst nach **Update installieren und neu starten**.

## Prüfsummen

Prüfsummen aus derselben Version herunterladen. Im Download-Ordner:

```powershell
Get-FileHash .\Hushlink-v0.3.0-windows-x64-setup.exe -Algorithm SHA256
Get-FileHash .\Hushlink-v0.3.0-windows-x64.exe -Algorithm SHA256
Get-FileHash .\Hushlink-v0.3.0-windows-x64.zip -Algorithm SHA256
```

Die Werte mit `SHA256SUMS.txt` vergleichen. Prüfsummen erkennen eine beschädigte oder abweichende Datei; sie ersetzen keine unabhängige Prüfung der Downloadquelle.

## Grenzen der ersten Version

- Internet, Konten und ein zentraler Server sind für den Chat nicht erforderlich.
- Bluetooth-Sichtbarkeit und Verkehrsmuster können beobachtet werden; die App verspricht keine Anonymität.
- Ein kompromittierter oder entsperrter PC kann Inhalte preisgeben. Die App wurde nicht extern sicherheitsgeprüft.
- Echtes Bluetooth-Verhalten hängt von Windows, Treibern und Adaptern ab. Der genaue automatische und manuelle Teststatus steht in den Release-Hinweisen.
- Lokales Löschen eines Verlaufs verspricht keine sichere physische Löschung auf SSDs.

Bei Problemen README und SECURITY im ZIP beziehungsweise im Repository lesen.

Vor dem Update die geschlossene Tresordatei sichern. v0.3 migriert v0.1-/v0.2-Profile beim Speichern; danach die neue App verwenden. Beide PCs brauchen dasselbe neue Protokoll; bereits bestätigte Identitäten bleiben gespeichert. Deinstallation bewahrt Tresor und Einstellungen.

## Neu in v0.3

- Verbindungsassistent für Adapter, Pairing, Empfang/Suche und Sicherheitscode.
- Dateien per Drag & Drop ablegen, Empfänger prüfen und jede Datei bewusst anbieten.
- Nach Unterbruch neu verbinden und im ausgehenden Verlauf „Erneut anbieten / fortsetzen“ wählen; die andere Person bestätigt erneut. Bereits im Tresor gesicherte Teile werden geprüft und weiterverwendet.
- Unter „Kontakt verwalten“ eigenen Namen, Favoriten und Geräteübersicht nutzen.
- Private Benachrichtigungen enthalten keinen Inhalt. Zum Empfang im Hintergrund die entsperrte App minimieren; Schliessen ins Tray sperrt weiterhin und beendet Sitzungen.


Im Entwicklungsstand v0.4 kommen Antworten, Reaktionen, Bearbeitungen, Suche/Pins,
optionale Schreibanzeige, Zwischenablage-Bilder, QR-Kamera, optionales Windows Hello,
verschlüsselte Sicherungen und Speicherübersicht hinzu. Beide PCs benötigen v0.4.
Vor der Migration eine verschlüsselte Kopie behalten. Neue Bedienung und Grenzen
stehen in README.md; Windows-Hello- und Signierstatus sind in docs dokumentiert.
Ein v0.4-Release ist vor Einrichtung und Prüfung des echten Signierdienstes gesperrt.
