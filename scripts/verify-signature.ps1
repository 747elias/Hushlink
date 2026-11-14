param(
    [Parameter(Mandatory=$true)][string]$Path,
    [Parameter(Mandatory=$true)][string]$CertificateSha256
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($CertificateSha256 -notmatch '^[0-9a-fA-F]{64}$') { throw 'Expected publisher certificate SHA-256 is not configured' }
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or $signature.SignatureType -ne 'Authenticode') { throw 'Windows did not validate an embedded Authenticode signature' }
if ($null -eq $signature.SignerCertificate -or $null -eq $signature.TimeStamperCertificate) { throw 'Publisher certificate or trusted timestamp is missing' }
$actual = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($signature.SignerCertificate.RawData))
if ($actual -ine $CertificateSha256) { throw 'Unexpected publisher certificate' }
$usage = @($signature.SignerCertificate.Extensions | Where-Object { $_.Oid.Value -eq "2.5.29.37" })
$eku = @($usage | ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
if ('1.3.6.1.5.5.7.3.3' -notin $eku) { throw 'Certificate does not authorize code signing' }
$tools = @(Get-ChildItem -Path "${env:ProgramFiles(x86)}/Windows Kits/10/bin/*/x64/signtool.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending)
if ($tools.Count -eq 0) { throw 'Windows SDK signtool was not found' }
& $tools[0].FullName verify /pa /all /tw /v $Path
if ($LASTEXITCODE -ne 0) { throw 'signtool verification failed' }
Write-Output "Verified trusted publisher signature and timestamp: $([IO.Path]::GetFileName($Path))"
