param([string]$Destination = 'work/inno-setup')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Official upstream release and GitHub's SHA-256 asset digest, checked 2026-11-14.
# Updating either value requires reviewing the upstream release and license.
$version = '7.1.1'
$url = 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe'
$expectedHash = '8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0'
$destinationPath = [IO.Path]::GetFullPath((Join-Path $pwd $Destination))
$downloadDirectory = [IO.Path]::GetFullPath((Join-Path $pwd 'work/installer-tools'))
New-Item -ItemType Directory -Force $downloadDirectory | Out-Null
$download = Join-Path $downloadDirectory "innosetup-$version-x64.exe"
if (-not (Test-Path -LiteralPath $download)) {
    Invoke-WebRequest -Uri $url -OutFile $download
}
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedHash) {
    throw 'The pinned Inno Setup compiler download has an unexpected SHA-256. Refusing to execute it.'
}
# This is a build-time dependency only. Hushlink's installed app needs no compiler,
# .NET, App SDK, MSIX identity, administrator rights or separately installed CRT.
$arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$destinationPath`"")
$process = Start-Process -FilePath $download -ArgumentList $arguments -PassThru
if (-not $process.WaitForExit(120000)) {
    Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
    throw 'Timed out installing the pinned Inno Setup compiler.'
}
if ($process.ExitCode -ne 0) { throw "Compiler installation failed with exit code $($process.ExitCode)" }
$compiler = Join-Path $destinationPath 'ISCC.exe'
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) { throw 'ISCC.exe was not installed.' }
$actualVersion = (Get-Item -LiteralPath $compiler).VersionInfo.ProductVersion
if ($actualVersion -notmatch '^7\.1\.1(?:\.0)?$') { throw "Unexpected compiler product version: $actualVersion" }
Write-Host "Verified and installed Inno Setup $version ($expectedHash)."
