# Windows CI/release staging for Lu's read-only OpenAlex Research provider.
# Uses the trusted Windows System32 curl, never PATH, a downloaded binary,
# Docker, user-picked tools, or a model-provided command.
[CmdletBinding()]
param([switch]$BuildInstaller)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if(-not $IsWindows -and $env:OS -ne 'Windows_NT'){
  throw 'This research client staging path supports Windows only'
}
$root=Split-Path -Parent $PSScriptRoot
$source=Join-Path $env:SystemRoot 'System32\curl.exe'
$target=Join-Path $root 'lu-desktop/src-tauri/resources/lu-research/curl.exe'
$overlay=Join-Path $root 'lu-desktop/src-tauri/tauri.research.conf.json'
if(-not (Test-Path -LiteralPath $source -PathType Leaf)){
  throw 'Windows System32 curl.exe is missing'
}
$signature=Get-AuthenticodeSignature -LiteralPath $source
if($signature.Status -ne 'Valid' -or
   $signature.SignerCertificate.Subject -notmatch 'Microsoft'){
  throw 'System32 curl.exe lacks a verified Microsoft publisher signature'
}
$checksum=(Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash.ToLowerInvariant()
if($checksum -notmatch '^[0-9a-f]{64}$'){
  throw 'Invalid Windows Research client digest'
}
New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force|Out-Null
Copy-Item -LiteralPath $source -Destination $target -Force
$staged=(Get-FileHash -Algorithm SHA256 -LiteralPath $target).Hash.ToLowerInvariant()
if($checksum -cne $staged){throw 'Staged Research client hash mismatch'}
$env:LU_RESEARCH_CURL_SHA256=$checksum
if($env:GITHUB_ENV){
  Add-Content -LiteralPath $env:GITHUB_ENV -Value "LU_RESEARCH_CURL_SHA256=$checksum"
}
Set-Content -LiteralPath (Join-Path $root 'lu-research-curl.sha256') -Value $checksum -NoNewline
Write-Output "Lu Research client staged with reviewed builder-origin: $checksum"
if($BuildInstaller){
  $tauri=Join-Path $root 'lu-desktop/src-tauri/Cargo.toml'
  & cargo tauri build --ci --config $overlay --manifest-path $tauri
  if($LASTEXITCODE -ne 0){throw 'Lu Research installer build failed'}
}
