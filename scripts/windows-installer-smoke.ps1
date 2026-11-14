param(
    [Parameter(Mandatory=$true)][string]$Installer,
    [Parameter(Mandatory=$true)][string]$Application,
    [Parameter(Mandatory=$true)][string]$Version
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Version -notmatch '^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$') { throw 'Invalid version' }
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$applicationPath = (Resolve-Path -LiteralPath $Application).Path
$report = [IO.Path]::GetFullPath((Join-Path $pwd 'work/installer-smoke'))
New-Item -ItemType Directory -Force $report | Out-Null
$installDirectory = Join-Path $env:LOCALAPPDATA 'Programs/Hushlink'
$installedExe = Join-Path $installDirectory 'Hushlink.exe'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{36F98DE2-AD17-432E-AE2D-759CA74B8574}_is1'
$startMenuShortcut = Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs/Hushlink/Hushlink.lnk'
$expectedRun = '"' + $installedExe + '" --tray'
$checks = [Collections.Generic.List[string]]::new()
$processes = [Collections.Generic.List[Diagnostics.Process]]::new()
$installedFixture = Join-Path $installDirectory 'installer-smoke-unowned.vault'
$externalFixture = Join-Path $report 'synthetic chosen location.vault'
$fixture = [byte[]]::new(512)
[Security.Cryptography.RandomNumberGenerator]::Fill($fixture)
[IO.File]::WriteAllBytes($externalFixture, $fixture)
$fixtureHash = (Get-FileHash -LiteralPath $externalFixture -Algorithm SHA256).Hash

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class HushlinkInstallerWindows {
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr handle);
}
'@

function Get-StartupValue {
    if (-not (Test-Path -LiteralPath $runKey)) { return $null }
    $properties = Get-ItemProperty -LiteralPath $runKey
    $entry = $properties.PSObject.Properties['Hushlink']
    if ($null -eq $entry) { return $null }
    return $entry.Value
}

function Invoke-Setup([bool]$NoLaunch, [string]$LogName) {
    $arguments = @('/SP-', '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CLOSEAPPLICATIONS', '/RESTARTAPPLICATIONS', "/LOG=`"$(Join-Path $report $LogName)`"")
    if ($NoLaunch) { $arguments += '/NoLaunch' }
    $setup = Start-Process -FilePath $installerPath -ArgumentList $arguments -PassThru
    if (-not $setup.WaitForExit(120000)) {
        Stop-Process -Id $setup.Id -ErrorAction SilentlyContinue
        throw 'Silent installation timed out.'
    }
    if ($setup.ExitCode -ne 0) { throw "Silent installation failed: $($setup.ExitCode)" }
}

function Stop-TestProcess([Diagnostics.Process]$Process) {
    $Process.Refresh()
    if (-not $Process.HasExited) {
        # Only synthetic, locked CI sessions are killed; no user profile is open.
        Stop-Process -Id $Process.Id
        $Process.WaitForExit(10000) | Out-Null
    }
}

function Test-AppStartup([bool]$Tray) {
    $arguments = @()
    if ($Tray) { $arguments += '--tray' }
    $start = @{FilePath=$installedExe; PassThru=$true}
    if ($arguments.Count -gt 0) { $start.ArgumentList = $arguments }
    $app = Start-Process @start
    $processes.Add($app)
    Start-Sleep -Seconds 8
    $app.Refresh()
    if ($app.HasExited) { throw "Installed app exited during startup: $($app.ExitCode)" }
    $visible = $app.MainWindowHandle -ne 0 -and [HushlinkInstallerWindows]::IsWindowVisible($app.MainWindowHandle)
    if ($Tray -and $visible) { throw '--tray unexpectedly displayed the main window.' }
    if (-not $Tray -and -not $visible) { throw 'Installed app did not expose a visible main window.' }
    Stop-TestProcess $app
}

function Assert-Fixtures {
    foreach ($path in @($externalFixture, $installedFixture)) {
        if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $fixtureHash) {
            throw 'Installation or removal changed a synthetic user-owned vault fixture.'
        }
    }
}

function Invoke-Uninstall {
    $uninstallers = @(Get-ChildItem -LiteralPath $installDirectory -Filter 'unins*.exe' -File)
    if ($uninstallers.Count -ne 1) { throw 'Expected one per-user uninstaller.' }
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$(Join-Path $report 'uninstall.log')`"")
    $uninstall = Start-Process -FilePath $uninstallers[0].FullName -ArgumentList $arguments -PassThru
    if (-not $uninstall.WaitForExit(120000)) { throw 'Silent uninstallation timed out.' }
    if ($uninstall.ExitCode -ne 0) { throw "Uninstallation failed: $($uninstall.ExitCode)" }
}

# Hosted runners are disposable. Refuse to touch an existing personal installation.
if ((Test-Path -LiteralPath $installDirectory) -or (Test-Path -LiteralPath $uninstallKey) -or $null -ne (Get-StartupValue)) {
    throw 'Installer smoke requires a clean Windows account; a Hushlink installation or startup entry already exists.'
}

