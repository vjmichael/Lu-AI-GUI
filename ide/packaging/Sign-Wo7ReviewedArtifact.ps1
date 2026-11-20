# WO7-C explicit signing step. No self-generated keys, automatic downloads,
# silent certificate selection, publication, or release promotion.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidateSet('Service','Desktop','Installer')][string]$Kind,
  [Parameter(Mandatory=$true)][string]$Artifact,
  [Parameter(Mandatory=$true)][string]$SignTool,
  [Parameter(Mandatory=$true)][string]$SignToolSha256,
  [Parameter(Mandatory=$true)][string]$SignerThumbprint,
  [Parameter(Mandatory=$true)][string]$TimestampUrl
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
function Deny([string]$reason) { throw "DENIED: WO7 signing: $reason" }
if ($env:OS -ne 'Windows_NT') { Deny 'native_windows_required' }
if ($SignerThumbprint -notmatch '^[a-fA-F0-9]{40}$' -or
    $SignToolSha256 -notmatch '^[a-fA-F0-9]{64}$') { Deny 'pinned_identity_or_tool_missing' }
if ($TimestampUrl -notin @('https://timestamp.digicert.com','http://timestamp.digicert.com')) {
  Deny 'unreviewed_timestamp_authority'
}
$path=(Resolve-Path -LiteralPath $Artifact).Path
$tool=(Resolve-Path -LiteralPath $SignTool).Path
if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) {
  Deny 'linked_artifact_denied'
}
if ((Get-Item -LiteralPath $tool).Attributes -band [IO.FileAttributes]::ReparsePoint) {
  Deny 'linked_signing_tool_denied'
}
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $tool).Hash -ine $SignToolSha256) {
  Deny 'signing_tool_digest_mismatch'
}
$basename=[IO.Path]::GetFileName($path)
switch ($Kind) {
  'Service' { if ($basename -ine 'windows_recovery_service.exe') { Deny 'not_fixed_service' } }
  'Desktop' { if ($basename -ine 'lu-ide-desktop.exe') { Deny 'not_fixed_desktop' } }
  'Installer' { if ($basename -notmatch '^Lu IDE.*setup\.exe$') { Deny 'not_fixed_nsis_installer' } }
}
$certificate=Get-ChildItem Cert:\CurrentUser\My,Cert:\LocalMachine\My |
  Where-Object { $_.Thumbprint -ieq $SignerThumbprint -and $_.HasPrivateKey } |
  Select-Object -First 1
if (!$certificate) { Deny 'reviewed_real_signing_certificate_unavailable' }
$prior=Get-AuthenticodeSignature -LiteralPath $path
if ($prior.Status -eq 'Valid') { Deny 'already_signed_release_asset_refuse_mutation' }
& $tool sign /fd SHA256 /sha1 $SignerThumbprint /tr $TimestampUrl /td SHA256 $path
if ($LASTEXITCODE -ne 0) { Deny 'authenticode_signing_command_failed' }
$post=Get-AuthenticodeSignature -LiteralPath $path
if ($post.Status -ne 'Valid' -or !$post.SignerCertificate -or
    $post.SignerCertificate.Thumbprint -ine $SignerThumbprint) {
  Deny 'signed_artifact_postverification_failed'
}
$record=[ordered]@{
  schema='lu.ide.wo7.signed-asset.v1'
  role=$Kind
  sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
  signerThumbprint=$post.SignerCertificate.Thumbprint.ToLowerInvariant()
  authenticode='Valid'
  releaseAccepted=$false
}
$record | ConvertTo-Json -Compress
