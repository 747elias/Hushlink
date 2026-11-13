param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
New-Item -ItemType Directory -Force smoke-report | Out-Null
$exePath = (Resolve-Path $Executable).Path
$process = Start-Process -FilePath $exePath -PassThru -RedirectStandardError "$pwd/smoke-report/stderr.txt" -RedirectStandardOutput "$pwd/smoke-report/stdout.txt"
try {
    Start-Sleep -Seconds 8
    $process.Refresh()
    if ($process.HasExited) {
        throw "Hushlink exited during startup (code $($process.ExitCode)). Check desktop/Direct3D availability and runtime dependencies."
    }
    if ($process.MainWindowHandle -eq 0) {
        Write-Warning 'The hosted desktop does not expose a main window handle; screenshot evidence must be assessed separately.'
    }
    [IO.File]::WriteAllText("$pwd/smoke-report/startup.txt", "Native Windows executable remained running for 8 seconds. Window handle: $($process.MainWindowHandle).`nCommit: $env:GITHUB_SHA`nThis is a startup smoke test, not an interactive UI or Bluetooth test.`n", [Text.UTF8Encoding]::new($false))
    try {
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        $bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bitmap = New-Object Drawing.Bitmap $bounds.Width, $bounds.Height
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
            $bitmap.Save("$pwd/smoke-report/startup.png", [Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
    } catch {
        Write-Warning "Screen capture is unavailable on this hosted desktop: $($_.Exception.Message)"
    }
} catch {
    [IO.File]::WriteAllText("$pwd/smoke-report/failure.txt", $_.Exception.Message + "`n", [Text.UTF8Encoding]::new($false))
    throw
} finally {
    $process.Refresh()
    if (-not $process.HasExited) {
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(5000)) { Stop-Process -Id $process.Id }
    }
}
