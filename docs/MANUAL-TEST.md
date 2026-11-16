# Zwei-PC-Abnahme – Windows 11 x64

Status: **nicht durchgeführt in der Linux-Cloud-Umgebung**. GitHub-Windows-Runner besitzen hier keine bestätigte physische Bluetooth-Gegenstelle. Ein In-Memory-Test ist kein Funknachweis. Vor produktiver Nutzung diesen Ablauf auf zwei geeigneten physischen Geräten durchführen und Ergebnisse mit Build-Commit, Windows-Version, Adapter-/Treiber-Version, Bildschirmauflösung und Skalierung festhalten. Nur künstliche Profile und Nachrichten verwenden.

## Vorbereitung

- PC A und PC B mit Windows 11 x64, Bluetooth Classic/RFCOMM, funktionierendem Treiber und normalem Benutzerkonto. Internet/WLAN ausschalten, um den ausschliesslichen Bluetooth-Betrieb sichtbar zu belegen.
- Gleiches Release-EXE auf beide PCs laden, SHA-256 prüfen. ZIP-EXE mit einzelnem Download vergleichen. Start ohne Administratorrechte und ohne installierte Rust-/VC-Redistributable-Werkzeuge auf sauberem System prüfen. SmartScreen-/Unternehmensrichtlinien dokumentieren.
- Einen PC mit 100 %, einen mit 150 % Skalierung verwenden; zusätzlich 125/200 %, 720×540-Fenster, Tastatur-/Tab-Navigation prüfen.

## Grundablauf

1. Erststart: keine Beispielkontakte oder erfolgreiche Verbindung sichtbar. Anzeigenamen „Prüfung Ä“ und „Test B 🦉“, langes Testpasswort plus Bestätigung festlegen. Abweichende/zu kurze Bestätigung abweisen. Verlustwarnung sichtbar.
2. App beenden/neustarten: bestehender Tresor verlangt Passwort. Falsches Passwort ändert keine Datei; korrektes öffnet Profil. Zweite Instanz mit demselben Profil darf nicht gleichzeitig schreiben.
3. Bluetooth deaktivieren, dann aktivieren. Verständliche Meldungen für fehlenden Adapter/Classic-Unterstützung prüfen. Keine fiktiven Geräte zeigen. Auf A Empfang starten, auf B Suche bewusst starten. Beobachten, dass Hushlink beim Stoppen keine eigene SDP-Ankündigung hinterlässt und Windows-Sichtbarkeit nicht heimlich dauerhaft ändert.
4. Gegebenenfalls Windows-Pairing durchführen. B verbindet A über den Hushlink-Dienst. A lehnt Anfrage zunächst ab; B muss einen Unterbruch und keinen Chat sehen. Erneut verbinden, A nimmt an.
5. Vor Bestätigung auf beiden Seiten keinen Text senden können; auch nach nur einseitiger Bestätigung gesperrt. Vollständiger Sicherheitscode stimmt auf beiden Geräten überein. Beide Personen vergleichen unabhängig und bestätigen. Kontakt erst dann gespeichert; Verbindung korrekt angezeigt.
6. Beidseitig „Synthetisch-ÄÖÜ-東京-🦉-2026“ sowie mehrere Zeilen senden. Enter einmal sendet genau eine Nachricht; Shift+Enter fügt Zeilenumbruch ein. Leerer Text wird abgewiesen; 16-KiB-Grenze sinnvoll angezeigt. Statusfolge prüfen: ausstehend → gesendet → empfangen nach Gegenbestätigung. „Empfangen“ ist keine Lesebestätigung.
7. Nachricht senden, Funk/Empfang sofort unterbrechen. Nicht bestätigte Nachrichten müssen fehlgeschlagen werden; keine automatische Doppelübermittlung. Wiederverbinden führt neuen Handshake durch. Identität stimmt mit Kontakt überein; beide Apps dürfen ihre bereits gespeicherten, bestätigten Schlüssel automatisch erneut bestätigen. Beide Richtungen funktionieren wieder.
8. Apps neu starten/entsperren: Kontakt und Verlauf unverändert vorhanden. Dateien unter lokalem App-Datenordner mit Hex-/Textsuche auf eindeutigen Testtext, Anzeigenamen und Bluetooth-Adressen prüfen; keine Klartexttreffer. Keine Chat-Logs/temporären Klartextdateien erzeugt.
9. Manuell sperren (Button und Strg+L), während Nachricht/Discovery läuft: Verbindung endet, sichtbare Inhalte und Eingabe verschwinden. Entsperren baut keine alte Sitzung wieder auf. Keine verspäteten Worker-Ereignisse zeigen alte Inhalte. Passwortdialog-Undo darf alte Passwörter nicht wiederherstellen.
10. Inaktivität auf 1 Minute stellen, ohne Eingabe abwarten. App sperrt und schliesst Sitzung. Windows mit Win+L sperren, entsperren: Hushlink bleibt gesperrt. Auch während Verbindungsaufbau, Handshake, Kontaktprüfung und Passwortarbeit prüfen. Zeitanpassung des Systems darf die monotone Inaktivitätsmessung nicht umgehen.
11. Passwort mit falschem bisherigem Passwort ändern: abweisen. Mit korrektem Passwort ändern: Kontakte und Verlauf bleiben. Nach Neustart altes Passwort abweisen, neues akzeptieren. Keine unverschlüsselte Zwischenkopie.
12. Profil auf B bewusst neu anlegen (Testtresor zuvor bei geschlossener App sichern), gleiche Bluetooth-Adresse. A muss Schlüsselwechsel blockieren. Abbrechen erhält alten Kontakt. Bewusste erneute Prüfung erfordert neuen vollständigen Code auf beiden Seiten; frühere Bestätigung darf nicht gelten.
13. Verlauf auf A löschen und neu starten: lokal entfernt, auf B unverändert. Keine physische SSD-Löschung versprechen.

