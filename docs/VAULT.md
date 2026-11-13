# Tresorformat, Version 1

Hushlink schreibt ausschliesslich verschlüsselte Profildaten. Das Verzeichnis wird vom Runtime-Modul aus dem lokalen, benutzerspezifischen Windows-App-Datenordner gewählt. Eine portable EXE speichert keinen Tresor neben der EXE. Das Passwort wird weder gespeichert noch übertragen. Ein vergessenes Passwort bedeutet ohne eigene Sicherung und bekanntes Passwort den Verlust des Profils, der Identität und der lokalen Verläufe; ein eingebauter Wiederherstellungsmechanismus existiert nicht.

## Schlüssel und Parameter

Ein Profil besitzt einen unabhängigen, zufälligen 256-Bit-Tresorschlüssel. Ein Passwortschlüssel schützt ausschliesslich diesen Tresorschlüssel. Der Tresorschlüssel schützt den gesamten JSON-Inhalt: Anzeigename, privaten und öffentlichen Identitätsschlüssel, Kontakte einschliesslich bestätigter öffentlicher Identitäten und Bluetooth-Adressen, Nachrichten, Zustände, Zeitangaben und Einstellungen. Verbindungsschlüssel entstehen separat im Noise-Protokoll und gelangen nicht in den Tresor.

Passwortableitung: Argon2id, Version 0x13, 32 Byte Ausgabe, frischer zufälliger 16-Byte-Salt, 65'536 KiB Speicher, drei Durchläufe, eine Lane. 64 MiB und drei Durchläufe folgen der speichersparenden zweiten Empfehlung des RFC 9106; eine Lane hält die Verarbeitung im einzelnen Hintergrundarbeiter vorhersehbar. Dies ist keine Aussage über ein Passwort mit geringer Entropie. Argon2id erschwert Offline-Angriffe, verhindert sie aber nicht. Die UI verlangt mindestens zwölf Zeichen; lange einzigartige Passphrasen bleiben notwendig. Lokale Wartezeiten ersetzen den KDF-Schutz nicht.

Der Parser akzeptiert ausschliesslich Version 1, die unten beschriebenen Algorithmen und begrenzte KDF-Werte: Speicher 64–256 MiB, drei bis sechs Durchläufe, eine bis vier Lanes. Ungültige Werte werden vor der Ableitung verworfen. Eine Datei ist insgesamt auf 16 MiB begrenzt. Passwörter über 1'024 UTF-8-Byte werden abgelehnt. Namen sind auf 80 UTF-8-Byte ohne Steuerzeichen begrenzt; maximal 500 Kontakte und 10'000 Nachrichten mit jeweils höchstens 16 KiB Text sind zulässig. Die Dateigrenze kann vorher erreicht werden. Die Inaktivitätssperre ist auf 1 bis 120 Minuten einstellbar.

Beide Verschlüsselungen verwenden die etablierte RustCrypto-Implementierung von XChaCha20-Poly1305 mit 24-Byte-Nonce und 16-Byte-Authentifizierungstag. Zufallswerte stammen aus dem Betriebssystem über `OsRng`. Jeder neue Tresor verwendet einen neuen Salt, Tresorschlüssel, Wrapping-Nonce und Inhaltsnonce. Jede Speicherung verwendet eine neue Inhaltsnonce; Passwortwechsel erzeugen zusätzlich einen neuen Salt und eine neue Wrapping-Nonce. Es werden keine kryptografischen Algorithmen selbst implementiert.

## Binärlayout

Alle Ganzzahlen sind Little Endian. Die einzigen unverschlüsselten Felder beschreiben das Format, die Ableitungsparameter, die Nonces und die verschlüsselten Längen beziehungsweise den verschlüsselten Tresorschlüssel. Keine Kontaktnamen, Schlüsselidentitäten, Nachrichtenzahlen oder Zeitstempel stehen im Header. Die Gesamtdateigrösse bleibt sichtbar.

| Offset | Länge | Bedeutung |
| --- | ---: | --- |
| 0 | 8 | `HUSHVLT` gefolgt von NUL |
| 8 | 2 | Formatversion 1 |
| 10 | 1 | KDF-Kennung 1 = Argon2id v0x13 |
| 11 | 1 | AEAD-Kennung 1 = XChaCha20-Poly1305 |
| 12 | 4 | Argon2-Speicher in KiB |
| 16 | 4 | Argon2-Durchläufe |
| 20 | 4 | Argon2-Lanes |
| 24 | 16 | Passwortsalt |
| 40 | 24 | Wrapping-Nonce |
| 64 | 24 | Inhaltsnonce |
| 88 | 4 | Länge des verschlüsselten Inhalts einschliesslich Tag |
| 92 | 48 | Verschlüsselter 32-Byte-Tresorschlüssel einschliesslich Tag |
| 140 | variabel | Verschlüsselter JSON-Inhalt einschliesslich Tag |

Die Bytes 0–63 sind Additional Authenticated Data (AAD) der Schlüsselhülle. Der gesamte Header 0–139 ist AAD der Inhaltsverschlüsselung. Damit sind **alle** Headerfelder authentifiziert. Der stabile Wrapping-Präfix bleibt bei normalen Speichervorgängen gleich; die Inhaltsnonce und Länge können sich ändern, ohne den Passwortschlüssel im Arbeitsspeicher aufzubewahren. Kein Inhalt wird vor erfolgreicher Prüfung des zugehörigen Tags interpretiert oder freigegeben. Änderungen am Inhaltsnonce, an der Hülle oder am Header führen zu einem Abbruch. Ungültige Längen und zusätzliche Bytes sind unzulässig.

