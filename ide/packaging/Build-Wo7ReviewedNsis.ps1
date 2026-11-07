# WO7-B/C explicit Windows reviewed payload -> Tauri NSIS -> real signing.
# Requires PRE-BUILT and PRE-SIGNED app/service binaries with separately
# approved exact source, manifest, license and import closure. No npm downloads.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$RepositoryRoot,
  [Parameter(Mandatory=$true)][string]$ApprovedInventory,
  [Parameter(Mandatory=$true)][string]$InputRoot,
  [Parameter(Mandatory=$true)][string]$NewPayloadRoot,
  [Parameter(Mandatory=$true)][string]$NewCandidateRoot,
  [Parameter(Mandatory=$true)][string]$SourceSha,
  [Parameter(Mandatory=$true)][string]$SignTool,
  [Parameter(Mandatory=$true)][string]$SignToolSha256,
  [Parameter(Mandatory=$true)][string]$InstallerSignerThumbprint,
  [Parameter(Mandatory=$true)][string]$TimestampUrl
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
function Deny([string]$stage) { throw "DENIED: WO7 NSIS build: $stage" }
if ($env:OS -ne 'Windows_NT') { Deny 'windows_required' }
if ($SourceSha -notmatch '^[0-9a-fA-F]{40}$') { Deny 'source_sha_missing' }
$repo=(Resolve-Path -LiteralPath $RepositoryRoot).Path
$head=(& git -C $repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $head -ine $SourceSha) { Deny 'source_head_mismatch' }
$dirty=@(& git -C $repo status --porcelain)
if ($LASTEXITCODE -ne 0 -or $dirty.Count -gt 0) { Deny 'source_tree_not_clean' }
$tauri=Join-Path $repo 'ide/app/src-tauri'
$app=Join-Path $tauri 'target/release/lu-ide-desktop.exe'
if (!(Test-Path -LiteralPath $app -PathType Leaf)) { Deny 'reviewed_prebuilt_desktop_missing' }
$desktopSigner=Get-AuthenticodeSignature -LiteralPath $app
if ($desktopSigner.Status -ne 'Valid' -or !$desktopSigner.SignerCertificate) {
  Deny 'reviewed_desktop_authenticode_missing'
}
$cliRoot=Join-Path $repo 'ide/app/node_modules/@tauri-apps/cli'
$cliManifest=Join-Path $cliRoot 'package.json'
$cli=Join-Path $repo 'ide/app/node_modules/.bin/tauri.cmd'
if (!(Test-Path $cliManifest) -or !(Test-Path $cli)) { Deny 'preinstalled_pinned_tauri_cli_missing' }
$cliVersion=(Get-Content $cliManifest -Raw | ConvertFrom-Json).version
if ($cliVersion -ne '2.12.1') { Deny 'bundler_version_not_reviewed' }
$scriptRoot=Join-Path $repo 'ide/packaging'
& python (Join-Path $scriptRoot 'stage_wo7_windows_release.py') `
  --manifest $ApprovedInventory --inputs $InputRoot --output $NewPayloadRoot --source-sha $SourceSha
if ($LASTEXITCODE -ne 0) { Deny 'reviewed_stage_denied' }
& python (Join-Path $scriptRoot 'verify_wo7_pe_imports.py') --staged-root $NewPayloadRoot
if ($LASTEXITCODE -ne 0) { Deny 'native_static_pe_import_closure_failed' }
& python (Join-Path $scriptRoot 'prepare_wo7_tauri_bundle.py') `
  --payload $NewPayloadRoot --desktop $app --tauri-dir $tauri --source-sha $SourceSha
if ($LASTEXITCODE -ne 0) { Deny 'source_to_native_tauri_payload_mismatch' }
$desktopSha=(Get-FileHash -Algorithm SHA256 -LiteralPath $app).Hash
Push-Location (Join-Path $repo 'ide/app')
try {
  & $cli bundle --no-binary-patching --bundles nsis --config src-tauri/tauri.wo7.nsis.conf.json --ci
  if ($LASTEXITCODE -ne 0) { Deny 'native_nsis_bundle_failed' }
} finally {
  Pop-Location
}
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $app).Hash -ine $desktopSha) {
  Deny 'bundler_mutated_reviewed_signed_desktop'
}
$bundles=Join-Path $tauri 'target/release/bundle/nsis'
$installers=@(Get-ChildItem -LiteralPath $bundles -File -Filter '*setup.exe')
if ($installers.Count -ne 1) { Deny 'one_exact_nsis_candidate_required' }
$installer=$installers[0].FullName
& (Join-Path $scriptRoot 'Sign-Wo7ReviewedArtifact.ps1') `
  -Kind Installer -Artifact $installer -SignTool $SignTool `
  -SignToolSha256 $SignToolSha256 -SignerThumbprint $InstallerSignerThumbprint `
  -TimestampUrl $TimestampUrl
if ($LASTEXITCODE -ne 0) { Deny 'native_authenticode_installer_signing_failed' }
$signature=Get-AuthenticodeSignature -LiteralPath $installer
if ($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or
    $signature.SignerCertificate.Thumbprint -ine $InstallerSignerThumbprint) {
  Deny 'signed_installer_postverify_failed'
}
if ((Test-Path -LiteralPath $NewCandidateRoot) -or [IO.Path]::GetFullPath($NewCandidateRoot) -eq $repo) {
  Deny 'candidate_output_not_new'
}
New-Item -ItemType Directory -Path $NewCandidateRoot | Out-Null
$service=Join-Path $NewPayloadRoot 'resources/recovery/windows_recovery_service.exe'
$policy=Get-Content (Join-Path $NewPayloadRoot 'resources/wo7-service-policy.json') -Raw | ConvertFrom-Json
Copy-Item -LiteralPath $installer -Destination (Join-Path $NewCandidateRoot 'lu-ide-signed-setup.exe')
Copy-Item -LiteralPath $service -Destination (Join-Path $NewCandidateRoot 'windows_recovery_service.exe')
Copy-Item -LiteralPath $app -Destination (Join-Path $NewCandidateRoot 'lu-ide-desktop.exe')
$candidate=[ordered]@{
  schema='lu.ide.wo7.signed-nsis.v1'
  releaseApproved=$false
  sourceSha=$SourceSha
  licensesReviewed=$true
  acceptance=@{nativeWindows='pending';installedSigned='pending';serviceLifecycle='pending';uninstall='pending'}
  installer=@{path='lu-ide-signed-setup.exe';sha256=(Get-FileHash (Join-Path $NewCandidateRoot 'lu-ide-signed-setup.exe') -Algorithm SHA256).Hash.ToLowerInvariant();signerThumbprint=$InstallerSignerThumbprint.ToLowerInvariant()}
  service=@{path='windows_recovery_service.exe';sha256=(Get-FileHash (Join-Path $NewCandidateRoot 'windows_recovery_service.exe') -Algorithm SHA256).Hash.ToLowerInvariant();signerThumbprint=$policy.signerThumbprint}
  desktop=@{path='lu-ide-desktop.exe';sha256=$desktopSha.ToLowerInvariant();signerThumbprint=$desktopSigner.SignerCertificate.Thumbprint.ToLowerInvariant()}
}
$candidate | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $NewCandidateRoot 'wo7-release.manifest.json') -Encoding UTF8
Write-Host 'WO7_SIGNED_CANDIDATE_PREPARED_ONLY=PASS'
Write-Host 'WO7_RELEASE_AND_INSTALLED_PROTECTED_FEATURE_ACCEPTANCE=NOT_GRANTED'