## Fehler, Abbruch und Dateisicherheit

- Gegenstelle während Passwortänderung, Speicherung, Verbindung oder Empfang schliessen. Keine eingefrorene Oberfläche; Abbruch führt zu verständlichem Zustand. Prozess während Tresorspeicherung mit ausschliesslich Testdaten beenden; danach muss ein vollständig gültiger alter oder neuer Tresor vorliegen, keine automatisch überschriebene beschädigte Datei.
- Geschlossenen künstlichen Tresor kürzen und einzelne Bytes ändern. Öffnen verständlich abweisen, Datei bitgenau erhalten. Vor jedem Fall neue Testkopie verwenden. Vorhandenen defekten Tresor niemals als Erststart überschreiben.
- Schreibgeschützten Datenordner, vollen Datenträger und geöffnete zweite Instanz prüfen: keine falschen Zustellbestätigungen oder stillen Datenverluste. Dateifehler müssen sichtbar werden.
- Nach unerwartetem Adapterverlust erneut verfügbar machen und neu verbinden. Empfang starten/stoppen mehrfach; keine hängenden Dienste/Threads. Zwei gleichzeitige Anfragen sinnvoll begrenzen.
- 500 Kontakte/10.000 Nachrichten/128-MiB-Tresorgrenze und 80 MiB Gesamtanhänge mit ausschliesslich generierten Testdaten prüfen; Fehler erlauben Löschen alter Verläufe. Langes Gespräch bis Sitzungsgrenze erfordert neue Sitzung.

## Dateiversand, Tresorwahl, Tray und Updates ab v0.2

