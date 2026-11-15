# Changelog

## 0.4.1 – noch nicht veröffentlicht

- Geführter Erststart mit Willkommen, Profil/Speicherort und freier Passwortwahl.
- Passwortregeln in UI, Neuerstellung, Passwortwechsel und Öffnen entfernt; kurze, leere und lange Unicode-Passwörter werden unterstützt. Verschlüsselung und Wiederholungsprüfung bleiben erhalten.

## 0.4.0 – veröffentlichte unsignierte Vorabversion

- Vollständig überarbeitete Messenger-Oberfläche: Navigation, durchsuchbare Chatliste mit Vorschauen/Filtern, Initialen, kompakte Nachrichtenblasen, Emoji-Eingabe und Aktions-/Anhangmenüs. Neuer Erststart/Entsperren und gegliederte Einstellungen; responsive Spalte und Scrollbereiche für kleine Fenster. Lokale Windows-Uhrzeiten, lokal eingebettete Inter-Schrift und eigene Vektoricons.

- Authentifizierte Antworten, Bearbeitungen und Emoji-Reaktionen mit dauerhaften verschlüsselten Bestätigungen und sicherer Wiederholung nach neuer Verbindung.
- Optionale Schreibanzeige, lokale Verlaufssuche, Pins, Kontakt-Stummschaltung und Sperre unbekannter Anfragen.
- PNG-Zwischenablageversand mit bewusster Freigabe, Rate und Restzeit, vollständiger QR-Sicherheitsvergleich per Kamera.
- Optionaler echter Windows-Hello-Schlüsselplatz, authentifizierte Hülle 2, Passwort-Fallback und Profilschema 4.
- Verschlüsselte Sicherungen mit Aufbewahrung und sicherem Wiederherstellen an einen neuen Pfad; Speicherübersicht und gezieltes Löschen.
- Vorbereitete SignPath-Pipeline für App und Setup, Authenticode-/Timestamp-Prüfung vor Installation/Veröffentlichung und Herausgeberzertifikatsbindung im Updater. Ohne verifizierten Dienst kein signierter Release.

[Prüfstand und offene Hardwaretests](docs/RELEASE-v0.4.0.md), [Signiereinrichtung](docs/SIGNING.md).


## 0.3.0 – Benachrichtigungen und Dateifortsetzung

- Inhaltsfreie Windows-Benachrichtigungen und ungelesene Zähler, unabhängig von egui-Repaints.
- Verschlüsselte Dateicheckpoints, erneutes Anbieten nach Neustart und Fortsetzung in frischer Sitzung nach erneuter Zustimmung; Präfixprüfung und Schutz vor doppelten Dateien nach verlorener Bestätigung.
- Drag & Drop mit begrenzter Vormerkliste, Empfängerbindung und bewusster Freigabe.
- Eigene Kontaktnamen, Favoriten, Geräteübersicht und bewusste lokale Entfernung samt Verlauf.
- Verbindungsassistent mit echten Zuständen und unveränderter Identitätsprüfung.
- Profilschema 3 mit Migration vorhandener Tresore; Protokoll 3 erfordert v0.3 auf beiden PCs.

Prüfstatus und verbleibende Hardwaretests: [Release-Hinweise](docs/RELEASE-v0.3.0.md) und [Prüfstand](docs/VALIDATION.md).

## 0.2.0 – Dateien und Windows-Installation

- Authentifizierter Bluetooth-Dateiversand bis 20 MiB mit ausdrücklicher Annahme, Fortschritt, Abbruch und bestätigter verschlüsselter Speicherung; bewusster Export statt automatischem Öffnen.
- Alle privaten Daten einschließlich Anhängen in einem verschlüsselten Tresor; Speicherortwahl, sichere Verlagerung und Migration von v0.1-Profilen.
- Per-user Windows-Installer, Startmenü und Deinstallation mit Erhalt der Tresore; Systemtray, Einzelinstanz und wählbarer gesperrter Autostart.
- Abschaltbare automatische GitHub-Update-Prüfung und geprüfte Downloads; Installation und Neustart nach bewusster Auswahl, mit sicherer Prozessübergabe.
- Protokollversion 2, erweiterte negative Tests und verpflichtende native Windows-Installer-/Update-Prüfungen vor Veröffentlichung.

Prüfstatus und verbleibende Hardwaretests: [Release-Hinweise](docs/RELEASE-v0.2.0.md), [Prüfstand](docs/VALIDATION.md). Keine externe Sicherheitsprüfung oder Authenticode-Signatur. Beide Chatpartner benötigen v0.2. Vor der Tresormigration eine geschlossene Sicherung behalten.

## 0.1.0 – erste Vorabversion

- Deutsche Rust-Desktopoberfläche für lokalen 1:1-Textchat mit Bluetooth Classic/RFCOMM auf Windows 11 x64.
- Eigener SDP-Dienst, bewusste Gerätesuche und Empfang, eingehende Anfragen und Wiederverbindung.
- Ende-zu-Ende-verschlüsselte Sitzungen, vollständiger Sicherheitscode, beidseitige Bestätigung und Sperre bei bekanntem Identitätswechsel.
- Passwortgeschützter authentifiziert verschlüsselter lokaler Tresor mit Kontakten, Verlauf, atomarem Speichern und Passwortwechsel.
- Unicode, mehrzeilige Nachrichten und authentifizierte Empfangsbestätigungen; manuelle, Inaktivitäts- und Windows-Sitzungssperre.
- Gated GitHub-Actions-Prüfungen und native Windows-MSVC-Auslieferung als EXE und ZIP mit SHA-256-Werten.

Prüfstatus: Automatische Ergebnisse sind im zugehörigen Release-Actions-Lauf nachprüfbar. Physische Zwei-PC-Bluetooth-Tests und eine interaktive Windows-UI-Abnahme wurden in der Linux-Cloud-Umgebung nicht durchgeführt. Keine externe Sicherheitsprüfung, kein Authenticode-Herausgeberzertifikat. Siehe SECURITY.md und docs/MANUAL-TEST.md.
