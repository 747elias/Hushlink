# Interne Sicherheits- und Integrationsprüfung, v0.1.0

Stand: 13. November 2026. Dies ist eine entwicklungsbegleitende Codeprüfung
innerhalb dieser Codex-Aufgabe. Sie ist **keine externe Sicherheitsprüfung**,
keine Zertifizierung und kein Nachweis eines Bluetooth-Hardwaretests.

## Geprüfter Umfang

- Trennung von nativen RFCOMM-Sockets, Noise-Sitzung, Tresor, Hintergrundarbeitern
  und UI; kein Internet-/TCP-Ersatztransport in der ausgelieferten App.
- Noise XX mit frischen Handshakes, getrennten Transportzuständen und vollständigem,
  kontextgebundenem SHA-256-Fingerabdruck über kanonisch geordnete Identitäten.
- Beidseitige authentifizierte Bestätigung vor Chat; Schlüsselwechsel schliesst
  die Sitzung. Erneutes Prüfen erfordert einen bewussten Schritt und einen neuen
  Handshake. Gespeicherte menschliche Bestätigung darf bei unveränderter Identität
  für die erneute lokale Sitzungsbestätigung verwendet werden.
- Grenzen der Längenrahmen, zulässige Zustände und Steuerpakete, Wiederholungen,
  Empfangsbestätigungen und Abbruch bei Authentifizierungsfehlern.
- Zufälliger unabhängiger Tresorschlüssel, Argon2id-Passwortableitung, getrenntes
  XChaCha20-Poly1305-Key-Wrapping und Inhaltsverschlüsselung. Der vollständige
  Header ist über die Inhalts-AEAD authentifiziert; das Wrapping authentifiziert
  seinen stabilen Parameterpräfix. Speichern verwendet frische Inhaltsnonces.
- Dateisperre, beschränkte Dateigrösse, atomarer verschlüsselter Austausch und
  Erkennung externer Änderungen vor dem Ersetzen. Keine Klartext-Backups.
- Lock-Generationen, Abbruch offener Sockets, begrenzte Warteschlangen,
  veraltete Ergebnisse und UI-Passwort-/TextEdit-Kopien.

## Während der Prüfung korrigierte Punkte

| Befund | Korrektur |
| --- | --- |
| Unterschiedliche Grenzen für Profilnamen und verschlüsseltes Hello | Prüfung gegen dieselbe Grenze von 80 UTF-8-Bytes; keine Steuerzeichen |
| `anyhow` und `rand` hatten bekannte Unsoundness-Hinweise | Auf kompatible korrigierte Releases 1.0.104 und 0.8.8 fixiert; Lockfiles aktualisiert |
| Argon2s gewöhnliche Scratch-Allokation wird nicht automatisch gelöscht | `zeroize` aktiviert; Scratch-Blöcke und abgeleiteter Schlüssel in `Zeroizing` |
| UI-Zeichenpuffer allein erfasst TextEdit-Undo-Speicher nicht | UI-Memory beim Sperren und nach Passwortübermittlung zurückgesetzt |
| Ausgehende Socket-Schreiboperation konnte der Tresorpersistierung vorauslaufen | Arbeiter wartet auf erfolgreiche dauerhafte Speicherung der ausstehenden Nachricht |
| Verspätete Ergebnisse von Empfang/Verbindungsversuchen konnten neue UI-Zustände verändern | Vorgangsbezogene Abbruchmarken zusätzlich zur Lock-Generation |
| Volle UI-Ereigniswarteschlange konnte den letzten Zustand verlieren | Letzter Zustand und Busy-Wert in einer zusammengefassten Mailbox |
| Langlebige Abbruch-Handles konnten alte Sockets festhalten | Besitzgebundene Socket-Leases entfernen abgeschlossene Handles |
| Unbekannter Windows-Sitzungsflag konnte als entsperrt gelten | Nur der ausdrückliche UNLOCK-Zustand gilt als entsperrt |

Die Tests und der allgemeine Build-Status stehen in `../README.md`,
`MANUAL-TEST.md` und den Release-Hinweisen. Automatische Protokolltests verwenden
synthetische Identitäten und einen nur für Tests übersetzten In-Memory-Transport.
Sie sind kein Ersatz für zwei physische Windows-11-PCs.

## Abhängigkeitsprüfung

