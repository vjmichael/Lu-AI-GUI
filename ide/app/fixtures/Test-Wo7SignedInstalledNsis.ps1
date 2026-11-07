# WO7-D real signed installed NSIS / SCM witness; NEVER a synthetic release pass.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][string]$SignerThumbprint,
  [Parameter(Mandatory=$true)][string]$ServiceSha256,
  [Parameter(Mandatory=$true)][string]$ServiceSignerThumbprint,
  [Parameter(Mandatory=$true)][string]$InstallRoot
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$ServiceName='LuIdeRecoveryBroker'
$installed=$false
$launched=@()
$uninstalled=$false

function Deny([string]$code) { throw "DENIED: WO7 installed acceptance: $code" }
function SignaturePin([string]$path,[string]$sha,[string]$thumb) {
  if (!(Test-Path -LiteralPath $path -PathType Leaf)) { Deny 'signed_artifact_missing' }
  if ($sha -notmatch '^[a-fA-F0-9]{64}$' -or $thumb -notmatch '^[a-fA-F0-9]{40}$') {
    Deny 'release_pins_missing'
  }
  if ((Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash -ine $sha) {
    Deny 'binary_digest_mismatch'
  }
  $signature=Get-AuthenticodeSignature -LiteralPath $path
  if ($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or
      $signature.SignerCertificate.Thumbprint -ine $thumb) {
    Deny 'authenticode_signature_mismatch'
  }
}
function ServiceIdentity([string]$expectedImage) {
  $service=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
  if (!$service) { Deny 'protected_scm_service_missing' }
  if ($service.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE')) {
    Deny 'service_identity_wrong'
  }
  if ($service.PathName.Trim('"') -ine $expectedImage -or $service.ServiceType -ne 'Own Process') {
    Deny 'service_image_or_type_wrong'
  }
  return $service
}
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  Deny 'dedicated_native_windows_runner_required'
}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$principal=New-Object Security.Principal.WindowsPrincipal($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Deny 'administrator_required'
}
$root=[IO.Path]::GetFullPath($InstallRoot).TrimEnd('\')
$fixedRoots=@((Join-Path $env:ProgramFiles 'Lu IDE'))
$pfX86=[Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
if ($pfX86) { $fixedRoots+=Join-Path $pfX86 'Lu IDE' }
if ($root -notin $fixedRoots) {
  Deny 'fixed_program_files_install_root_required'
}
if ((Test-Path -LiteralPath $root) -or (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
  Deny 'installation_or_service_already_present'
}
$setup=(Resolve-Path -LiteralPath $InstallerPath).Path
SignaturePin $setup $InstallerSha256 $SignerThumbprint
try {
  $install=Start-Process -FilePath $setup -ArgumentList @('/S',('/D='+$root)) -PassThru -Wait
  if ($install.ExitCode -ne 0) { Deny 'signed_nsis_setup_failed' }
  $installed=$true
  $app=Join-Path $root 'lu-ide-desktop.exe'
  $serviceExe=Join-Path $root 'resources\recovery\windows_recovery_service.exe'
  SignaturePin $serviceExe $ServiceSha256 $ServiceSignerThumbprint
  $service=ServiceIdentity $serviceExe
  if ($service.State -notin @('Stopped','Running')) { Deny 'unexpected_service_state' }
  if (!(Test-Path -LiteralPath $app -PathType Leaf)) { Deny 'installed_desktop_missing' }
  foreach ($cycle in 1..2) {
    $proc=Start-Process -FilePath $app -WorkingDirectory $root -PassThru
    $launched+= $proc
    Start-Sleep -Seconds 5
    $proc.Refresh()
    if ($proc.HasExited) { Deny 'desktop_failed_during_launch' }
    ServiceIdentity $serviceExe | Out-Null
    Stop-Process -Id $proc.Id -Force
    $proc.WaitForExit(10000) | Out-Null
  }
  # A successful install/launch is NOT protected Run/Test/Debug acceptance.
  Write-Host 'WO7_SIGNED_INSTALL_TWO_LAUNCHES_AND_SCM_IDENTITY=PASS'
  $uninstaller=@(Get-ChildItem -LiteralPath $root -File -Filter '*uninstall*.exe')
  if ($uninstaller.Count -ne 1) { Deny 'nsis_uninstaller_missing_or_ambiguous' }
  $uninstall=Start-Process -FilePath $uninstaller[0].FullName -ArgumentList '/S' -PassThru -Wait
  if ($uninstall.ExitCode -ne 0) { Deny 'nsis_service_uninstall_failed' }
  $uninstalled=$true
  Start-Sleep -Seconds 3
  if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Deny 'protected_service_left_after_uninstall'
  }
  if (Test-Path -LiteralPath $serviceExe -PathType Leaf) {
    Deny 'protected_service_binary_left_after_uninstall'
  }
} finally {
  foreach($proc in $launched) {
    try { $proc.Refresh(); if (!$proc.HasExited) { Stop-Process -Id $proc.Id -Force } } catch {}
  }
  if ($installed -and !$uninstalled) {
    Write-Warning 'WO7 partial installation requires separately verified cleanup; do not report acceptance'
  }
}
Write-Host 'WO7_SIGNED_INSTALLED_SCM_SMOKE=PASS'
Write-Host 'WO7_INSTALLED_PROTECTED_RUN_TEST_DEBUG_AND_RECOVERY=NOT_RUN'
