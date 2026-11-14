# Tresorformat: Hülle 1, Profilschema 2

Hushlink schreibt Profil, private Identität, Kontakte, Nachrichten und gespeicherte Dateianhänge in **eine verschlüsselte Tresordatei**. Standard ist der lokale benutzerspezifische Windows-App-Datenordner; beim Erststart beziehungsweise im gesperrten Zustand kann eine andere neue oder bestehende Tresordatei gewählt werden. Ein entsperrter Tresor lässt sich über die Einstellungen an einen freien Zielpfad verschieben. Eine portable EXE speichert keinen Tresor neben der EXE. Ein externes Laufwerk muss verfügbar und beschreibbar sein; Sicherungskopien und Laufwerkszugriff bleiben in der Verantwortung der Person am Gerät. Das Passwort wird weder gespeichert noch übertragen. Ein vergessenes Passwort bedeutet ohne eigene Sicherung und bekanntes Passwort den Verlust des Profils, der Identität und der lokalen Verläufe; ein eingebauter Wiederherstellungsmechanismus existiert nicht.

## Schlüssel und Parameter

Ein Profil besitzt einen unabhängigen, zufälligen 256-Bit-Tresorschlüssel. Ein Passwortschlüssel schützt ausschliesslich diesen Tresorschlüssel. Der Tresorschlüssel schützt den gesamten JSON-Inhalt: Anzeigename, privaten und öffentlichen Identitätsschlüssel, Kontakte einschliesslich bestätigter öffentlicher Identitäten und Bluetooth-Adressen, Nachrichten, Zustände, Zeitangaben, Einstellungen sowie Dateinamen, Dateiinhalt und Datei-Prüfsummen. Verbindungsschlüssel entstehen separat im Noise-Protokoll und gelangen nicht in den Tresor.

Passwortableitung: Argon2id, Version 0x13, 32 Byte Ausgabe, frischer zufälliger 16-Byte-Salt, 65'536 KiB Speicher, drei Durchläufe, eine Lane. 64 MiB und drei Durchläufe folgen der speichersparenden zweiten Empfehlung des RFC 9106; eine Lane hält die Verarbeitung im einzelnen Hintergrundarbeiter vorhersehbar. Dies ist keine Aussage über ein Passwort mit geringer Entropie. Argon2id erschwert Offline-Angriffe, verhindert sie aber nicht. Die UI verlangt mindestens zwölf Zeichen; lange einzigartige Passphrasen bleiben notwendig. Lokale Wartezeiten ersetzen den KDF-Schutz nicht.

Der Parser akzeptiert ausschliesslich Version 1, die unten beschriebenen Algorithmen und begrenzte KDF-Werte: Speicher 64–256 MiB, drei bis sechs Durchläufe, eine bis vier Lanes. Ungültige Werte werden vor der Ableitung verworfen. Eine Datei ist insgesamt auf 128 MiB begrenzt. Passwörter über 1'024 UTF-8-Byte werden abgelehnt. Namen sind auf 80 UTF-8-Byte ohne Steuerzeichen begrenzt; maximal 500 Kontakte und 10'000 Nachrichten mit jeweils höchstens 16 KiB Text sind zulässig. Ein einzelner Anhang enthält maximal 20 MiB; höchstens 1’000 Anhänge mit zusammen 80 MiB Rohdaten sind erlaubt. Die Base64-Darstellung im verschlüsselten JSON benötigt zusätzlich Platz; die 128-MiB-Dateigrenze kann mit viel Text früher erreicht werden. Ungültige Namen, unpassende Nachrichtenverknüpfungen, doppelte Nachrichten-IDs pro Kontakt, global doppelte Anhang-IDs und abweichende SHA-256-Prüfsummen werden verworfen. SHA-256 ersetzt hierbei nicht den AEAD-Authentifizierungstag. Die Inaktivitätssperre ist auf 1 bis 120 Minuten einstellbar.

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

## Profilschema und Abwärtskompatibilität

Die binäre Hülle bleibt Version 1. Der entschlüsselte Inhalt trägt seit Hushlink 0.2 `schema_version: 2`; dazu kommen `attachments` und die optionale `attachment_id` einer Nachricht. Anhänge enthalten ID, Kontakt-ID, Namen, Base64-Daten, SHA-256, Richtung und Zeitstempel. Sie dürfen nur gemeinsam mit einer passenden Nachricht existieren. Unbekannte Felder beziehungsweise zukünftige Schemaversionen werden abgelehnt.

Ein 0.1-Profil ohne Schemaversion, Anhänge und neue Desktop-Einstellungen wird als Schema 1 gelesen. Identität, Passwort, Kontakte und Verlauf bleiben erhalten. Bei der nächsten Speicherung schreibt Hushlink Schema 2; ältere App-Versionen können diesen Inhalt nicht lesen. Vor einem Versionswechsel ist eine eigene verschlüsselte Sicherung sinnvoll. Die Desktop-Einstellungen erhalten bei alten Profilen Standardwerte; vorhandene Einstellungen im Bootstrap haben beim Start Vorrang, damit ein Update-Opt-out beim Entsperren erhalten bleibt.

