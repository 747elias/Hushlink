param(
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Commit
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Version -notmatch '^v\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$') { throw 'Invalid version tag' }
if ($Commit -notmatch '^[0-9a-f]{40}$') { throw 'Invalid commit SHA' }
$cargo = Get-Content Cargo.toml -Raw
$packageVersion = [regex]::Match($cargo, '(?m)^version = "([^\"]+)"\r?$').Groups[1].Value
if ($Version -cne "v$packageVersion") { throw 'Tag does not match Cargo.toml version' }
$builtCommit = (Get-Content build-artifact/build-commit.txt -Raw).Trim()
if ($builtCommit -cne $Commit) { throw 'Build artifact was not produced for this commit' }
python scripts/check-notices.py
if ($LASTEXITCODE -ne 0) { throw 'Dependency notices are stale' }
python scripts/check-pe.py build-artifact/hushlink.exe
if ($LASTEXITCODE -ne 0) { throw 'PE validation failed' }
$baseName = "Hushlink-$Version-windows-x64"
New-Item -ItemType Directory -Force release-assets | Out-Null
$stage = "package-stage/$baseName"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item build-artifact/hushlink.exe "release-assets/$baseName.exe"
Copy-Item build-artifact/hushlink.exe "$stage/$baseName.exe"
Copy-Item docs/QUICKSTART.md "$stage/KURZSTART.md"
Copy-Item README.md, SECURITY.md, LICENSE, CHANGELOG.md, AGENTS.md, THIRD_PARTY_NOTICES.md $stage
Copy-Item licenses "$stage/licenses" -Recurse
# Preserve relative README links and include the two-PC hardware test procedure.
Copy-Item docs "$stage/docs" -Recurse
Copy-Item "docs/RELEASE-$Version.md" "$stage/RELEASE-NOTES.md"
[IO.File]::WriteAllText("$pwd/$stage/BUILD-COMMIT.txt", "$Commit`n", [Text.UTF8Encoding]::new($false))
Compress-Archive -Path "$stage/*" -DestinationPath "release-assets/$baseName.zip" -CompressionLevel Optimal
$lines = @()
foreach ($name in @("$baseName.exe", "$baseName.zip")) {
    $hash = (Get-FileHash "release-assets/$name" -Algorithm SHA256).Hash.ToLowerInvariant()
    $lines += "$hash  $name"
}
[IO.File]::WriteAllText("$pwd/release-assets/SHA256SUMS.txt", ($lines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
# Independently re-read and verify the final assets, including ZIP's EXE.
foreach ($line in (Get-Content release-assets/SHA256SUMS.txt)) {
    $parts = $line -split '  ', 2
    if ((Get-FileHash "release-assets/$($parts[1])" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $parts[0]) { throw 'Checksum verification failed' }
}
Expand-Archive "release-assets/$baseName.zip" -DestinationPath package-verification
if ((Get-FileHash "package-verification/$baseName.exe").Hash -cne (Get-FileHash "release-assets/$baseName.exe").Hash) { throw 'ZIP binary differs from standalone EXE' }
Get-Content release-assets/SHA256SUMS.txt
