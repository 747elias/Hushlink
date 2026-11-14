# Hushlink v0.4.0

1. Bestand/AGENTS, Signierkonfiguration und offizielle Windows-APIs prüfen. Kein Vertrauen durch selbst ausgestellte Zertifikate vortäuschen.
2. Tresor/Protokoll für Antworten, Reaktionen, Bearbeitungen, Schreibanzeige, Pins und lokale Suche erweitern; Grenzen und negative Tests.
3. Zwischenablage-Bilder als ausdrücklich bestätigte Anhänge, Übertragungsrate/Restzeit, Kontakt-Stummschaltung, Speicherübersicht und Anfragebegrenzung integrieren.
4. Vollständigen Sicherheitscode als QR vergleichen und per Kamera prüfen; Windows Hello nur mit echter Bindung an einen vom Windows-Hello-Provider geschützten nicht exportierbaren Schlüssel. Passwort bleibt Rückweg.
5. Automatische verschlüsselte Backups mit begrenzter Aufbewahrung und sicherem Wiederherstellungsablauf. Keine Klartext-Exporte oder Sicherungen von Schlüsseln.
6. Format, Clippy, Tests, tatsächliche Windows-MSVC-/Installer-Prüfungen, Oberfläche, regelmässige Commits/Pushes.
7. Signieren vor der Verpackung/Prüfung; Authenticode und Timestamp prüfen. Eine signierte Veröffentlichung braucht eine gültige autorisierte Herausgeberidentität. Ohne sie keinen unsignierten Release als fertige Erfüllung veröffentlichen.

Signierstatus zu Beginn: lokal keine passende Konfiguration gefunden. Die GitHub-Integration darf Actions-Secrets/Variablen nicht auflisten (403); ein dort vorhandenes Zertifikat kann daher nicht bestätigt werden. Nutzer nach vorhandenem Signierdienst/Zertifikat gefragt. Feature-Arbeit geht unabhängig weiter.

Zwischenstand: Nutzer bestätigt, noch kein Zertifikat oder Signierdienst. Antworten, Bearbeitungen, Reaktionen, optionale Schreibanzeige, Suche, Pins, Kontakt-Stummschaltung, Anfragebegrenzung und Rate/Restzeit implementiert; strenger lokaler Clippy erfolgreich, erweiterte Tests laufen. Windows-/UI-Nachweis für v0.4 noch ausstehend.

Zweiter Schritt: verschlüsselte Sicherungen mit Aufbewahrung/Wiederherstellung, Speicherübersicht/gezieltes Löschen, bestätigter Bildversand aus Zwischenablage und vollständiger QR-Vergleich implementiert. Backup-/PNG-/QR-Roundtrip und korrigierter Migrationstest lokal erfolgreich; Kamera-Hardware noch ungetestet.

Dritter Schritt: kryptografisch gebundener Hello-Zugang und authentifizierte Hülle 2 implementiert; Passwort-/Manipulations-/Schlüsselplatztests lokal erfolgreich. SignPath-Ablauf mit zwei Signierstufen, Windows-Verifikation, Zertifikatsbindung im Updater und Veröffentlichungs-Schranke vorbereitet. Reale Hello-/Kamera-/Bluetooth-Hardware und autorisierte Signierkonfiguration fehlen weiterhin.

Abnahme-Zwischenstand: Windows-Lauf 43634633664 für 7336f17 vollständig grün
(100 Tests, MSVC-EXE und sämtliche Start-/Installer-Smokes); Artefakte tatsächlich
heruntergeladen und auf Commit/SHA/PE/ZIP geprüft. Linux 97 vollständige Tests grün.
Zusätzliche Rückfalltests, Native-Lock-Medienabbruch ohne UI-Frames, Kill-on-close-
Kamerajob und verzögert geladene optionale Kamera-DLLs werden im letzten Lauf geprüft.
Signatur/Veröffentlichung bleibt ausdrücklich wegen fehlendem externem Dienst offen.

Abnahme fortgeschritten: 3df3d1b besteht mit 101 Linux- und 104 Windows-Tests;
MSVC, optionale Kamera-Importtabellen und alle Installer-Smokes erfolgreich
(43636751840). Echter Linux-UI-Test belegt Suche/Pins/Antwortverweise/Mute,
Speicheransicht, Ciphertext-Backup, automatische und manuelle Sperre; keine
Klartextmarker in Profil/Backup/Logs. Zusätzlicher Restore-Akteurtest erfolgreich.
Letzter Schutz verhindert Enter-Entsperren hinter dem Restore-/Einstellungsdialog.

Abschlusskorrektur: Backup-Aufbewahrung schützt die soeben erzeugte Kopie auch bei rückgestellter Uhr (mit zuvor fehlgeschlagener Regression). CNG akzeptiert gemäss offizieller API ausschliesslich ERROR_SUCCESS (0). Erneuter gesamter Prüflauf erforderlich.
