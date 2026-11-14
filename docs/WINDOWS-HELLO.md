# Optionaler Windows-Hello-Zugang

Die Funktion ist implementiert und benötigt noch einen Test auf einem physischen
Windows-11-PC mit eingerichtetem Windows Hello. Sie ist keine reine zusätzliche
Passwortmaske: Der zufällige Tresorschlüssel wird über Windows CNG mit einem
separaten, persistenten RSA-Schlüssel im **Microsoft Passport Key Storage Provider**
geschützt. Dieser Schlüssel ist nicht exportierbar und an das Windows-Benutzerkonto
gebunden. Ohne funktionierenden privaten Schlüssel kann die App den Hello-Zugang
nicht zum Entschlüsseln verwenden. Der Passwort-Schlüsselplatz bleibt erhalten.

Zum Einrichten im entsperrten Tresor unter Einstellungen das aktuelle Passwort
angeben und **Windows Hello einrichten** wählen. Die App verlangt einen
2048-Bit-Schlüssel, ausschliessliche Entschlüsselungsnutzung, Exportpolicy 0 und
`NgcCacheType = AUTH_MANDATORY (1)`. Für jeden privaten Zugriff setzt sie
`PinCacheIsGestureRequired = 1`. Ein stiller Probezugriff muss scheitern;
ein anschliessender interaktiver Zugriff muss denselben Tresorschlüssel liefern.
Erst danach wird der zusätzliche Schlüsselplatz atomar gespeichert. Kein
Software-KSP-, DPAPI- oder blosses `UserConsentVerifier`-Fallback.

Die Verschlüsselung übernimmt `NCryptEncrypt/NCryptDecrypt` mit RSA-OAEP/SHA-256.
Es werden keine eigenen RSA-/Padding-Algorithmen implementiert. Manche Passport-
Provider unterstützen diese Kombination oder die strenge Cache-Policy nicht;
dann bleibt die Einrichtung ausdrücklich nicht verfügbar. Die NGC-Cache-Property
ist eine providerspezifische, nicht in den öffentlichen SDK-Konstanten garantierte
Eigenschaft. Ein nicht bestätigbarer Schutz wird abgewiesen. Aus erfolgreicher
Kompilierung folgt keine Behauptung der Unterstützung auf jedem Hello-Gerät.

Der öffentliche Header enthält nur eine zufällige Schlüsselkennung und den
verschlüsselten Tresorschlüssel. Der Benutzer-SID wird beim Zugriff lokal bestimmt
und nicht in den Tresor geschrieben. Der private CNG-Schlüssel wird nie exportiert.
Die gesamte Erweiterung ist zusätzlich an die Inhalts-AEAD gebunden; Manipulation
führt zum Fehler. Bei einer Sperre werden Generationen sofort ungültig; ein
zurückkehrender Hello-Zugriff darf keinen gesperrten UI-Zustand wieder entsperren.

**Passwort und Sicherungen:** Auf einem anderen PC oder Windows-Konto ist der
private Hello-Schlüssel nicht vorhanden. Zum Öffnen einer Kopie das zum damaligen
Zeitpunkt gültige Passwort verwenden. Eine Passwortänderung schützt den vorhandenen
unabhängigen Tresorschlüssel neu; sie deaktiviert Hello nicht automatisch. Dafür
**Windows Hello deaktivieren** mit aktuellem Passwort verwenden. Erst wird der
Hello-Slot aus dem Tresor entfernt, dann wird versucht, den eigenen CNG-Schlüssel
zu löschen. Fehlschlag dieser OS-Bereinigung wird gemeldet. Ältere Sicherungen
können frühere Passwort-/Hello-Slots enthalten. Verliert man beide Zugangswege,
kann der Entwickler die Daten nicht wiederherstellen.

Keine Erhöhung von Rechten, MSIX-Identität oder App-SDK-Laufzeit. Ein
kompromittiertes/entsperrtes Windows-Benutzerkonto bleibt ausserhalb des Schutzes.
Hello ersetzt keine externe Sicherheitsprüfung und keine Geräteverschlüsselung.

Offizielle API-Grundlagen, geprüft am 14.11.2026:

- https://learn.microsoft.com/windows/uwp/security/microsoft-passport
- https://learn.microsoft.com/windows/security/identity-protection/hello-for-business/hello-how-it-works
- https://learn.microsoft.com/windows/win32/api/ncrypt/nf-ncrypt-ncryptcreatepersistedkey
- https://learn.microsoft.com/windows/win32/api/ncrypt/nf-ncrypt-ncryptdecrypt
- https://learn.microsoft.com/windows/win32/seccng/key-storage-property-identifiers