1. Auf beiden PCs v0.2 installieren. Erstinstallation, Startmenü, Deinstallation und Autostart-Auswahl mit normalem Konto prüfen. Autostart zeigt keine entsperrten Inhalte. Die separat heruntergeladene EXE muss ohne Installer ebenfalls starten. Vorhandenen v0.1-Testtresor vorher geschlossen sichern; Migration erhält Identität, Kontakte und Verlauf. Die alte App muss die neue gespeicherte Kopie sicher abweisen.
2. Gesperrt einen neuen Speicherort mit Leerzeichen/Umlauten wählen, Profil anlegen, App neu starten: gewählter Tresor wird gefunden. Nach Chat/Dateiempfang trennen und entsperrten Tresor auf ein anderes Laufwerk verschieben. Neustart, Passwortwechsel und bestehender Verlauf/Anhänge funktionieren. Vorhandene Zieldatei, volles Laufwerk und schreibgeschütztes Ziel dürfen keine Kopie zerstören. Bei unsicherem Abschluss erhaltene Kopien prüfen, ohne automatisches Überschreiben.
3. Vor/bei einseitiger Kontaktprüfung darf kein Dateiversand möglich sein. Nach beidseitiger Bestätigung Dateiangebot mit Unicode-Dateinamen senden, zunächst ablehnen. Keine Datei automatisch öffnen, keinen Klartextexport erzeugen. Erneut anbieten, annehmen und Fortschritt beobachten. SHA-256 des bewusst exportierten Anhangs mit der künstlichen Quelldatei vergleichen. Export darf keine vorhandene Datei ersetzen.
4. Leere Datei, Binärdatei, mehrzeiliger Text, 20 MiB und 20 MiB plus ein Byte prüfen. Grenze abweisen. In beide Richtungen gleichzeitig übertragen, währenddessen Nachrichten senden. Abbruch vor/nach Annahme, während Blöcken und beim Abschluss prüfen. Kein falscher Empfangsstatus, keine alten Daten nach neuer Verbindung. In einem künstlichen Tresor/Logs nach eindeutigen Datei-/Nachrichteninhalten und Dateinamen suchen: keine Klartexttreffer.
5. Funkunterbruch, Speicherfehler auf dem Empfänger und Sperre während Übertragung dürfen keine positive Speicherbestätigung erzeugen. Nach Neustart bestätigte Anhänge weiterhin exportieren können. Verlauf löschen entfernt dessen lokale Anhänge. Gegenstelle und bewusst exportierte Kopien bleiben unverändert.
6. Fenster schliessen: App sperrt, trennt und verschwindet nur bei funktionierendem Tray. Tray öffnen, sperren, Updates prüfen und beenden. Erneuter normaler App-Start zeigt die existierende Instanz; erneuter `--tray`-Start erzeugt kein zweites Fenster. Explorer-Neustart und 100/150/200-%-Tray-DPI prüfen. Win+L und Inaktivität auch bei verstecktem Fenster prüfen.
7. Updates deaktivieren und neu starten: keine automatische GitHub-Anfrage. Erneut aktivieren; ohne Netz verständlicher Fehler bei weiter nutzbarem Chat. Nur für Updateprüfung Internet einschalten. Bei einer künftig tatsächlich höheren veröffentlichten Version Download, bewusste Installation, Warten auf App-Ende, Neustart gesperrt im Tray und Erhalt von Tresor/Autostart-Verzicht prüfen. Manipuliertes Download-Artefakt muss vor Ausführung abgewiesen werden. Aktuell automatische Update-Parser-/Integritätstests und Installer-Wiederinstallation nicht als vollständiges Versionsupgrade ausgeben.
8. Deinstallation entfernt Programm, Shortcuts und Autostart, erhält jedoch gewählten Tresor und Konfiguration. Neuinstallation mit vorhandenem Tresor funktioniert. Keine sichere physische SSD-Löschung behaupten.

## Ergebnisprotokoll

| Prüffall | PC/Adapter/Build | Ergebnis | Beobachtung |
|---|---|---|---|
| Start ohne Zusatzinstallation | offen | nicht durchgeführt | |
| Classic-SDP anbieten/finden/verbinden/annehmen | offen | nicht durchgeführt | |
| Beidseitiger Chat ohne IP | offen | nicht durchgeführt | |
| Code-/Identitätsprüfung | offen | nicht durchgeführt | |
| Windows-Sperre und Abbruch | offen | nicht durchgeführt | |
| Neustart/Passwortwechsel/Löschen | offen | nicht durchgeführt | |
| Skalierung/Tastatur/Emojis | offen | nicht durchgeführt | |

## Erweiterungen v0.3 – auf beiden Geräten prüfen

