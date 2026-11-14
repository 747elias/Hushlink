; Compile only through scripts/package.ps1, which supplies verified payload paths.
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef NumericVersion
  #error NumericVersion is required
#endif
#ifndef PayloadDirectory
  #error PayloadDirectory is required
#endif
#ifndef OutputDirectory
  #error OutputDirectory is required
#endif
#ifndef AssetBase
  #error AssetBase is required
#endif
#define AppId "{36F98DE2-AD17-432E-AE2D-759CA74B8574}"
#define UninstallKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\" + AppId + "_is1"
#define RunKey "Software\Microsoft\Windows\CurrentVersion\Run"

[Setup]
AppId={{36F98DE2-AD17-432E-AE2D-759CA74B8574}
AppName=Hushlink
AppVersion={#AppVersion}
AppPublisher=Hushlink contributors
AppPublisherURL=https://github.com/747elias/Hushlink
AppSupportURL=https://github.com/747elias/Hushlink/issues
AppUpdatesURL=https://github.com/747elias/Hushlink/releases
DefaultDirName={localappdata}\Programs\Hushlink
DefaultGroupName=Hushlink
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
OutputDir={#OutputDirectory}
OutputBaseFilename={#AssetBase}-setup
SetupIconFile=..\assets\hushlink.ico
UninstallDisplayIcon={app}\Hushlink.exe
VersionInfoVersion={#NumericVersion}.0
VersionInfoDescription=Hushlink-Installation
VersionInfoProductName=Hushlink
VersionInfoProductVersion={#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE
CloseApplications=yes
CloseApplicationsFilter=Hushlink.exe
RestartApplications=yes
RestartIfNeededByRun=no
UsePreviousAppDir=yes
UsePreviousTasks=yes
Uninstallable=yes
UninstallDisplayName=Hushlink
AppComments=Verschlüsselter Bluetooth-Chat mit lokalem Tresor

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "autostart"; Description: "Hushlink mit Windows im Infobereich starten (Tresor bleibt gesperrt)"; Check: ShowStartupTask
Name: "desktopicon"; Description: "Desktop-Verknüpfung erstellen"; Flags: unchecked

[Files]
Source: "{#PayloadDirectory}\{#AssetBase}.exe"; DestDir: "{app}"; DestName: "Hushlink.exe"; Flags: ignoreversion
Source: "{#PayloadDirectory}\*.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDirectory}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDirectory}\BUILD-COMMIT.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDirectory}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#PayloadDirectory}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Hushlink"; Filename: "{app}\Hushlink.exe"
Name: "{group}\Kurzanleitung"; Filename: "{app}\KURZSTART.md"
Name: "{autodesktop}\Hushlink"; Filename: "{app}\Hushlink.exe"; Tasks: desktopicon

[Registry]
; Record cleanup even when autostart is enabled later in the application.
Root: HKCU; Subkey: "{#RunKey}"; ValueType: none; ValueName: "Hushlink"; Flags: dontcreatekey uninsdeletevalue
Root: HKCU; Subkey: "{#RunKey}"; ValueType: string; ValueName: "Hushlink"; ValueData: """{app}\Hushlink.exe"" --tray"; Check: EnableStartup

[Run]
Filename: "{app}\Hushlink.exe"; Description: "Hushlink öffnen"; Flags: nowait postinstall skipifsilent; Check: ShouldLaunch
; A checked update restarts locked in the tray. /NoLaunch is for deployment and CI.
Filename: "{app}\Hushlink.exe"; Parameters: "--tray"; Flags: nowait skipifnotsilent; Check: ShouldLaunch

[Code]
var
  ExistingInstallation: Boolean;
  ExistingStartup: Boolean;

function InitializeSetup(): Boolean;
var
  InstalledVersion: String;
  InstalledPacked, NewPacked: Int64;
begin
  ExistingInstallation := RegKeyExists(HKCU, '{#UninstallKey}');
  ExistingStartup := RegValueExists(HKCU, '{#RunKey}', 'Hushlink');
  Result := True;
  if RegQueryStringValue(HKCU, '{#UninstallKey}', 'DisplayVersion', InstalledVersion) and
     StrToVersion(InstalledVersion, InstalledPacked) and
     StrToVersion('{#NumericVersion}', NewPacked) then
  begin
    if ComparePackedVersion(InstalledPacked, NewPacked) > 0 then
    begin
      SuppressibleMsgBox('Eine neuere Hushlink-Version ist bereits installiert.', mbError, MB_OK, IDOK);
      Result := False;
    end;
  end;
end;

function ShowStartupTask(): Boolean;
begin
  Result := not ExistingInstallation;
end;

function EnableStartup(): Boolean;
begin
  if ExistingInstallation then
    Result := ExistingStartup
  else
    Result := WizardIsTaskSelected('autostart');
end;

function ShouldLaunch(): Boolean;
var
  Index: Integer;
begin
  Result := True;
  for Index := 1 to ParamCount do
    if CompareText(ParamStr(Index), '/NoLaunch') = 0 then
      Result := False;
end;

; Deliberately no [UninstallDelete]: user-selected vaults, bootstrap settings and
; other files not installed by this script must survive removal and updates.
