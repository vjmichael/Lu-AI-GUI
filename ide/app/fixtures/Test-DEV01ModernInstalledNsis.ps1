# Genuine unsigned NSIS, installed WebView2 UIA and native OS two-process regression.
# Does not grant debugger, SCM, remote publication, signed release or consent.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$SetupPath,
      [Parameter(Mandatory)][string]$SourceSha)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason) { throw 'DEV01_MODERN_NSIS_DENIED:'+$reason }
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  Deny 'native_disposable_windows_runner_required'
}
if ($SourceSha -cnotmatch '^[a-f0-9]{40}$' -or
    (git rev-parse HEAD).Trim() -cne $SourceSha) { Deny 'exact_source_sha_drift' }
$setup=(Resolve-Path -LiteralPath $SetupPath).Path
$expected=(Join-Path (Get-Location) 'ide\app\src-tauri\target\release\bundle\nsis\')
if (!$setup.StartsWith($expected,[StringComparison]::OrdinalIgnoreCase) -or
    (Get-Item -LiteralPath $setup).LinkType) { Deny 'untrusted_installer_location' }
$install=Join-Path $env:RUNNER_TEMP ('lu-ide-dev01-modern-installed-'+$env:GITHUB_RUN_ID)
$fixture=Join-Path $env:RUNNER_TEMP ('lu-ide-dev01-modern-project-'+$env:GITHUB_RUN_ID)
if ((Test-Path -LiteralPath $install) -or (Test-Path -LiteralPath $fixture)) {
  Deny 'non_disposable_installed_fixture'
}
New-Item -ItemType Directory -Path $fixture | Out-Null
[IO.File]::WriteAllText((Join-Path $fixture 'main.rs'),"fn main() { }"+"`n")
[IO.File]::WriteAllText((Join-Path $fixture 'second.rs'),"fn second() { }"+"`n")
[IO.File]::WriteAllText((Join-Path $fixture 'move-me.rs'),"fn move_me() { }"+"`n")
for($d=0;$d -lt 24;$d++) {
  $folder=Join-Path $fixture ('src-{0:D2}' -f $d)
  New-Item -ItemType Directory -Path $folder | Out-Null
  for($n=0;$n -lt 36;$n++) {
    [IO.File]::WriteAllText((Join-Path $folder ('file-{0:D3}.rs' -f $n)),
      ('// source {0} {1}' -f $d,$n)+"`n")
  }
}
if (@(Get-ChildItem -LiteralPath $fixture -File -Recurse -Filter '*.rs').Count -ne 867) {
  Deny 'original_867_file_fixture_missing'
}
$installed=$false
$removed=$false
try {
  $p=Start-Process -FilePath $setup -ArgumentList @('/S',('/D='+$install)) -Wait -PassThru
  if ($p.ExitCode -ne 0) { Deny 'native_nsis_install_error' }
  $installed=$true
  $exe=Join-Path $install 'lu-ide-desktop.exe'
  if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { Deny 'installed_webview2_app_missing' }
  $sha=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
  & 'ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1' -InstalledExecutable $exe `
    -ExpectedSha256 $sha -Scenario Dev01Modern -FixtureRoot $fixture
  if (!$?) { Deny 'modern_dev01_native_installed_first_process_failed' }
  & 'ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1' -InstalledExecutable $exe `
    -ExpectedSha256 $sha -Scenario Dev01ModernRecovery -FixtureRoot $fixture
  if (!$?) { Deny 'modern_dev01_native_installed_restart_failed' }
  $uninstallers=@(Get-ChildItem -LiteralPath $install -Filter '*uninstall*.exe' -File)
  if ($uninstallers.Count -ne 1) { Deny 'native_uninstaller_missing_or_ambiguous' }
  $u=Start-Process -FilePath $uninstallers[0].FullName -ArgumentList '/S' -Wait -PassThru
  if ($u.ExitCode -ne 0 -or (Test-Path -LiteralPath $exe)) {
    Deny 'installed_app_not_cleanly_uninstalled'
  }
  $removed=$true
  Write-Host 'DEV01_MODERN_WINDOWS_NSIS_ACTUAL_INSTALLED_UI=PASS'
  Write-Host 'DEV01_MODERN_WINDOWS_PHYSICAL_RELEASE_OR_HUMAN_APPROVAL=NOT_TESTED'
} finally {
  if ($installed -and !$removed) {
    $u=@(Get-ChildItem -LiteralPath $install -Filter '*uninstall*.exe' -File -ErrorAction SilentlyContinue)
    if ($u.Count -eq 1) {
      try { Start-Process -FilePath $u[0].FullName -ArgumentList '/S' -Wait -PassThru | Out-Null } catch {}
    }
    Write-Warning 'DEV01 modern installed NSIS group incomplete; cleanup attempted'
  }
}