- Vorab geschlossenen v0.2-Testtresor sichern. Upgrade erhält Identität, Kontakte, Nachrichten und Anhänge; neue Eigennamen/Favoriten bleiben nach Neustart und Passwortwechsel erhalten. Kontaktentfernung löscht lokal auch Anhänge/Teilstände und verlangt beim nächsten Kontakt wieder die unabhängige Prüfung.
- Verbindung mit dem Assistenten vollständig in beiden Rollen aufbauen. Fehlender/abgeschalteter Adapter, keine Geräte, Pairing-Abbruch, abgelehnte Anfrage, Zeitlimit und Schlüsselwechsel dürfen keinen vorgetäuschten Fortschritt zeigen. Vorgänge explizit stoppen; nach Schliessen eines bewusst gestarteten Empfangs Assistenten bleibt dieser bis zum Stoppen sichtbar aktiv.
- 1,5–20 MiB künstliche Datei übertragen, Funk unterbrechen. Sender neu verbinden und Datei im Verlauf erneut anbieten, Empfänger erneut bestätigen lassen. Vorhandene Bytes werden übersprungen, exportierte Enddatei hat exakt denselben SHA-256. Während des Abbruchs beide Apps neu starten und entsperren; noch einmal fortsetzen. Abrupt beenden: höchstens letzter ungesicherter Abschnitt fehlt. Ein fehlendes Receipt darf keine zweite Datei/Nachricht erzeugen.
- Erneut verbinden mit anderer Identität: kein Zugriff auf alten Teilstand. Falsche Präfixdaten/Größe/Digest werden abgewiesen. Explizites Abbrechen und Ablehnen verwerfen gespeicherte Empfangsbytes; vollständiges Löschen eines Teilstands erlaubt später einen Neubeginn. Keine Bestätigung bei Speicherfehler oder falschem vollständigem Digest.
- Eine und mehrere Dateien ins ausgewählte bestätigte Gespräch ziehen: zunächst nur Vormerkliste mit korrektem Empfänger, keine automatische Übertragung. Ohne Vertrauen, gesperrt, nach Empfängerwechsel und nach Trennung keine Freigabe. Maximal 16 lokale Dateien; Verzeichnisse/Dateien über 20 MiB sinnvoll abweisen. Sperren leert auch eine offene Vormerkliste.
- Entsperrte App minimieren/in den Hintergrund bringen. Text und Dateiangebot lösen einen generischen Windows-Hinweis ohne Namen/Vorschau aus. Klick öffnet das Fenster. Ungelesene Zähler im Kontakt/Tray prüfen; ausgewähltes sichtbares Gespräch wird gelesen. Bei deaktivierten Hinweisen, Fokus auf App, Windows-Ruhemodus und Sperre keine unzulässigen Hinweise. Geschlossenes Tray bleibt gesperrt ohne Sitzung. Explorer-Neustart und 100/150/200-%-Skalierung prüfen.

## Erweiterungen v0.4 – zwei physische PCs erforderlich

1. Geschlossenen v0.3-Tresor sichern, beide Apps aktualisieren. Schema-4-Migration,
   Kontaktidentität, Anhänge und frühere Einstellungen prüfen. Alte App darf die
   neue gespeicherte Kopie nicht öffnen. Kein Downgrade der Protokollversion.
2. Antworten in beide Richtungen, Umlaute/Emoji/Zeilenumbrüche. Verweise stimmen,
   fehlender lokaler Ursprung wird benannt. Eigene Textnachricht bearbeiten:
   Kennzeichnung sichtbar, fremde Texte/Anhänge nicht editierbar. Beide Personen
   reagieren und entfernen ihre eigene Reaktion; Änderungen bleiben bis zum
   Receipt unbestätigt. Abbruch zwischen Speicherung und Receipt, Neustart,
   erneut verbinden und explizit erneut übertragen: keine zweite Nachricht und
   kein Rücksetzen einer bereits neueren Änderung. Keine Freigabe vor Vertrauen/nach Sperre.
3. Suche mit Umlauten, über alle Kontakte und nur in gemerkten Nachrichten.
   Pins/Suche ohne Verbindung nutzbar. Suche und alle sensiblen Dialoge werden
   bei Strg+L, Inaktivität und Win+L geleert. Minimales Fenster und 100/150/200 %.
4. Schreibanzeige ist zunächst aus; einschalten, kontinuierlich tippen, Pause,
   Senden/Trennen/Sperren. Kein Entwurf übertragen, Indikator spätestens nach
   sechs Sekunden weg. Temporäre/dauerhafte Kontakt-Stummschaltung unterdrückt
   Hinweise, erhält Verlauf/Zähler und überlebt Neustart.
5. Unbekannte Anfragen 30 Minuten/dauerhaft sperren und wieder zulassen. Bekannte
   Adresse kann weiterhin anfragen, fremde Identität wird dennoch blockiert.
   Mehrere unbekannte Versuche: höchstens eine Anfrage alle zehn Sekunden.
6. Kleinen Screenshot kopieren, im fokussierten Nachrichtenfeld Strg+V sowie
   alternativ Bild-einfügen-Button prüfen. Erst bewusst anbieten, dann beim
   Empfänger annehmen. PNG exportieren und Bildinhalt vergleichen. Text-Paste
   bleibt normal. Leere/gesperrte Zwischenablage, Bild >16 MP, >20 MiB, Kontakt-
   wechsel und Sperre während Lesen/Vormerkung: keine falsche Sendung/Temporärdatei.
