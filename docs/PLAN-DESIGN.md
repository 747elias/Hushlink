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

Stand: Konzept auf vorhandene egui-Architektur abgebildet, Designsystem in Umsetzung.

Zwischenstand: neues Designsystem, Navigation, Kontaktliste, Chatblasen, Anhänge-/Nachrichtenmenüs, Emoji-Eingabe, neue Authentifizierung und gegliederte Einstellungen implementiert. Lokales Clippy und Build erfolgreich; echte Erststartansicht mit künstlichem Profil sichtbar geprüft. Weitere Layout- und Bedienprüfung läuft.

Live-Prüfung: Erststart, Entsperren, leerer Chat und künstlicher Verlauf geprüft. Überlappende lange Kontaktnamen, zu hohe Nachrichtenaktionen und eine überdehnte erste Nachrichtenblase korrigiert. Reaktionen/Anhänge/Antworten bleiben zugänglich. Responsive Sidebar und scrollbar begrenzte Einstellungen für 720 × 540 werden nachgeprüft. Windows-Zeitdarstellung nutzt die dokumentierten nativen Zeitzonen-APIs; keine neue Laufzeitabhängigkeit.
