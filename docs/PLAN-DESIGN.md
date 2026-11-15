# Messenger-Redesign

1. Sauberen Bestand prüfen, Aufgabenbranch `feature/messenger-redesign` verwenden.
2. Eigenständiges dunkles Messenger-Design im Stil moderner WhatsApp-Desktop-Layouts:
   schmale Navigation, durchsuchbare Kontaktliste, Initialen, letzte Nachrichten,
   klare Chatkopfzeile, kompakte Blasen und abgesetzte Eingabe. Einheitliche lokale
   Vektoricons und eingebettete Inter-Schrift; keine nachgeladenen UI-Assets.
3. Erststart, Entsperren, Sicherheitsprüfung, Verbindung und Einstellungen neu gestalten.
   Echte Zustände verwenden; keine erfundenen Kontakte, Online- oder Sicherheitsanzeigen.
4. Bedienung/Lesbarkeit bei kleinen Fenstern prüfen. Neue UI-Suchpuffer bei Sperre
   löschen; Enter/Shift+Enter, Bestätigungen und sämtliche bisherigen Funktionen erhalten.
5. Format, Clippy, vollständige Tests und native Windows-/Installer-CI. Laufende
   Oberfläche mit ausdrücklich getrennten künstlichen Daten ansehen und korrigieren.
6. Sinnvolle Schritte committen/pushen und geprüften Windows-Testbuild bereitstellen.
   Die bekannte fehlende autorisierte Signiermöglichkeit wird durch UI-Arbeit nicht ersetzt.

Stand: Design und responsive Ansichten implementiert. Zwei abgeschlossene Schritte
liegen auf GitHub; abschliessende Prüfung und Windows-Artefaktkontrolle folgen.

Zwischenstand: neues Designsystem, Navigation, Kontaktliste, Chatblasen, Anhänge-/Nachrichtenmenüs, Emoji-Eingabe, neue Authentifizierung und gegliederte Einstellungen implementiert. Lokales Clippy und Build erfolgreich; echte Erststartansicht mit künstlichem Profil sichtbar geprüft. Weitere Layout- und Bedienprüfung läuft.

Live-Prüfung: Erststart, Entsperren, leerer Chat und künstlicher Verlauf geprüft. Überlappende lange Kontaktnamen, zu hohe Nachrichtenaktionen und eine überdehnte erste Nachrichtenblase korrigiert. Reaktionen/Anhänge/Antworten bleiben zugänglich. Responsive Sidebar und scrollbar begrenzte Einstellungen für 720 × 540 werden nachgeprüft. Windows-Zeitdarstellung nutzt die dokumentierten nativen Zeitzonen-APIs; keine neue Laufzeitabhängigkeit.

Abschliessende lokale Prüfung: 102 Tests erfolgreich, strenges Clippy und Build
erfolgreich. Echte Live-Prüfung bei 720 × 540 und 1180 × 812: Erststart/Entsperren,
Chatliste, lange Namen, mehrzeiliger künstlicher Verlauf, Antworten, Pins,
Reaktionen, Anhang, Suche mit Treffer/Leerzustand, Einstellungen und Strg+L.
Überdehnte Blasen, überlappende Namen und abgeschnittene kleine Ansichten
behoben. Ein Scan des gesperrten künstlichen Tresors samt Logdateien findet
keinen der eindeutigen Testklartexte/Passwörter. Testdaten und Testtresore werden
nicht ausgeliefert; nur eine ausdrücklich beschriftete Live-Abbildung wird
dokumentiert. Tastaturfokus für eigene Iconknöpfe ist sichtbar.

Der [CI-Lauf dieses Branches](https://github.com/747elias/Hushlink/actions/workflows/ci.yml?query=branch%3Afeature%2Fmessenger-redesign)
führt Format, strenges Windows-Clippy, vollständige Tests, MSVC-Release-Build,
EXE-Start, PE-/Paketprüfung sowie Installer/Tray/Autostart/Update-Übergabe und
Deinstallation aus. Sein exakter Commit und seine Download-Artefakte sind der
Nachweis für den jeweiligen Windows-Testbuild; kein Tag und kein Release werden
durch dieses Design erzeugt. Die Veröffentlichung bleibt mangels autorisiertem
Signierdienst/Zertifikat blockiert.

Noch manuell erforderlich: Windows-DPI/Monitorwechsel, lokale Zeitzonen und
Emoji-Darstellung, echte Dialoge beider Bluetooth-PCs, Kamera, Hello und Win+L
mit geeigneter Hardware. Automatische Memory-Transport- und Windows-Starttests
werden nicht als Hardwareabnahme ausgegeben.

Offizielle Referenzen für die Windows-Zeitdarstellung (abgerufen 15.11.2026):
[FileTimeToSystemTime](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-filetimetosystemtime),
[SystemTimeToTzSpecificLocalTimeEx](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-systemtimetotzspecificlocaltimeex).
Die Konvertierung nutzt die eingestellte dynamische Windows-Zeitzone, prüft beide
API-Ergebnisse und überlaufsichere FILETIME-Umrechnung. Linux zeigt bewusst UTC.
