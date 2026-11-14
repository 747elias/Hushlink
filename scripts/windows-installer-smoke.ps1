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
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class HushlinkInstallerWindows {
    private delegate bool WindowCallback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")]
    private static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);
    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    private static extern int GetClassName(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr handle);
    public static IntPtr[] MainWindows(int process) {
        var result = new List<IntPtr>();
        EnumWindows((window, parameter) => {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            var title = new StringBuilder(512);
            GetWindowText(window, title, title.Capacity);
            if (owner == process && title.ToString() == "Hushlink") result.Add(window);
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    public static string[] DescribeWindows(int process) {
        var result = new List<string>();
        EnumWindows((window, parameter) => {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == process) {
                var title = new StringBuilder(512);
                var name = new StringBuilder(256);
                GetWindowText(window, title, title.Capacity);
                GetClassName(window, name, name.Capacity);
                result.Add($"HWND={window}; visible={IsWindowVisible(window)}; class={name}; title={title}");
            }
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
}
'@

function Get-StartupValue {
    if (-not (Test-Path -LiteralPath $runKey)) { return $null }
    $properties = Get-ItemProperty -LiteralPath $runKey
    $entry = $properties.PSObject.Properties['Hushlink']
    if ($null -eq $entry) { return $null }
    return $entry.Value
}

function Invoke-Setup([bool]$NoLaunch, [string]$LogName, [int]$ParentProcessId = 0) {
    $arguments = @('/SP-', '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CLOSEAPPLICATIONS', '/RESTARTAPPLICATIONS', "/LOG=`"$(Join-Path $report $LogName)`"")
    if ($NoLaunch) { $arguments += '/NoLaunch' }
    if ($ParentProcessId -gt 0) { $arguments += "/HUSHLINKPARENTPID=$ParentProcessId" }
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
    $label = if ($Tray) { 'tray' } else { 'visible' }
    $start = @{FilePath=$installedExe; PassThru=$true; RedirectStandardOutput=(Join-Path $report "$label-stdout.txt"); RedirectStandardError=(Join-Path $report "$label-stderr.txt")}
    if ($arguments.Count -gt 0) { $start.ArgumentList = $arguments }
    $app = Start-Process @start
    $processes.Add($app)
    Start-Sleep -Seconds 8
    $app.Refresh()
    if ($app.HasExited) { throw "Installed app exited during startup: $($app.ExitCode)" }
    [IO.File]::WriteAllLines((Join-Path $report "$label-windows.txt"), [HushlinkInstallerWindows]::DescribeWindows($app.Id))
    $windows = @([HushlinkInstallerWindows]::MainWindows($app.Id))
    if ($windows.Count -ne 1) { throw 'Installed app must own exactly one Hushlink main window.' }
    $visible = [HushlinkInstallerWindows]::IsWindowVisible($windows[0])
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

    # Exercise the actual updater ordering: setup starts while the existing app
    # still holds Local\Hushlink, then waits for that parent before AppMutex.
    $parent = Start-Process -FilePath $installedExe -ArgumentList '--tray' -PassThru
    $processes.Add($parent)
    Start-Sleep -Seconds 4
    $parent.Refresh()
    if ($parent.HasExited) { throw 'Update handoff parent exited early.' }
    $closer = Start-Job -ArgumentList $parent.Id -ScriptBlock {
        param($AppProcessId)
        Start-Sleep -Seconds 5
        Stop-Process -Id $AppProcessId -ErrorAction Stop
    }
    try {
        Invoke-Setup $true 'update-live-parent.log' $parent.Id
        Receive-Job $closer -Wait -ErrorAction Stop | Out-Null
    } finally { Remove-Job $closer -Force }
    Assert-Fixtures
    $checks.Add('Updater installer waits for the live app parent before checking its instance mutex')

    New-ItemProperty -LiteralPath $runKey -Name Hushlink -Value $expectedRun -PropertyType String -Force | Out-Null
    Invoke-Setup $false 'update-restart.log'
    Start-Sleep -Seconds 8
    $restarted = @(Get-Process -Name Hushlink -ErrorAction SilentlyContinue | Where-Object { $_.Path -ieq $installedExe })
    foreach ($app in $restarted) { $processes.Add($app) }
    if ($restarted.Count -ne 1) { throw 'Silent update did not restart exactly one installed Hushlink process.' }
    $restarted[0].Refresh()
    if ($restarted[0].HasExited) { throw 'Hushlink exited after the silent update.' }
    [IO.File]::WriteAllLines((Join-Path $report 'restart-windows.txt'), [HushlinkInstallerWindows]::DescribeWindows($restarted[0].Id))
    $restartWindows = @([HushlinkInstallerWindows]::MainWindows($restarted[0].Id))
    if ($restartWindows.Count -ne 1) { throw 'Restarted app must own exactly one Hushlink main window.' }
    if ([HushlinkInstallerWindows]::IsWindowVisible($restartWindows[0])) {
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
    try {
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        $bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bitmap = [Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
            $bitmap.Save((Join-Path $report 'failure.png'), [Drawing.Imaging.ImageFormat]::Png)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    } catch { Write-Warning 'Installer failure screenshot unavailable.' }
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
