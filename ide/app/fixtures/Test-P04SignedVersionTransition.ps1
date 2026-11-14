# P04 native two-version signed Windows witness. No production upgrader.
# Requires two separately reviewed Authenticode candidates and an empty CI VM.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$OldCandidate,
  [Parameter(Mandatory=$true)][string]$NewCandidate,
  [Parameter(Mandatory=$true)][string]$OldSourceSha,
  [Parameter(Mandatory=$true)][string]$NewSourceSha,
  [Parameter(Mandatory=$true)][string]$FixtureRoot,
  [ValidateSet('Upgrade','InjectedRecovery')][string]$Mode='Upgrade'
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason) { throw "P04_NATIVE_SIGNED_UPGRADE_DENIED:$reason" }
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  Deny 'isolated_ci_only'
}
$who=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (!$who.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { Deny 'admin_required' }
if ($OldSourceSha -notmatch '^[a-fA-F0-9]{40}$' -or $NewSourceSha -notmatch '^[a-fA-F0-9]{40}$' -or
  $OldSourceSha -ieq $NewSourceSha) { Deny 'two_distinct_reviewed_heads_required' }
$root=Join-Path $env:ProgramFiles 'Lu IDE'
$exe=Join-Path $root 'lu-ide-desktop.exe'
$service=Join-Path $root 'resources\recovery\windows_recovery_service.exe'
$catalog=Join-Path $env:APPDATA 'org.lu.ide\projects.json'
$runner=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')
$fixture=[IO.Path]::GetFullPath($FixtureRoot)
if (!$fixture.StartsWith(($runner+'\'),[StringComparison]::OrdinalIgnoreCase) -or
  !(Test-Path -LiteralPath (Join-Path $fixture 'main.rs') -PathType Leaf)) { Deny 'invalid_fixture' }
if ((Test-Path -LiteralPath $root) -or (Test-Path -LiteralPath $catalog) -or
    (Get-Service LuIdeRecoveryBroker -ErrorAction SilentlyContinue)) {
  Deny 'preexisting_installed_app_or_state'
}
function Candidate([string]$path,[string]$source) {
  $dir=[IO.Path]::GetFullPath($path).TrimEnd('\')
  $workspace=[IO.Path]::GetFullPath($env:GITHUB_WORKSPACE).TrimEnd('\')
  $oldRoot=Join-Path $workspace 'signed-old'
  $newRoot=Join-Path $workspace 'signed-new'
  # Downloaded signed artifacts land in precisely these reviewed directories,
  # not in RUNNER_TEMP. Never accept any other repository or user directory.
  if (!$dir.StartsWith(($runner+'\'),[StringComparison]::OrdinalIgnoreCase) -and
      $dir -ine $oldRoot -and $dir -ine $newRoot) { Deny 'candidate_outside_ci' }
  $manifestPath=Join-Path $dir 'wo7-release.manifest.json'
  if (!(Test-Path -LiteralPath $manifestPath -PathType Leaf)) { Deny 'candidate_manifest_missing' }
  $manifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
  if ($manifest.schema -ne 'lu.ide.wo7.signed-nsis.v1' -or $manifest.releaseApproved -ne $false -or
      $manifest.licensesReviewed -ne $true -or $manifest.sourceSha -ine $source) {
    Deny 'candidate_not_reviewed'
  }
  if ($manifest.installer.path -notmatch '^[a-zA-Z0-9._/-]{1,160}$' -or
      $manifest.installer.path.Split('/') -contains '..') { Deny 'candidate_path_unsafe' }
  $setup=[IO.Path]::GetFullPath((Join-Path $dir $manifest.installer.path))
  if (!$setup.StartsWith(($dir+'\'),[StringComparison]::OrdinalIgnoreCase) -or
      !(Test-Path -LiteralPath $setup -PathType Leaf)) { Deny 'candidate_path_escape' }
  return [pscustomobject]@{manifest=$manifest; setup=$setup}
}
function Pin([string]$file,[string]$sha,[string]$thumb) {
  if ($sha -notmatch '^[a-fA-F0-9]{64}$' -or $thumb -notmatch '^[a-fA-F0-9]{40}$') { Deny 'pin_missing' }
  if (!(Test-Path -LiteralPath $file -PathType Leaf)) { Deny 'signed_binary_missing' }
  if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ine $sha) { Deny 'hash_mismatch' }
  $s=Get-AuthenticodeSignature -LiteralPath $file
  if ($s.Status -ne 'Valid' -or !$s.SignerCertificate -or
      $s.SignerCertificate.Thumbprint -ine $thumb) { Deny 'signer_mismatch' }
}
function RunFixed([string]$file,[string[]]$arguments) {
  $p=Start-Process -FilePath $file -ArgumentList $arguments -PassThru
  if (!$p.WaitForExit(120000)) {
    try { Stop-Process -Id $p.Id -Force } catch {}
    Deny 'installer_timed_out'
  }
  if ($p.ExitCode -ne 0) { Deny 'installer_nonzero' }
}
function Install($candidate) {
  Pin $candidate.setup $candidate.manifest.installer.sha256 $candidate.manifest.installer.signerThumbprint
  RunFixed $candidate.setup @('/S',('/D='+$root))
  Pin $exe $candidate.manifest.desktop.sha256 $candidate.manifest.desktop.signerThumbprint
  Pin $service $candidate.manifest.service.sha256 $candidate.manifest.service.signerThumbprint
  $svc=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
  if (!$svc -or $svc.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE') -or
      $svc.PathName.Trim('"') -ine $service) { Deny 'service_identity_drift' }
  return (Get-Item -LiteralPath $exe).VersionInfo.ProductVersion
}
function RemoveInstalled {
  $uninstallers=@(Get-ChildItem -LiteralPath $root -Filter '*uninstall*.exe' -File)
  if ($uninstallers.Count -ne 1) { Deny 'ambiguous_uninstaller' }
  RunFixed $uninstallers[0].FullName @('/S')
  if ((Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $service) -or
      (Get-Service LuIdeRecoveryBroker -ErrorAction SilentlyContinue)) { Deny 'service_not_removed' }
}
$old=Candidate $OldCandidate $OldSourceSha
$new=Candidate $NewCandidate $NewSourceSha
if ($old.manifest.installer.sha256 -ieq $new.manifest.installer.sha256) { Deny 'same_signed_installer_not_upgrade' }
$installed=$false
$oldKnown=$false
try {
  $versionOld=Install $old
  $installed=$true
  $oldKnown=$true
  $parameters=@{
    InstalledExecutable=$exe
    ExpectedSha256=$old.manifest.desktop.sha256
    FixtureRoot=$fixture
    Scenario='ProjectReadOnly'
  }
  & ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1 @parameters
  if (!$?) { Deny 'native_old_project_registration_not_verified' }
  if (!(Test-Path -LiteralPath $catalog -PathType Leaf)) { Deny 'catalog_not_persisted' }
  $catalogHash=(Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
  $sourceHash=(Get-FileHash -LiteralPath (Join-Path $fixture 'main.rs') -Algorithm SHA256).Hash
  RemoveInstalled
  $installed=$false
  $versionNew=Install $new
  $installed=$true
  if (!$versionNew -or !$versionOld -or $versionNew -eq $versionOld) { Deny 'different_real_versions_not_observed' }
  $current=$new
  if ($Mode -eq 'InjectedRecovery') {
    # Deterministic post-install fault, NOT a failed binary install receipt.
    RemoveInstalled
    $installed=$false
    $restored=Install $old
    $installed=$true
    if ($restored -ne $versionOld) { Deny 'old_version_not_restored' }
    $current=$old
    Write-Host 'P04_INJECTED_POST_INSTALL_ROLLBACK=PASS'
    Write-Host 'P04_ACTUAL_BROKEN_INSTALLER_ROLLBACK=NOT_TESTED'
  }
  $parameters=@{
    InstalledExecutable=$exe
    ExpectedSha256=$current.manifest.desktop.sha256
    FixtureRoot=$fixture
    Scenario='P04ReinstalledProject'
  }
  & ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1 @parameters
  if (!$?) { Deny 'registered_native_project_not_reopened' }
  if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash -or
      (Get-FileHash -LiteralPath (Join-Path $fixture 'main.rs') -Algorithm SHA256).Hash -ne $sourceHash) {
    Deny 'catalog_or_user_source_changed'
  }
  RemoveInstalled
  $installed=$false
  Write-Host 'P04_TWO_VERSION_NATIVE_CATALOG_AND_SCM_CLEANUP=PASS'
  Write-Host 'P04_INPLACE_UPGRADE=NOT_TESTED'
  Write-Host 'P04_SIGNED_RELEASE_APPROVED=NO'
} catch {
  # Attempt recovery from a rejected upgrade using ONLY the old reviewed
  # signed package. If this fails, never claim recovery or discard user data.
  if ($oldKnown) {
    try {
      if ($installed) { RemoveInstalled; $installed=$false }
      if (!(Test-Path -LiteralPath $exe)) {
        $revived=Install $old
        $installed=$true
        Write-Warning 'P04 rejected transaction: previous reviewed version reinstalled, not release accepted'
      }
    } catch { Write-Warning 'P04 rollback could not be verified; isolated runner requires manual disposal' }
  }
  throw
} finally {
  if ($installed) { Write-Warning 'P04 native candidate remains installed after test; no acceptance' }
}
