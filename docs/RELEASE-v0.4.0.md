# Hushlink v0.4.0 – Arbeitsstand, noch nicht veröffentlicht

Diese Version wird derzeit erweitert und ist kein fertiger veröffentlichter Release.

Implementiert im aktuellen Entwicklungsstand: Antworten, gekennzeichnete Bearbeitungen,
Emoji-Reaktionen mit verschlüsselten Empfangsbestätigungen, optionale Schreibanzeige,
lokale Suche und gemerkte Nachrichten, Kontakt-Stummschaltung, Sperre unbekannter
Anfragen sowie Rate und Restzeit bei Dateiübertragungen.

Beide PCs benötigen für die neuen Nachrichten Protokollversion 4. Bestehende
v0.1–v0.3-Tresore werden beim erfolgreichen Speichern auf Profilschema 4 erweitert.
Vor einer Migration eine verschlüsselte Sicherung aufbewahren; ältere Apps können
neu gespeicherte Profile nicht öffnen.

Noch in Umsetzung/Prüfung: Zwischenablage-Bilder, QR/Kamera, Windows Hello,
verschlüsselte automatische Backups mit Wiederherstellung und Speicherverwaltung.
Windows-MSVC-/Installer-/Oberflächenprüfung und Signaturprüfung stehen noch aus.

Es besteht derzeit kein Code-Signing-Zertifikat oder verifizierter Signierdienst.
Die Veröffentlichung einer vertrauenswürdig signierten Version bleibt dadurch blockiert.
Keine Behauptung einer externen Sicherheitsprüfung oder eines Tests mit zwei
physischen Bluetooth-PCs.
