param(
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Commit,
    [string]$Compiler = 'work/inno-setup/ISCC.exe'
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
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
if (Test-Path -LiteralPath 'package-verification') { Remove-Item -LiteralPath 'package-verification' -Recurse -Force }
foreach ($existing in (Get-ChildItem -LiteralPath release-assets -File)) {
    if ($existing.Name -notin @("$baseName.exe", "$baseName-setup.exe", "$baseName.zip", 'SHA256SUMS.txt')) {
        throw 'release-assets contains an unexpected file. Use a fresh generated output directory.'
    }
    Remove-Item -LiteralPath $existing.FullName -Force
}
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
$compilerPath = (Resolve-Path -LiteralPath $Compiler).Path
if ((Get-Item -LiteralPath $compilerPath).VersionInfo.ProductVersion -notmatch '^7\.1\.1(?:\.0)?$') {
    throw 'The release requires the pinned Inno Setup 7.1.1 compiler. Run installer/install-compiler.ps1.'
}
$payloadDirectory = (Resolve-Path -LiteralPath $stage).Path
$outputDirectory = (Resolve-Path -LiteralPath release-assets).Path
$numericVersion = [regex]::Match($packageVersion, '^\d+\.\d+\.\d+').Value
& $compilerPath "/DAppVersion=$packageVersion" "/DNumericVersion=$numericVersion" "/DPayloadDirectory=$payloadDirectory" "/DOutputDirectory=$outputDirectory" "/DAssetBase=$baseName" installer/Hushlink.iss
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
python scripts/check-pe.py "release-assets/$baseName-setup.exe" --installer
if ($LASTEXITCODE -ne 0) { throw 'Installer PE validation failed' }
Copy-Item "release-assets/$baseName-setup.exe" "$stage/$baseName-setup.exe"
Compress-Archive -Path "$stage/*" -DestinationPath "release-assets/$baseName.zip" -CompressionLevel Optimal
$lines = @()
foreach ($name in @("$baseName.exe", "$baseName-setup.exe", "$baseName.zip")) {
    $hash = (Get-FileHash "release-assets/$name" -Algorithm SHA256).Hash.ToLowerInvariant()
    $lines += "$hash  $name"
}
[IO.File]::WriteAllText("$pwd/release-assets/SHA256SUMS.txt", ($lines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
# Independently re-read final hashes, PE containers, ZIP binaries and commit.
python scripts/verify-package.py --directory release-assets --tag $Version --commit $Commit
if ($LASTEXITCODE -ne 0) { throw 'Release asset verification failed' }
