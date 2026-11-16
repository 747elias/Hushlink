# v1.1.0 – direkte Gruppenchats

1. Paarweise authentifizierte Zustellung über vorhandene Noise-/RFCOMM-Sitzungen,
   keine eigene Gruppenkryptografie, kein Relay/Server und kein Internet-Fallback.
2. Maximal acht Mitglieder, Besitzer verwaltet die versionierte Mitgliederliste.
   Alle Mitglieder vor Beitritt unabhängig als Kontakte prüfen. Einladungen und
   neue Listen bewusst annehmen; Beitrittszustände werden direkt ausgetauscht.
3. Nachrichten und Zustellwarteschlangen im verschlüsselten Tresor. Pro Empfänger
   echte dauerhafte Bestätigung, Wiederholung nach frischer Sitzung, keine
   Freigabe vor Kontaktprüfung/Beitritt. Empfangsdigests gegen Duplikate behalten.
4. Sichtbar aktivierbare Synchronisierung verbindet vorhandene Mitglieder
   nacheinander; RFCOMM bleibt eine aktive Sitzung. Offline-Mitglieder bleiben
   ausstehend. Pausieren/Sperren beendet automatische Verbindungsversuche.
5. Gruppenliste, Anlegen, Einladen, Mitglieder ändern, Beitreten/Ablehnen,
   Verlassen/Schliessen, Textchat und lokale Verlaufsverwaltung. Dateien bleiben
   zunächst im 1:1-Chat. Entfernen wirkt bei Empfängern nach Empfang der neuen
   Liste; keine Zusage sofortiger globaler Sperrung bei unerreichbaren Geräten.
6. Negative Protokoll-/Gruppen-/Actor-/Tresor-Tests, echte UI-Prüfung mit getrennten
   synthetischen Daten, Format/Clippy/Tests und Windows-/Installer-CI. Erst danach
   Tag v1.1.0, unsignierter Release wie vom Maintainer angefordert, öffentliche
   Downloads/Tag/Hashes/PE prüfen. Reale Tests mit drei Bluetooth-PCs separat.

## Umsetzungsstand

- Schritte 1–5 implementiert und mehrfach nach main gepusht.
- Gruppen-/Protokoll-/Actor-Tests decken drei synthetische Identitäten, echte
  Noise-Handshakes, verlorene Bestätigungen, erneute Sitzungen, Kontakt-/Beitritts-
  sperren, unbekannte/geänderte Identitäten, Listenwechsel/Austritt/Entfernung,
  Parsergrenzen, Manipulation, dauerhafte Duplikatsperren und Speicherfehler ab.
- Gesamte Linux-Suite: 113 Tests bestanden; Format, strenges Clippy, Notice-
  Prüfung und Actionlint bestanden. Ergänzende Replay-Regressionssuite bestanden.
- Laufende Linux-Oberfläche mit getrenntem GRUPPEN-TEST-Profil geprüft: Anlegen,
  Einladung/Beitritt, genau eine mehrzeilige Enter-Sendung, Sperren, Neustart,
  verschlüsselte Persistenz ohne Klartext in Datei/Logs, 1180×812 und 720×540.
- Windows CI prüft für jeden Commit MSVC, alle Tests, nativen Start, PE/ZIP,
  Installer, Tray, Autostart, Update-Prozessübergabe und Deinstallation. Die
  Tag-Pipeline wiederholt dies für den exakten Release-Commit und prüft danach
  öffentliche Downloads. Veröffentlichung erst nach diesen Schranken.
- Physische Drei-PC-Bluetooth-, DPI-, Win+L-, Hello- und Kameratests weiterhin offen;
  kein In-Memory-/UI-Test wird als Funknachweis ausgegeben.