try {
    Invoke-Setup $true 'install.log'
    if (-not (Test-Path -LiteralPath $installedExe)) { throw 'Installer did not create Hushlink.exe.' }
    if ((Get-FileHash -LiteralPath $installedExe).Hash -cne (Get-FileHash -LiteralPath $applicationPath).Hash) {
        throw 'Installed EXE differs from the checked standalone EXE.'
    }
    if ((Get-StartupValue) -cne $expectedRun) { throw 'Autostart does not quote the installed EXE and --tray correctly.' }
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.DisplayVersion -cne $Version) { throw 'Installed version registration differs from the release.' }
    if ($registration.InstallLocation.TrimEnd('\') -cne $installDirectory.TrimEnd('\')) { throw 'Unexpected install location.' }
    if (-not (Test-Path -LiteralPath $startMenuShortcut)) { throw 'Start menu shortcut is missing.' }
    $shell = New-Object -ComObject WScript.Shell
    if ($shell.CreateShortcut($startMenuShortcut).TargetPath -ine $installedExe) { throw 'Start menu shortcut target is incorrect.' }
    $checks.Add('Per-user silent install, exact EXE hash, version, quoted autostart and Start menu shortcut')
    [IO.File]::WriteAllBytes($installedFixture, $fixture)
    Test-AppStartup $false
    Test-AppStartup $true
    $checks.Add('Installed EXE remained running with a visible window and separately hidden with --tray')

    Remove-ItemProperty -LiteralPath $runKey -Name Hushlink
    Invoke-Setup $true 'update-disabled-autostart.log'
    if ($null -ne (Get-StartupValue)) { throw 'Update re-enabled explicitly disabled autostart.' }
    Assert-Fixtures
    $checks.Add('Repeated installation preserves disabled autostart and synthetic user files')

    New-ItemProperty -LiteralPath $runKey -Name Hushlink -Value $expectedRun -PropertyType String -Force | Out-Null
    Invoke-Setup $false 'update-restart.log'
    Start-Sleep -Seconds 8
    $restarted = @(Get-Process -Name Hushlink -ErrorAction SilentlyContinue | Where-Object { $_.Path -ieq $installedExe })
    foreach ($app in $restarted) { $processes.Add($app) }
    if ($restarted.Count -ne 1) { throw 'Silent update did not restart exactly one installed Hushlink process.' }
    $restarted[0].Refresh()
    if ($restarted[0].HasExited) { throw 'Hushlink exited after the silent update.' }
    if ($restarted[0].MainWindowHandle -ne 0 -and [HushlinkInstallerWindows]::IsWindowVisible($restarted[0].MainWindowHandle)) {
        throw 'Silent update restarted with an unexpected visible window.'
    }
    if ((Get-StartupValue) -cne $expectedRun) { throw 'Update changed enabled autostart incorrectly.' }
    Stop-TestProcess $restarted[0]
    Assert-Fixtures
    $checks.Add('Silent update preserved enabled autostart and restarted the installed app hidden in tray mode')

    Invoke-Uninstall
    if ((Test-Path -LiteralPath $installedExe) -or (Test-Path -LiteralPath $uninstallKey) -or $null -ne (Get-StartupValue)) {
        throw 'Uninstall left the app, uninstall registration or autostart entry behind.'
    }
    if (Test-Path -LiteralPath $startMenuShortcut) { throw 'Uninstall left the Start menu shortcut behind.' }
    Assert-Fixtures
    $checks.Add('Uninstall removed executable, registration, startup and shortcut while preserving user-owned vault fixtures')
    $evidence = @{commit=$env:GITHUB_SHA; version=$Version; checks=@($checks); hardware='No physical Bluetooth or Windows session-lock test'; fixture='Cryptographically random synthetic bytes only'}
    $evidence | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $report 'verification.json') -Encoding utf8NoBOM
    $checks | ForEach-Object { Write-Host $_ }
} catch {
    [IO.File]::WriteAllText((Join-Path $report 'failure.txt'), $_.Exception.ToString(), [Text.UTF8Encoding]::new($false))
    throw
} finally {
    foreach ($app in $processes) { Stop-TestProcess $app }
    if (Test-Path -LiteralPath $uninstallKey) {
        try { Invoke-Uninstall } catch { Write-Warning "Cleanup uninstall failed: $($_.Exception.Message)" }
    }
    # The only removals here are fixtures created above, never discovered profiles.
    if (Test-Path -LiteralPath $installedFixture) { Remove-Item -LiteralPath $installedFixture -Force }
    if ((Test-Path -LiteralPath $installDirectory) -and @(Get-ChildItem -LiteralPath $installDirectory -Force).Count -eq 0) {
        Remove-Item -LiteralPath $installDirectory
    }
    [Array]::Clear($fixture, 0, $fixture.Length)
}