## Speicherort und Bootstrap

Neben der Standardposition liegen `hushlink-settings.json` und eine leere Konfigurationssperrdatei. Die versionierte JSON-Datei enthält **nur** den ausgewählten absoluten Tresorpfad und die drei nicht geheimen Desktop-Einstellungen für Autostart, Tray und automatische Updates. Diese Informationen müssen vor der Passwortabfrage verfügbar sein; sie sind nicht verschlüsselt. Wer bereits im Dateinamen sensible Informationen verwendet, macht diese dadurch lokal sichtbar. Der Parser begrenzt die Datei auf 32 KiB und lehnt unbekannte Versionen, Zusatzfelder sowie relative Pfade ab. Schreiben erfolgt atomar mit derselben Synchronisierung wie beim Tresor. Eine beschädigte Konfiguration erzeugt keinen neuen Tresor. Die ausdrückliche Auswahl eines anderen Pfades kann einen beschädigten JSON-Inhalt reparieren; Autostart und Updates bleiben dabei zunächst ausgeschaltet. Eine übergrosse Konfiguration muss nach eigener Prüfung entfernt oder ersetzt werden.

Ausserdem bleibt neben jeder benutzten Tresordatei eine leere `.lock`-Datei. Sie enthält keine Profildaten und ist keine zweite Datenbank. Ohne diese Betriebssystemsperre kann die App konkurrierende Änderungen nicht zuverlässig abweisen. Der Inhalt des Profils und alle gespeicherten Anhänge liegen weiterhin vollständig in einer einzigen verschlüsselten Datei. Deinstallation bewahrt die gewählte Tresordatei.

Beim Verschieben bleibt die Quelle gesperrt. Zuerst wird ihr gültiger Inhalt mit frischer Nonce gespeichert; dann schreibt und synchronisiert die App ausschliesslich den fertigen verschlüsselten Inhalt am freien Zielpfad. Existierende Ziele werden nie ersetzt. Erst nach dauerhaftem Zielschreibvorgang wird der Bootstrap atomar auf den neuen Pfad gesetzt. Danach entfernt die App die unveränderte Quelle und hält die Zieldatei weiterhin gesperrt. Bei einem Fehler bleibt die Quelle erhalten. Meldet der Bootstrap-Schreibvorgang einen Fehler, kann seine atomare Ersetzung trotzdem bereits erfolgt sein, während nur die letzte Synchronisierung fehlgeschlagen ist. Daher bleiben in diesem Fall beide verschlüsselten Kopien erhalten; weitere Schreibvorgänge sind bis zum erneuten Öffnen blockiert. Die Runtime sperrt bei jedem fehlgeschlagenen Umzug und liest den Locator neu ein, statt eine möglicherweise veraltete Kopie weiterzuschreiben. Ein Fehler bei anschliessender Bereinigung lässt den neuen Pfad aktiv und meldet, dass eine alte verschlüsselte Kopie manuell geprüft werden muss. Nach Stromausfall können zwei verschlüsselte Kopien verbleiben; der Ablauf soll niemals einen gültigen Locator auf eine nur teilweise geschriebene Datei erzeugen. Diese Reihenfolge ersetzt keine Sicherung und garantiert keine Haltbarkeit auf defekter Hardware.

Dateien werden nur nach Zustimmung und erfolgreicher Identitätsprüfung empfangen. Unvollständige Übertragungen bleiben im Arbeitsspeicher und werden beim Sperren verworfen. Vollständige, geprüfte Dateidaten und die zugehörige Nachricht werden gemeinsam im Tresor gespeichert. Erst danach darf eine verschlüsselte Empfangsbestätigung gesendet werden. Es gibt keine automatisch erzeugten Klartext-Dateianhänge neben dem Tresor. „Datei speichern“ ist ein ausdrücklicher Export an einen ausgewählten freien Pfad: Die exportierte Datei ist dann unverschlüsselt und unterliegt nicht mehr der Tresorsperre. Die App führt sie nicht automatisch aus.

## Speichern und Fehler

Eine dauerhafte `.lock`-Begleitdatei besitzt eine exklusive Betriebssystemsperre (`File::try_lock`) über die gesamte entsperrte Sitzung. Der leere Sperrdatei-Inode wird absichtlich nicht gelöscht; nach einem Prozessabbruch gibt das Betriebssystem die Sperre frei. Dies verhindert konkurrierende Hushlink-Schreibvorgänge. Der verschlüsselte Dateihash wird ausserdem vor jeder Ersetzung mit dem beim Öffnen beziehungsweise letzten Speichern bekannten Wert verglichen. Externe Änderungen, Beschädigung oder Löschung führen zum Konflikt; die vorhandene Datei wird dann nicht ersetzt. Gegen gleichzeitige böswillige Änderungen durch einen Prozess mit denselben lokalen Benutzerrechten entsteht dadurch keine neue Sicherheitsgrenze.

