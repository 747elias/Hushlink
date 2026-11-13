# Hushlink – Kurzstart für Windows 11 x64

1. Die EXE herunterladen und starten. Alternativ die ZIP vollständig entpacken und die enthaltene EXE starten. Die App benötigt keine Installation und keine Administratorrechte. Profile liegen im lokalen Windows-App-Datenordner, nicht neben der EXE.
2. Falls Windows SmartScreen warnt: Diese erste Version ist nicht mit einem vertrauenswürdigen Herausgeberzertifikat signiert. Quelle und SHA-256-Prüfsumme vor dem Start prüfen. Keine Sicherheitsfunktion pauschal deaktivieren.
3. Auf beiden PCs Bluetooth einschalten. Die Adapter müssen Bluetooth Classic/RFCOMM unterstützen; reine BLE-Adapter reichen nicht. Wenn nötig die PCs über die Windows-Bluetooth-Einstellungen koppeln.
4. Beim Erststart Anzeigename und ein starkes, einzigartiges Passwort festlegen. Ein vergessenes Passwort kann nicht zurückgesetzt werden; ohne das Passwort ist der lokale Verlauf verloren.
5. Auf einem PC bewusst den Hushlink-Dienst anbieten. Auf dem anderen PC Geräte suchen und eine Verbindung anfordern. Die eingehende Anfrage auf dem ersten PC annehmen.
6. Den vollständigen Sicherheitsfingerabdruck auf beiden PCs über einen unabhängigen, vertrauenswürdigen Weg vergleichen, zum Beispiel direkt nebeneinander. Beide Personen müssen ihn auf ihrem Gerät bestätigen. Ein Bluetooth-Gerätename beweist keine Identität.
7. Danach schreiben: Enter sendet, Umschalt+Enter fügt einen Zeilenumbruch ein. „Empfangen“ bedeutet, dass die Gegenstelle eine geschützte Bestätigung geschickt hat.
8. Vor dem Verlassen des PCs die App sperren. Die Inaktivitätssperre lässt sich in der App einstellen. Sperren beendet laufende Verbindungen.

## Prüfsummen

Alle drei Release-Dateien aus derselben Version herunterladen. Im Download-Ordner:

```powershell
Get-FileHash .\Hushlink-v0.1.0-windows-x64.exe -Algorithm SHA256
Get-FileHash .\Hushlink-v0.1.0-windows-x64.zip -Algorithm SHA256
```

Die Werte mit `SHA256SUMS.txt` vergleichen. Prüfsummen erkennen eine beschädigte oder abweichende Datei; sie ersetzen keine unabhängige Prüfung der Downloadquelle.

## Grenzen der ersten Version

- Internet, Konten und ein zentraler Server sind für den Chat nicht erforderlich.
- Bluetooth-Sichtbarkeit und Verkehrsmuster können beobachtet werden; die App verspricht keine Anonymität.
- Ein kompromittierter oder entsperrter PC kann Inhalte preisgeben. Die App wurde nicht extern sicherheitsgeprüft.
- Echtes Bluetooth-Verhalten hängt von Windows, Treibern und Adaptern ab. Der genaue automatische und manuelle Teststatus steht in den Release-Hinweisen.
- Lokales Löschen eines Verlaufs verspricht keine sichere physische Löschung auf SSDs.

Bei Problemen README und SECURITY im ZIP beziehungsweise im Repository lesen.
