# Hushlink v1.0.0 – Kurzstart für Windows 11 x64

1. **Hushlink-v1.0.0-windows-x64-setup.exe** im eigenen Benutzerkonto installieren.
   Alternativ die EXE direkt starten oder das ZIP vollständig entpacken.
2. Der Assistent führt durch Willkommen, Anzeigename/Speicherort und Passwort.
   Keine Längen- oder Zeichenvorgaben, auch leer erlaubt. Ohne Passwort kann
   jede Person mit der Tresordatei deren Inhalte öffnen. Keine Wiederherstellung
   vergessener Passwörter; ein starkes Passwort schützt besser.
3. Auf beiden PCs Bluetooth einschalten, bei Bedarf in Windows koppeln. Über
   das Plus neben Chats auf einem PC Empfang anbieten, auf dem anderen Geräte
   suchen und verbinden. Die Gegenstelle muss annehmen.
4. Den vollständigen Sicherheitscode unabhängig vergleichen und auf beiden PCs
   bestätigen. Ein Gerätename genügt nicht. Erst dann wird der Chat freigegeben.
5. Enter sendet, Shift+Enter fügt einen Zeilenumbruch ein. Die Büroklammer bietet
   Dateien/Bilder an. Empfang erst nach Zustimmung; Anhänge bleiben im Tresor.
   Download am Anhang erzeugt bewusst eine unverschlüsselte Kopie.
6. Schloss oder Strg+L sperrt und trennt. Einstellungen enthalten Passwortwechsel,
   Speicherort, Sicherungen, Autostart und Tray. Schliessen ins Tray sperrt.
7. Vor Updates die geschlossene Tresordatei sichern. Die Deinstallation erhält
   eigene Tresore. Ein Hintergrundempfang benötigt eine entsperrte App.

## Prüfsummen und Signatur

`Get-FileHash .\Hushlink-v1.0.0-windows-x64-setup.exe -Algorithm SHA256`
mit SHA256SUMS.txt vergleichen. Das gilt entsprechend für EXE und ZIP.

**Nicht signiert:** Windows SmartScreen kann warnen. Prüfsummen ersetzen keine
Herausgebersignatur. Die Signaturprüfung im Updater bleibt aktiv, diese Version
manuell installieren. Kein zusätzliches Rust-/VC++-Paket oder MSIX nötig.

## Grenzen

Chat und Dateiversand benötigen kein Internet/Konto/Server. Nur Updates nutzen
Internet. Bluetooth-Metadaten bleiben sichtbar, keine Anonymitätszusage.
Entsperrte oder kompromittierte Geräte können Daten preisgeben. Keine externe
Sicherheitsprüfung. Reale Bluetooth-/Kamera-/Hello-/Win+L-/DPI-Tests stehen aus.
Löschen eines Verlaufs verspricht keine sichere physische SSD-Löschung.
