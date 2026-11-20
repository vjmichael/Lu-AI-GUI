# E2 release-independent installed-app smoke: UNSIGNED default IDE only.
# This script must never certify signed E2 LSP acceptance or OS containment.
# It only runs on a fresh ephemeral Windows Actions runner, not a user's PC.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][string]$InstallRoot
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  throw 'DENIED: unsigned installed smoke restricted to disposable GitHub Actions runner'
}
$runnerTemp=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
$absoluteRoot=[IO.Path]::GetFullPath($InstallRoot).TrimEnd('\')+'\'
if (!$absoluteRoot.StartsWith($runnerTemp,[StringComparison]::OrdinalIgnoreCase) -or
    $absoluteRoot.Length -le $runnerTemp.Length+12) {
  throw 'DENIED: install destination is not a dedicated disposable runner path'
}
if (Test-Path -LiteralPath $absoluteRoot) {
  throw 'DENIED: refuse to overwrite an existing installation'
}
$installer=(Resolve-Path -LiteralPath $InstallerPath).Path
if (![IO.Path]::IsPathRooted($installer) -or
    [IO.Path]::GetExtension($installer) -ne '.exe') {
  throw 'DENIED: requires a real absolute Windows NSIS setup executable'
}
if (Get-Service -Name 'LuIdeRecoveryBroker' -ErrorAction SilentlyContinue) {
  throw 'DENIED: unsigned IDE test runner unexpectedly has protected E2 SCM service'
}
$sha=(Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash
if ($sha -notmatch '^[0-9A-Fa-f]{64}$') {
  throw 'DENIED: installer digest unavailable'
}
Write-Host "UNSIGNED_TEST_INSTALLER_SHA256=$sha"
$installed=$false
$uninstallAttempted=$false
$launched=@()
try {
  $installPath=$absoluteRoot.TrimEnd('\')
  $install=Start-Process -FilePath $installer -ArgumentList @('/S',("/D="+$installPath)) -PassThru -Wait
  if ($install.ExitCode -ne 0) {
    throw "DENIED: real NSIS installed-app setup exited $($install.ExitCode)"
  }
  $installed=$true
  $app=Join-Path $installPath 'lu-ide-desktop.exe'
  if (!(Test-Path -LiteralPath $app -PathType Leaf)) {
    throw 'DENIED: installed standalone desktop binary missing'
  }
  if ((Get-Item -LiteralPath $app).Length -le 1048576) {
    throw 'DENIED: installed desktop binary unexpectedly small'
  }
  if (Test-Path -LiteralPath (Join-Path $installPath 'language-servers')) {
    throw 'DENIED: default unsigned IDE unexpectedly bundled language servers'
  }
  if (Test-Path -LiteralPath (Join-Path $installPath 'recovery\windows_recovery_service.exe')) {
    throw 'DENIED: default unsigned IDE unexpectedly bundled protected SCM executable'
  }
  if (Get-Service -Name 'LuIdeRecoveryBroker' -ErrorAction SilentlyContinue) {
    throw 'DENIED: unsigned alpha unexpectedly installed an E2 privileged service'
  }
  foreach ($cycle in 1..2) {
    $proc=Start-Process -FilePath $app -WorkingDirectory $installPath -PassThru
    $launched += $proc
    Start-Sleep -Seconds 6
    $proc.Refresh()
    if ($proc.HasExited) {
      throw "DENIED: installed desktop exited on launch cycle $cycle (exit=$($proc.ExitCode))"
    }
    Write-Host "PASS: installed standalone desktop process remained live, cycle $cycle"
    Stop-Process -Id $proc.Id -Force -ErrorAction Stop
    $proc.WaitForExit(10000) | Out-Null
  }
  Write-Host 'PASS: unsigned default IDE installed, started twice, and did not launch LSP service'
} finally {
  foreach ($proc in $launched) {
    try { $proc.Refresh(); if (!$proc.HasExited) { Stop-Process -Id $proc.Id -Force } }
    catch { Write-Warning "OS process cleanup reported failure: $_" }
  }
  if ($installed -and (Test-Path -LiteralPath $absoluteRoot)) {
    $uninstaller=@(Get-ChildItem -LiteralPath $absoluteRoot -File -Filter '*uninstall*.exe' -ErrorAction SilentlyContinue)
    if ($uninstaller.Count -eq 1) {
      $uninstallAttempted=$true
      $uninstall=Start-Process -FilePath $uninstaller[0].FullName -ArgumentList '/S' -Wait -PassThru
      if ($uninstall.ExitCode -ne 0) {
        throw "DENIED: NSIS uninstall failed (exit=$($uninstall.ExitCode))"
      }
      Start-Sleep -Seconds 3
      if (Test-Path -LiteralPath (Join-Path $absoluteRoot 'lu-ide-desktop.exe')) {
        throw 'DENIED: installed binary remained after NSIS uninstall'
      }
      Write-Host 'PASS: unsigned standalone NSIS uninstall removed installed desktop executable'
    } else {
      throw "DENIED: expected exactly one installed NSIS uninstaller, found $($uninstaller.Count)"
    }
  }
}
if (!$installed -or !$uninstallAttempted) {
  throw 'DENIED: complete install-to-uninstall cycle not independently observed'
}
Write-Host 'UNSIGNED_INSTALLED_DESKTOP_SMOKE_ONLY=PASS'
Write-Host 'SIGNED_E2_LANGUAGE_SERVER_ACCEPTANCE=NOT_RUN'