`cargo-audit 0.23.0` prüfte das aktualisierte Haupt-Lockfile mit 356 Einträgen
gegen RustSec-Datenbank-Commit
`4ef2df8e806a65e2f1ecc14f56f51249062b23d6`
(aktualisiert am 13. November 2026). Ergebnis: **keine gemeldeten
Sicherheitslücken und keine verbleibenden Unsoundness-Hinweise**.

Ein Wartungshinweis bleibt: `ttf-parser 0.25.1`,
[RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html), ist
nicht mehr gepflegt und wird transitiv durch eguis Schriftverarbeitung verwendet.
Es gibt dafür keine korrigierte Veröffentlichung. Hushlink importiert keine
von Chatpartnern gelieferten Schriftdateien; verwendet werden mitgelieferte und
lokale Windows-Schriften. Der Hinweis bleibt eine zu verfolgende
Abhängigkeitsgrenze und bedeutet nicht, dass alle möglichen Schwachstellen
bekannt wären.

## Verbleibende Grenzen und notwendige manuelle Nachweise

- Windows-Hosted-Runner und Linux-UI-Prüfungen belegen keine echte Bluetooth-
  Dienstveröffentlichung, Funkreichweite, Windows-Pairing oder Verbindung zwischen
  zwei physischen Geräten. Der Ablauf in `MANUAL-TEST.md` muss durchgeführt werden.
- Speicherlöschung ist best effort. Betriebssystem, GPU, Auslagerung, Crash-Dumps,
  primitive/interne temporäre Bibliothekskopien und kompromittierte Geräte sind
  nicht vollständig kontrollierbar. `snow`-Adapter überschreiben die zugänglichen
  DH-/Cipher-Schlüsselslots beim Freigeben, führen aber keine eigene Kryptografie aus.
- Offline-Angriffe auf schwache Passwörter, Rückspielen älterer gültiger
  Tresordateien und Verfügbarkeit gegen aktive lokale Angreifer sind keine
  ausgeschlossenen Risiken. Formatvalidierung ersetzt keine starke Passphrase.
- Forward Secrecy der Verbindung schützt keine bereits lokal gespeicherten
  Nachrichten, wenn der Tresor später zugänglich wird.
- Sichtbarkeit, Bluetooth-Adressen und Verkehrsmuster bleiben Metadaten.
  Hushlink verspricht keine Anonymität und keine sichere physische SSD-Löschung.

## Abschliessende native Prüfung

Die Sitzungsüberwachung verwendet zusätzlich zu einer Zustandsabfrage ein eigenes
natives Nachrichtenfenster mit `WTSRegisterSessionNotification` auf einem separaten
Thread. `WM_WTSSESSION_CHANGE` mit LOCK verwirft auch noch wartende
Entsperrversuche durch eine neue Generation; UNLOCK stellt keine Sitzung wieder
her. Die UI vergleicht diese Generation vor der Anzeige und leert dann eigene
Klartext-, Passwort- und Widget-Zustände. Das hängt nicht vom Repaint-Takt ab.
Ein Windows-Test sendet eine künstliche Sperrnachricht durch die echte native
Warteschlange und prüft Registrierung, Behandlung und Thread-Abbau; er ist kein
Nachweis eines tatsächlichen Win+L-Vorgangs. Die unabhängige interne Nachprüfung
von FFI-Lebensdauer, Reentranz und Generationsübergängen fand keinen konkreten
Blocker.

Die Windows-Oberfläche verwendet Direct3D12 über egui/eframe statt des auf dem
Hosted-Runner unzureichenden OpenGL-Treibers. Der ausdrücklich gewählte
System-Shadercompiler FXC benötigt keine zusätzliche `dxcompiler.dll`; Windows
kann den WARP-Softwareadapter verwenden. Eine erneute RustSec-Prüfung des
aktualisierten Lockfiles meldete wiederum keine Sicherheitslücken oder
Unsoundness-Warnungen; der dokumentierte ttf-parser-Wartungshinweis bleibt.

Auch die Inaktivitätssperre wird vom Hintergrundakteur kontrolliert; ein
monotoner Aktivitätszeitpunkt und ein Regressionstest decken den Fall ohne UI-
Frames ab. Bei Rückkehr nach einer langen Framepause wird die Frist vor neuer
Eingabe geprüft. Beim Sperren und nach Passwortübergabe werden auch Text-,
Clipboard-Paste- und IME-Eingabepuffer des aktuellen egui-Frames gelöscht.
