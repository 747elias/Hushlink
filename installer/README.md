# Windows-Installation

`scripts/package.ps1` baut mit **Inno Setup 7.1.1 x64** einen Installer für Windows 11 x64. `installer/install-compiler.ps1` lädt ausschließlich den festgelegten [offiziellen Release](https://github.com/jrsoftware/issrc/releases/tag/is-7_1_1), prüft vor der Ausführung SHA-256 `8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0` und prüft die Compiler-Version. Der Hash stammt vom veröffentlichten GitHub-Asset-Digest (geprüft am 14.11.2026). Inno Setup wird nur zum Bauen benötigt; dessen [Lizenz](https://github.com/jrsoftware/issrc/blob/is-7_1_1/license.txt) bleibt im unveränderten Installationsprogramm erhalten.

Die dauerhafte AppId lautet `{36F98DE2-AD17-432E-AE2D-759CA74B8574}`. Die Installation erfolgt ohne Administratorrechte unter `%LOCALAPPDATA%\Programs\Hushlink`. Startmenü, Deinstallation und ein optionaler Desktop-Link sind enthalten. Auf Erstinstallation ist der Start mit Windows vorausgewählt. Der Wert `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Hushlink` enthält `"<Installationspfad>\Hushlink.exe" --tray`; spätere Updates respektieren den aktuell aktivierten oder deaktivierten Autostart. Der Start im Infobereich entsperrt keinen Tresor.

Für ein bereits geprüftes Update verwendet die App:

```text
Hushlink-v0.2.0-windows-x64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RESTARTAPPLICATIONS
```

Die App sperrt den Tresor und beendet sich vor dem Austausch. Nach erfolgreicher stiller Installation öffnet der Installer Hushlink mit `--tray`. `/NoLaunch` unterdrückt diesen letzten Schritt bei automatisierten Verpackungsprüfungen. Restart Manager übernimmt kooperatives Schließen; ein erzwungenes Beenden ist nicht konfiguriert. Profile gehören nicht zum installierten Payload. Es gibt keine pauschale Löschung von Verzeichnissen: Deinstallation lässt selbst gewählte Tresore und die lokalen App-Einstellungen bestehen.

Das Release enthält die direkt startbare EXE sowie den Installer und ein ZIP mit beiden Varianten, Anleitungen und Lizenzhinweisen. Es benötigt keine zusätzliche .NET-, MSIX-, App-SDK- oder VC++-Installation. Es besitzt keine behauptete Authenticode-Signatur; Windows kann deshalb SmartScreen-Hinweise anzeigen.

Der verpflichtende Windows-Check installiert den tatsächlich gebauten Installer still, prüft Binäridentität, Registrierung und Startmenü, startet die installierte App sichtbar und per `--tray`, überprüft den Erhalt der Autostart-Auswahl bei erneutem Installieren sowie einen stillen Neustart und deinstalliert anschließend. Künstliche Dateien innerhalb und außerhalb des Installationsverzeichnisses müssen dabei erhalten bleiben. Das ist kein physischer Bluetooth-Test.

Offizielle Grundlagen: [Nicht administrative Installation](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm), [Setup-Architektur](https://jrsoftware.org/ishelp/topic_setup_setuparchitecture.htm), [Restart Manager](https://jrsoftware.org/ishelp/topic_setup_closeapplications.htm), [Datei-/Registry-Deinstallation](https://jrsoftware.org/ishelp/topic_registrysection.htm), [Installationsparameter](https://jrsoftware.org/ishelp/topic_setupcmdline.htm).