7. 20-MiB-Datei übertragen, Rate/Restzeit prüfen. Nach Fortsetzung dürfen nur neu
   übertragene Bytes zur Rate zählen; Wartezeit auf Annahme wird nicht eingerechnet.
8. QR-Codes beider PCs vergleichen und mit eingebauter Kamera lesen. Gesamter
   Code korrekt; anderer Kontakt, falscher Kontext und beliebige URL passen nicht.
   Keine automatische Kontaktbestätigung. Mehrere Kameras, verweigerte Berechtigung,
   Treiberfehler, Zeitlimit und Win+L: Kamera-Hilfsprozess/LED endet spätestens bei
   Abbruch. Keine aufgenommenen Bilddateien. Manueller vollständiger Vergleich bleibt möglich.
9. Windows Hello bewusst einrichten: richtiges/falsches Passwort, PIN/Biometrie,
   Abbrechen. Jede Entsperrung verlangt erneute Hello-Bestätigung. Gesperrte OS-
   Sitzung/Strg+L während Prompt darf keine alten Inhalte wieder freigeben.
   Anbieter ohne OAEP/strikte Policy wird abgewiesen; kein schwächerer Fallback.
   Passwortwechsel erhält Hello und Verlauf. Hello deaktivieren, Passwort weiter
   gültig. Kopie auf anderem Windows-Konto/PC nur mit damaligem Passwort öffnen.
10. Sicherungsordner/Intervall/Aufbewahrung konfigurieren. Nur verschlüsselte
    passende eigene Dateien werden erstellt/entfernt; fremde Dateien und Symlinks
    bleiben erhalten. Schlechtes/volles Laufwerk verständlich melden. Gesperrt
    keine automatische Sicherung. Wiederherstellen mit falschem Passwort,
    manipuliertem Backup, belegtem Ziel: keine Änderung bestehender Dateien.
    Neues freies Ziel erhält vollständig Identität, Kontakte, Texte, Anhänge und
    Einstellungen. Alte Passwörter funktionieren nur für passende alte Sicherungen.
11. Speicherübersicht: einzelne Datei/Antwort lokal löschen, neu starten,
    keine Wiederherstellung durch spätes Receipt/Retry. Gegenstelle/ältere Backups
    behalten ihre Kopien. Gesamtverlauf und Kontakt bewusst entfernen.
12. Nach Einrichtung des realen Signierdienstes einen signierten Qualifikationslauf
    ausführen. App und Installer zeigen denselben autorisierten Herausgeber und
    gültigen Zeitstempel; DER-Zertifikat-SHA-256 stimmt. Manipulierte/unsignierte
    und fremd signierte Update-Installer werden vor Start blockiert. Tatsächliches
    Upgrade zwischen zwei signierten Versionen und anschliessende Deinstallation
    mit erhaltenem Tresor prüfen. Keine Signatur ohne konkreten Nachweis behaupten.

Alle diese physischen Hardwarefälle sind hier weiterhin **nicht durchgeführt**.
Automatische und sichtbare Cloud-UI-Prüfungen werden separat in VALIDATION.md geführt.

## Messenger-Design – visuelle Abnahme

Mit getrennten, ausdrücklich künstlichen Profilen prüfen; keine privaten
Verläufe als Testbilder oder CI-Artefakte verwenden.

1. Erststart und vorhandenen Tresor bei 720 × 540 sowie 1120 × 720 öffnen;
   Formular und Speicherort/Wiederherstellung per Scrollen/Tastatur erreichen.
2. Chatliste: lange Namen/Vorschauen werden mit Auslassung gekürzt, Uhrzeit und
   Zähler überlappen nicht. Alle/Ungelesen/Favoriten und Suche tatsächlich filtern.
3. Ein-/mehrzeilige Nachrichten, Umlaute, Emoji, lange Wörter, Antworten,
   Bearbeitung, Reaktionen, Pins und Anhänge ansehen. Zum Anfang/Ende scrollen:
   keine unverhältnismässig hohen Blasen, keine abgeschnittenen Aktionsmenüs.
4. Chat-Lupe öffnet und fokussiert die Suche. Treffer, leere Suche, Alle Chats
   und Gemerkte prüfen. Dreipunktmenü offline erlaubt nur lokale Aktionen;
   Eingabe und Netzwerkaktionen bleiben ohne bestätigte Verbindung gesperrt.
