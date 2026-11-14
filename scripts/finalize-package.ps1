param([Parameter(Mandatory=$true)][string]$Version,[Parameter(Mandatory=$true)][string]$Commit)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Version -notmatch '^v\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$' -or $Commit -notmatch '^[0-9a-f]{40}$') { throw 'Invalid package identity' }
$baseName = "Hushlink-$Version-windows-x64"
$stage = "package-stage/$baseName"
if ((Get-Content "$stage/BUILD-COMMIT.txt" -Raw).Trim() -cne $Commit) { throw 'Stage commit mismatch' }
# Called after signing setup.exe. Do not rebuild the signed installer here.
Copy-Item "release-assets/$baseName.exe" "$stage/$baseName.exe" -Force
Copy-Item "release-assets/$baseName-setup.exe" "$stage/$baseName-setup.exe" -Force
if (Test-Path "release-assets/$baseName.zip") { Remove-Item "release-assets/$baseName.zip" -Force }
Compress-Archive -Path "$stage/*" -DestinationPath "release-assets/$baseName.zip" -CompressionLevel Optimal
$lines = @()
foreach ($name in @("$baseName.exe", "$baseName-setup.exe", "$baseName.zip")) {
    $lines += "$((Get-FileHash "release-assets/$name" -Algorithm SHA256).Hash.ToLowerInvariant())  $name"
}
[IO.File]::WriteAllText("$pwd/release-assets/SHA256SUMS.txt", ($lines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
python scripts/verify-package.py --directory release-assets --tag $Version --commit $Commit
if ($LASTEXITCODE -ne 0) { throw 'Final signed package verification failed' }
