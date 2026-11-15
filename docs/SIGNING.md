# Vertrauenswürdige Windows-Signatur

Status der vertrauenswürdigen Signierung: **extern blockiert**. Der Maintainer hat am 14.11.2026 bestätigt,
dass noch kein Code-Signing-Zertifikat und kein Signierdienst vorhanden sind.
Es wurde keine Signatur behauptet oder ein selbst ausgestelltes Zertifikat als
vertrauenswürdig ausgegeben. Die GitHub-Integration konnte die vorhandenen
Actions-Secrets/Variablen nicht auflisten (403); das ist keine Aussage über deren Inhalt.

## Vorbereiteter Weg: SignPath Foundation

Hushlink ist ein öffentliches MIT-Projekt unter
https://github.com/747elias/Hushlink mit bereits veröffentlichten Windows-Versionen.
Dafür kommt eine Bewerbung bei der [SignPath Foundation](https://signpath.org/)
in Betracht. Die Stiftung entscheidet über die Aufnahme. Kostenfreiheit und
Genehmigung sind nicht zugesichert. Der Maintainer muss die aktuellen
[Teilnahmebedingungen](https://signpath.org/terms) selbst prüfen, den Antrag
mit seiner tatsächlichen Identität stellen und die Herausgeberberechtigung erhalten.
Es wurde kein Antrag im Namen des Nutzers eingereicht und kein kostenpflichtiger
Dienst bestellt. Alternative: eigenes vertrauenswürdiges Zertifikat oder ein
verifizierter Signierdienst; dann den gleichen Prüfablauf an dessen API anpassen.

Vorbereitete Angaben für den Antrag:

- Projekt/Produkt: Hushlink; Lizenz: MIT; Konto: persönliches GitHub-Konto `747elias`.
- Quellcode: https://github.com/747elias/Hushlink
- Zweck: lokaler verschlüsselter Bluetooth-Chat mit Dateiversand für Windows 11 x64.
- Zu signieren: native Rust-App `hushlink.exe` und Inno-Setup-Installer `Hushlink-setup.exe`.
- Build: öffentliche GitHub Actions, `windows-2025`, gepinnte Rust-MSVC-Toolchain;
  Signieren nur aus dem geprüften Workflow für denselben Commit.
- Herausgebername, Land, Kontakt und rechtliche Erklärungen: vom Maintainer ausfüllen.

Nach Genehmigung in SignPath das konkrete GitHub-Repository als Trusted Build
System verbinden und Signierrechte auf den Release-/Qualifikationsworkflow und
autorisierte Versionstags beschränken. Die XML-Dateien
[app.xml](../signing/app.xml) und [setup.xml](../signing/setup.xml) als zwei
Artifact Configurations importieren; einen SHA-256- und Timestamp-fähigen
Produktions-Signing-Policy verwenden. Es gibt keinen generischen Signier-Endpunkt
in der App. Der Token gehört ausschliesslich in GitHub Actions Secrets.

Im Repository konfigurieren:

| Art | Name | Wert |
| --- | --- | --- |
| Secret | `SIGNPATH_API_TOKEN` | vom Dienst autorisierter CI-Token |
| Variable | `SIGNPATH_ORGANIZATION_ID` | SignPath-Konto-ID, keine GitHub-Organisation nötig |
| Variable | `SIGNPATH_PROJECT_SLUG` | Projektkennung |
| Variable | `SIGNPATH_SIGNING_POLICY_SLUG` | genehmigte Produktionsrichtlinie |
| Variable | `SIGNPATH_APP_ARTIFACT_SLUG` | Konfiguration für `hushlink.exe` |
| Variable | `SIGNPATH_SETUP_ARTIFACT_SLUG` | Konfiguration für `Hushlink-setup.exe` |
| Variable | `HUSHLINK_PUBLISHER_SHA256` | 64 Hex-Zeichen: SHA-256 des DER-Herausgeberzertifikats |

Keine privaten Schlüssel oder Passwörter in Chat, Quellcode, Artefakte oder Logs
kopieren. Keine selbst ausgestellten Testzertifikate für diese Variablen verwenden.
Die Kontokonfiguration kann mangels vorhandenen Dienstes noch nicht geprüft werden.

## Ablauf und Schranken

1. Normaler Push/PR: Format, Clippy, Tests, native App, Installer und Smoke-Prüfungen
   ohne Signierzugriff. Diese CI-Artefakte sind ausdrücklich Entwicklungsbuilds.
2. Nach Übernahme des Dispatch-Workflows in den GitHub-Standardbranch vor dem ersten Tag `Qualify signed Windows build` auf dem gewünschten Branch
   auslösen. Ohne vollständige Konfiguration scheitert der erste Schritt verständlich.
3. Der signierte Ablauf führt dieselben Prüfungen aus, signiert die App, prüft mit
   Windows Authenticode und SDK-signtool Zertifikatskette, Code-Signing-EKU,
   erwarteten Zertifikat-SHA-256 und Timestamp. Erst dann folgen App-Start und Packaging.
4. Den fertig kompilierten Installer separat signieren und erneut prüfen. ZIP und
   SHA256SUMS erst mit den finalen signierten Bytes erzeugen. Danach Installation,
   Start, Update-Handoff und Deinstallation an genau diesen Bytes prüfen.
5. Der Tag-Workflow wiederholt diese Schranken für seinen exakten Commit. Nur der
   davon abhängige Publish-Job hat `contents: write`. Ein Tag allein umgeht nichts.
6. Vor Veröffentlichung werden beide Authenticode-Signaturen erneut geprüft.
   Anschliessend die tatsächlichen Downloads auf Commit, SHA-256, ZIP-Identität
   und Windows-PE-Format prüfen; heruntergeladene EXEs zusätzlich unter Windows
   mit `scripts/verify-signature.ps1` prüfen.

Die SignPath-Action ist auf den offiziellen v2-Commit
`c92b958760219087e01f8d67a1669ed57afe2627` fixiert. Die Windows-App prüft vor dem
Start eines heruntergeladenen Installers zusätzlich die Windows-Vertrauenskette,
Timestamp und das beim Build eingebettete Zertifikat an ihrem gegen Schreiben
und Ersetzen gesperrten Dateihandle. Unkonfigurierte Entwicklungsbuilds starten
keinen automatisch heruntergeladenen Installer. Bei Zertifikatsrotation muss
zunächst ein unabhängig geprüfter neuer Installer manuell installiert werden;
ein blosses Ändern von Prüfsummen umgeht die Zertifikatsbindung nicht.

App und Setup werden signiert. Der von Inno generierte Deinstallationsstub erhält
in diesem Ablauf keine separate Signatur; das wird nicht behauptet. Signaturen
beweisen Herausgeberschaft und Dateiintegrität, keine externe Sicherheitsprüfung.
Eine neue gültige Signatur verhindert SmartScreen-Hinweise nicht garantiert.

Offizielle Grundlagen (geprüft 14.11.2026):

- https://docs.signpath.io/trusted-build-systems/github
- https://docs.signpath.io/artifact-configuration/reference
- https://github.com/SignPath/github-action-submit-signing-request/tree/c92b958760219087e01f8d67a1669ed57afe2627
- https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options
- https://learn.microsoft.com/windows/win32/api/wintrust/nf-wintrust-winverifytrust
- https://learn.microsoft.com/windows/win32/seccrypto/using-signtool-to-verify-a-file-signature

## Freigegebene Ausnahme v0.4.0

Am 15.11.2026 hat der Maintainer ausdrücklich die Veröffentlichung von v0.4.0
als **unsignierte Vorabversion** erlaubt. Der Release ist veröffentlicht und
seine tatsächlichen Downloads sind geprüft. Diese Ausnahme gilt nur für den
in AGENTS.md festgehaltenen Release-Commit und dessen exakte geprüfte Dateien.
Sie beschafft kein Zertifikat, erlaubt keine Vertrauensbehauptung und lockert
weder den Updater noch die Signierpflicht zukünftiger Releases.