5. Alle vier Einstellungskategorien bei kleinem Fenster durchscrollen.
   Wiederherstellung, Kontaktverwaltung, QR- und Dateifenster bleiben erreichbar.
6. Mit fokussierter Suche, offenem Menü und ausgefüllter Eingabe Strg+L bzw.
   Schloss klicken. Keine früheren Namen/Inhalte/Entwürfe nach der Sperre;
   Entsperren stellt keine alte Verbindung oder Suche wieder her.
7. Unter Windows 100/125/150/200-%-Skalierung sowie Monitorwechsel prüfen.
   Zeitstempel mit lokaler Uhr/Zeitzone inklusive Mitternacht/Sommerzeit
   vergleichen. Alle Icons haben sichtbare Hover-Erklärungen/Tastaturfokus.

Die Linux-Live-Prüfung deckt Erststart/Entsperren, getrennten künstlichen Verlauf,
Umlaute/Zeilenumbrüche, lange Namen, Anhänge/Antworten/Reaktionen/Pins, Suche,
kleines Fenster, Einstellungen und Strg+L ab. Windows-DPI-/Emoji-/Zeitzonen-
Abnahme und reale Zwei-PC-Zustände bleiben manuell erforderlich. Der automatische
Windows-Start-/Installer-Test ist kein Ersatz dafür.

## Gruppenchats v1.1.0 – drei physische PCs

**Noch nicht durchgeführt.** Drei Windows-11-PCs A/B/C mit Classic/RFCOMM,
identischem v1.1-Commit, künstlichen Profilen und dokumentierten Treibern verwenden.

1. Ohne Internet starten. A↔B, A↔C und B↔C jeweils paaren/verbinden, vollständige
   Codes unabhängig vergleichen und beidseitig bestätigen. Alte v1.0-Gegenstelle
   muss den Protokollwechsel verständlich abbrechen. Schlüsselwechsel blockieren.
2. A erstellt eine Gruppe mit B/C. Auf allen PCs Sync einschalten. Auf B/C
   Einladung prüfen; vor Annahme dürfen keine Gruppentexte freigegeben werden.
   In separatem Test C als unbekannten Kontakt auf B belassen: Beitritt blockiert.
3. B/C nehmen an. Von allen drei PCs Umlaute, Emoji und mehrzeilige Texte senden,
   Enter/Shift+Enter prüfen. Pro Empfänger Speicherbestätigung; keine Doppeltexte.
   Kollidierende Verbindungsversuche müssen mit Wiederholung vorankommen.
4. C ausschalten; A sendet. B bestätigt, C bleibt ausstehend. A neu starten und
   entsperren: Text/Status bleibt, Sync bleibt pausiert. Sync aktivieren, C starten:
   C erhält genau eine Kopie, Bestätigung auf A zählt hoch. Verlorenen ACK durch
   Abbruch und frische Verbindung prüfen. Dateien im 1:1-Chat regressionsprüfen.
5. A entfernt das offline C, ändert danach erneut den Namen. B muss Liste erneut
   annehmen, alte Texte dürfen nicht in neue Revision übernommen werden.
   C erneut erreichbar: die ausstehende Entfernung muss trotzdem ankommen.
   C erneut einladen: explizite neue Zustimmung nötig; alte Revision abweisen.
6. B tritt aus. Erst nach Zustellung verschwindet B als Empfänger. Besitzer
   schliesst, lokale Verläufe bleiben. Lokalen Verlauf löschen, nach alter
   Wiederholung kein Wiedererscheinen. Keine physische SSD-Löschzusage.
7. Während Sync manuell, durch Inaktivität und Win+L sperren: Sitzungen/Arbeiter
   enden, sichtbare Inhalte verschwinden. Entsperren startet Sync nicht heimlich.
   Inhaltsfreie Hinweise/Tray-Zähler im Hintergrund und bei Sperren prüfen.
8. Gruppenfenster/Editor bei 720×540 und 100/125/150/200 % DPI prüfen. Lange Namen,
   unbekannte Identitäten, volle Warteschlange, Speicherfehler und deaktivierten
   Adapter verständlich darstellen. Keine simulierten Live-Verbindungen verwenden.

Ergebnisse mit Commit, Windows-/Adapterversion, Zeit und Screenshots protokollieren.
Automatische In-Memory-, Actor- und UI-Tests ersetzen diesen Funknachweis nicht.
