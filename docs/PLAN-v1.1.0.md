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
