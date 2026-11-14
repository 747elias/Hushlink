# Hushlink v0.2.0

1. Bestand und öffentlichen Release-Zugriff prüfen; bestehenden v0.1.0-Release unverändert lassen.
2. Authentifizierten Dateiversand mit ausdrücklicher Annahme, begrenzten Blöcken und verschlüsselter Speicherung im gemeinsamen Tresor ergänzen.
3. Tresorwahl und sichere Verlagerung sowie Migration vorhandener Profile implementieren.
4. Windows-Tray, Autostart, per-user Installer und öffentliche Update-Prüfung/-Downloads ergänzen. Installation/Update verändert keine Tresordaten.
5. Bestehende Sicherheitsprüfungen erweitern; native Windows-App und Installer testen. Regelmässig committen/pushen.
6. v0.2.0 über die bestehende exakte Release-Prüfschranke veröffentlichen und alle tatsächlichen Downloads prüfen.

Der Chat- und Dateitransport bleibt ausschliesslich Bluetooth. Update-Prüfungen benötigen sichtbar und abschaltbar Internet. Der Repository-Inhaber hat das bestehende Repository öffentlich gemacht; anonymer Zugriff auf die bisherigen Release-Metadaten wurde geprüft.

Alle privaten Profil-, Kontakt-, Nachrichten- und Anhangsdaten liegen in einer verschlüsselten Tresordatei. Ein minimaler Startverweis auf den gewählten Pfad und die leere Betriebssystemsperrdatei enthalten keine Chatdaten.
