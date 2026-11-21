# P04 signed-only native Windows NSIS uninstall/reinstall user-state acceptance.
# Requires a disposable, elevated GitHub runner and independently signed bytes.
# Clean reinstall is not an in-place upgrade or upgrade rollback proof.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][string]$InstallerSigner,
  [Parameter(Mandatory=$true)][string]$ServiceSha256,
  [Parameter(Mandatory=$true)][string]$ServiceSigner,
  [Parameter(Mandatory=$true)][string]$DesktopSha256,
  [Parameter(Mandatory=$true)][string]$DesktopSigner,
  [Parameter(Mandatory=$true)][string]$InstallRoot,
  [Parameter(Mandatory=$true)][string]$FixtureRoot
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
function Deny([string]$reason) { throw "P04_SIGNED_REINSTALL_DENIED:$reason" }
function Signed([string]$file,[string]$hash,[string]$thumb) {
  if ($hash -notmatch '^[0-9a-fA-F]{64}$' -or $thumb -notmatch '^[0-9a-fA-F]{40}$') { Deny 'missing_pins' }
  if (!(Test-Path -LiteralPath $file -PathType Leaf)) { Deny 'missing_signed_binary' }
  if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ine $hash) { Deny 'digest_mismatch' }
  $signature=Get-AuthenticodeSignature -LiteralPath $file
  if ($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or
      $signature.SignerCertificate.Thumbprint -ine $thumb) { Deny 'signer_mismatch' }
}
function NoReparse([string]$file) {
  $item=Get-Item -LiteralPath $file -Force -ErrorAction Stop
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { Deny 'reparse_point' }
}
function Service([string]$image) {
  $svc=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
  if (!$svc -or $svc.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE') -or
      $svc.PathName.Trim('"') -ine $image -or $svc.ServiceType -ne 'Own Process') {
    Deny 'service_identity_or_image_drift'
  }
}
function RunNsis([string]$image,[string[]]$arguments,[string]$stage) {
  $proc=Start-Process -FilePath $image -ArgumentList $arguments -PassThru
  if (!$proc.WaitForExit(120000)) {
    try { Stop-Process -Id $proc.Id -Force } catch {}
    Deny ($stage+'_timeout')
  }
  if ($proc.ExitCode -ne 0) { Deny ($stage+'_nonzero') }
}
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  Deny 'disposable_windows_ci_only'
}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { Deny 'admin_required' }
$root=[IO.Path]::GetFullPath($InstallRoot).TrimEnd('\')
$accepted=@((Join-Path $env:ProgramFiles 'Lu IDE'))
$x86=[Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
if ($x86) { $accepted+=Join-Path $x86 'Lu IDE' }
if (!($accepted | Where-Object { $root -ieq [IO.Path]::GetFullPath($_).TrimEnd('\') })) {
  Deny 'unexpected_install_root'
}
$fixture=[IO.Path]::GetFullPath($FixtureRoot).TrimEnd('\')
$temp=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
$source=Join-Path $fixture 'main.rs'
if (!$fixture.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or
    !(Test-Path -LiteralPath $source -PathType Leaf)) { Deny 'unexpected_project_fixture' }
NoReparse $fixture
NoReparse $source
$sourceHash=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
$appData=Join-Path $env:APPDATA 'org.lu.ide'
$catalog=Join-Path $appData 'projects.json'
if ((Test-Path -LiteralPath $root) -or (Test-Path -LiteralPath $appData) -or
    (Get-Service LuIdeRecoveryBroker -ErrorAction SilentlyContinue)) {
  Deny 'preexisting_installation_or_user_state'
}
$setup=(Resolve-Path -LiteralPath $InstallerPath).Path
Signed $setup $InstallerSha256 $InstallerSigner
$exe=Join-Path $root 'lu-ide-desktop.exe'
$serviceExe=Join-Path $root 'resources\recovery\windows_recovery_service.exe'
function Install {
  RunNsis $setup @('/S',('/D='+$root)) 'install'
  Signed $exe $DesktopSha256 $DesktopSigner
  Signed $serviceExe $ServiceSha256 $ServiceSigner
  Service $serviceExe
}
function Uninstall {
  $uninstallers=@(Get-ChildItem -LiteralPath $root -File -Filter '*uninstall*.exe')
  if ($uninstallers.Count -ne 1) { Deny 'uninstaller_missing_or_ambiguous' }
  RunNsis $uninstallers[0].FullName @('/S') 'uninstall'
  if ((Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $serviceExe) -or
      (Get-Service LuIdeRecoveryBroker -ErrorAction SilentlyContinue)) {
    Deny 'uninstall_failed_to_remove_binaries_and_service'
  }
}
$installed=$false
try {
  Install
  $installed=$true
  & ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1 -InstalledExecutable $exe -ExpectedSha256 $DesktopSha256 -Scenario ProjectReadOnly -FixtureRoot $fixture
  if (!$?) { Deny 'native_first_install_project_registration_failed' }
  if (!(Test-Path -LiteralPath $catalog -PathType Leaf)) { Deny 'catalog_not_persisted' }
  NoReparse $appData
  NoReparse $catalog
  $catalogHash=(Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash
  Uninstall
  $installed=$false
  if (!(Test-Path -LiteralPath $catalog -PathType Leaf) -or
      (Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash) {
    Deny 'uninstall_modified_or_deleted_user_catalog'
  }
  if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $sourceHash) {
    Deny 'uninstall_modified_user_source'
  }
  Write-Host 'P04_SIGNED_UNINSTALL_USER_CATALOG_PRESERVED=PASS'
  Install
  $installed=$true
  & ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1 -InstalledExecutable $exe -ExpectedSha256 $DesktopSha256 -Scenario P04ReinstalledProject -FixtureRoot $fixture
  if (!$?) { Deny 'reinstalled_native_editor_or_project_missing' }
  if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ne $catalogHash -or
      (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $sourceHash) {
    Deny 'reinstall_modified_user_catalog_or_source'
  }
  Uninstall
  $installed=$false
  Write-Host 'P04_SIGNED_NATIVE_REINSTALL_CATALOG_AND_SOURCE=PASS'
  Write-Host 'P04_INPLACE_VERSION_UPGRADE_AND_ROLLBACK=NOT_TESTED'
  Write-Host 'P04_PROTECTED_RUN_TEST_DEBUG=NOT_TESTED'
  Write-Host 'P04_RELEASE_ACCEPTED=NO'
} finally {
  if ($installed) { Write-Warning 'P04 partially installed candidate remains after failure; no acceptance granted' }
}