Speichern schreibt einen neuen, zufällig benannten `.hushlink-*.tmp`-Pfad im selben Verzeichnis mit exklusiver Neuanlage. Die Datei enthält nur das fertig verschlüsselte Format und wird vor der Ersetzung mit `sync_all` synchronisiert. Windows ersetzt über `MoveFileExW` auf demselben Volume mit `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`; die erstmalige Anlage erlaubt kein Überschreiben. `MOVEFILE_COPY_ALLOWED` ist ausgeschlossen. Unter Unix verwendet die Testimplementierung atomare Umbenennung beziehungsweise eine exklusive Verknüpfung für die Erstanlage und synchronisiert das Elternverzeichnis.

Bei fehlgeschlagenen Schreibvorgängen werden vorhandene Daten nicht absichtlich gelöscht. Ein verschlüsseltes temporäres Fragment oder eine vollständige temporäre Datei kann nach Fehler oder Absturz verbleiben. Es wird niemals automatisch als Tresor geöffnet und ersetzt keine beschädigte Datei. Für Wiederherstellung zuerst die vorhandenen verschlüsselten Dateien kopieren; keine Datei auf Verdacht überschreiben. Dateisystem, Laufwerk und Betriebssystem bestimmen die letzte Haltbarkeit bei Stromausfall; eine Garantie gegen defekte Hardware besteht nicht. Eigene verschlüsselte Sicherungen bleiben sinnvoll. Löschen von Chats entfernt sie aus der aktuellen verschlüsselten Datei; physische sichere Löschung auf SSDs, in Backups oder im Dateisystemjournal wird nicht zugesichert.

Falsches Passwort und authentifizierte Manipulation lassen sich kryptografisch nicht immer unterscheiden. Die App meldet deshalb einen gemeinsamen Authentifizierungsfehler. Fehlerhafte Formatkennung, Längen oder Parameter werden verständlich gemeldet. Fehler führen niemals zur automatischen Neuanlage oder zum Überschreiben einer vorhandenen Datei.

## Speicherhygiene

Passwortableitungs- und Tresorschlüssel, Argon2-Speicherblöcke sowie serialisierte Klartextpuffer verwenden `Zeroizing`; das Argon2-Feature `zeroize` ist aktiviert. Profil-, Kontakt-, Nachrichten- und Anhangobjekte implementieren `ZeroizeOnDrop`. Die Base64-Zwischenpuffer bei Anhangkodierung und Dekodierung verwenden ebenfalls `Zeroizing`. Der Tresor speichert keinen Passworttext und keinen Passwortschlüssel. Sperren muss auf Runtime-/UI-Ebene auch Sitzungen, anstehende Aufgaben und sichtbare Kopien verwerfen. Bibliotheksinterne temporäre Kopien, Allocator-Speicher, Swap, Grafiktreiberspeicher oder Betriebssystemabbilder lassen sich nicht vollständig kontrollieren. Hushlink erzeugt keine Nachrichtenlogs, Klartext-Backups oder eigenen Crash-Reports; dies ist keine Zusicherung gegen Administratorzugriff, Schadsoftware oder Betriebssystem-Dumps.

## Prüfung und Quellen

Automatisierte Tests verwenden ausschliesslich künstliche Daten: Roundtrip mit Unicode und Zeilenumbrüchen, Speicheränderungen mit Nonce-Rotation, falsches Passwort, beschädigter Inhalt und Header, Passwortwechsel mit Verlaufsbeibehaltung, gleichzeitige Instanzen, Begrenzungen und abgeschnittene Header, externe Beschädigung ohne Überschreiben, Anhang-Roundtrip samt Passwortwechsel, Schema-1-Migration, Hash-/Namens-/Verknüpfungsgrenzen, erfolgreicher Speicherortwechsel sowie unklarer Locator-Commit mit Erhalt beider Kopien und gesperrten weiteren Schreibvorgängen sowie veränderte Quelldateien ohne fremde Inhalte zu löschen. Gespeicherte Testdateien werden auf eindeutige Nachrichten, Dateinamen, Roh- und Base64-Dateiinhalte sowie Passwörter geprüft. Bootstrap-Tests prüfen Pfadauswahl, dauerhaftes Update-Opt-out und beschädigte beziehungsweise übergrosse Konfiguration. Der Windows-Pfad muss zusätzlich unter Windows getestet werden; Linux-Tests allein belegen keine Windows-Dateisystemeigenschaften.

- [RFC 9106, Argon2](https://www.rfc-editor.org/rfc/rfc9106.html), insbesondere Abschnitte 4 und 7.4, abgerufen am 13.11.2026.
- [RustCrypto Argon2](https://docs.rs/argon2/0.5.3/argon2/).
- [RustCrypto ChaCha20-Poly1305/XChaCha20-Poly1305](https://docs.rs/chacha20poly1305/0.10.1/chacha20poly1305/).
- [Microsoft: MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw), abgerufen am 13.11.2026.
- [Rust File locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).