Passwortänderung prüft zunächst das bisherige Passwort an der Schlüsselhülle und schützt denselben unabhängigen Tresorschlüssel mit einem neu abgeleiteten Schlüssel und frischen Zufallswerten. Der Inhalt wird ebenfalls frisch verschlüsselt; Identität, Kontakte und Verlauf bleiben erhalten. Ein altes, selbst angefertigtes Backup behält selbstverständlich sein altes Passwort.

## Speichern und Fehler

Eine dauerhafte `.lock`-Begleitdatei besitzt eine exklusive Betriebssystemsperre (`File::try_lock`) über die gesamte entsperrte Sitzung. Der leere Sperrdatei-Inode wird absichtlich nicht gelöscht; nach einem Prozessabbruch gibt das Betriebssystem die Sperre frei. Dies verhindert konkurrierende Hushlink-Schreibvorgänge. Der verschlüsselte Dateihash wird ausserdem vor jeder Ersetzung mit dem beim Öffnen beziehungsweise letzten Speichern bekannten Wert verglichen. Externe Änderungen, Beschädigung oder Löschung führen zum Konflikt; die vorhandene Datei wird dann nicht ersetzt. Gegen gleichzeitige böswillige Änderungen durch einen Prozess mit denselben lokalen Benutzerrechten entsteht dadurch keine neue Sicherheitsgrenze.

Speichern schreibt einen neuen, zufällig benannten `.hushlink-*.tmp`-Pfad im selben Verzeichnis mit exklusiver Neuanlage. Die Datei enthält nur das fertig verschlüsselte Format und wird vor der Ersetzung mit `sync_all` synchronisiert. Windows ersetzt über `MoveFileExW` auf demselben Volume mit `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`; die erstmalige Anlage erlaubt kein Überschreiben. `MOVEFILE_COPY_ALLOWED` ist ausgeschlossen. Unter Unix verwendet die Testimplementierung atomare Umbenennung beziehungsweise eine exklusive Verknüpfung für die Erstanlage und synchronisiert das Elternverzeichnis.

Bei fehlgeschlagenen Schreibvorgängen werden vorhandene Daten nicht absichtlich gelöscht. Ein verschlüsseltes temporäres Fragment oder eine vollständige temporäre Datei kann nach Fehler oder Absturz verbleiben. Es wird niemals automatisch als Tresor geöffnet und ersetzt keine beschädigte Datei. Für Wiederherstellung zuerst die vorhandenen verschlüsselten Dateien kopieren; keine Datei auf Verdacht überschreiben. Dateisystem, Laufwerk und Betriebssystem bestimmen die letzte Haltbarkeit bei Stromausfall; eine Garantie gegen defekte Hardware besteht nicht. Eigene verschlüsselte Sicherungen bleiben sinnvoll. Löschen von Chats entfernt sie aus der aktuellen verschlüsselten Datei; physische sichere Löschung auf SSDs, in Backups oder im Dateisystemjournal wird nicht zugesichert.

Falsches Passwort und authentifizierte Manipulation lassen sich kryptografisch nicht immer unterscheiden. Die App meldet deshalb einen gemeinsamen Authentifizierungsfehler. Fehlerhafte Formatkennung, Längen oder Parameter werden verständlich gemeldet. Fehler führen niemals zur automatischen Neuanlage oder zum Überschreiben einer vorhandenen Datei.

## Speicherhygiene

Passwortableitungs- und Tresorschlüssel, Argon2-Speicherblöcke sowie serialisierte Klartextpuffer verwenden `Zeroizing`; das Argon2-Feature `zeroize` ist aktiviert. Profil-, Kontakt- und Nachrichtenobjekte implementieren `ZeroizeOnDrop`. Der Tresor speichert keinen Passworttext und keinen Passwortschlüssel. Sperren muss auf Runtime-/UI-Ebene auch Sitzungen, anstehende Aufgaben und sichtbare Kopien verwerfen. Bibliotheksinterne temporäre Kopien, Allocator-Speicher, Swap, Grafiktreiberspeicher oder Betriebssystemabbilder lassen sich nicht vollständig kontrollieren. Hushlink erzeugt keine Nachrichtenlogs, Klartext-Backups oder eigenen Crash-Reports; dies ist keine Zusicherung gegen Administratorzugriff, Schadsoftware oder Betriebssystem-Dumps.

## Prüfung und Quellen

Automatisierte Tests verwenden ausschliesslich künstliche Daten: Roundtrip mit Unicode und Zeilenumbrüchen, Speicheränderungen mit Nonce-Rotation, falsches Passwort, beschädigter Inhalt und Header, Passwortwechsel mit Verlaufsbeibehaltung, gleichzeitige Instanzen, Begrenzungen und abgeschnittene Header, externe Beschädigung ohne Überschreiben sowie Prüfung sämtlicher gespeicherter Testdateien auf eindeutigen Klartext. Der Windows-Pfad muss zusätzlich unter Windows getestet werden; Linux-Tests allein belegen keine Windows-Dateisystemeigenschaften.

- [RFC 9106, Argon2](https://www.rfc-editor.org/rfc/rfc9106.html), insbesondere Abschnitte 4 und 7.4, abgerufen am 13.11.2026.
- [RustCrypto Argon2](https://docs.rs/argon2/0.5.3/argon2/).
- [RustCrypto ChaCha20-Poly1305/XChaCha20-Poly1305](https://docs.rs/chacha20poly1305/0.10.1/chacha20poly1305/).
- [Microsoft: MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw), abgerufen am 13.11.2026.
- [Rust File locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).
