# Technical verification only, never start any language server or install broker.
param([Parameter(Mandatory)][string]$SetupPath,
      [Parameter(Mandatory)][string]$SourceSha,
      [Parameter(Mandatory)][string]$Reference)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:GITHUB_ACTIONS -ne 'true' -or $env:OS -ne 'Windows_NT') { throw 'Disposable hosted Windows required' }
if ($SourceSha -cnotmatch '^[a-f0-9]{40}$' -or (git rev-parse HEAD).Trim() -cne $SourceSha) { throw 'Exact source mismatch' }
$setup=(Resolve-Path -LiteralPath $SetupPath).Path
$parent=(Join-Path (Get-Location) 'ide\app\src-tauri\target\release\bundle\nsis\')
if (!$setup.StartsWith($parent,[StringComparison]::OrdinalIgnoreCase) -or (Get-Item -LiteralPath $setup).LinkType) { throw 'Foreign installer' }
$install=Join-Path $env:RUNNER_TEMP ('lu-ide-dev02-technical-'+$env:GITHUB_RUN_ID+'-'+$env:GITHUB_RUN_ATTEMPT)
$fixture=$install+'-project'
if ((Test-Path -LiteralPath $install) -or (Test-Path -LiteralPath $fixture)) { throw 'Existing install or fixture' }
New-Item -ItemType Directory -Path $fixture | Out-Null
$file=Join-Path $fixture 'main.rs'
[IO.File]::WriteAllText($file,"fn main() {} `n")
$before=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
$exe=Join-Path $install 'lu-ide-desktop.exe'
try {
  $p=Start-Process -FilePath $setup -ArgumentList @('/S',('/D='+$install)) -Wait -PassThru
  if ($p.ExitCode -ne 0 -or !(Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Real NSIS installation failed' }
  $sha=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
  & python ide/packaging/verify_technical_lsp_install.py --reference $Reference --archives (Join-Path $env:RUNNER_TEMP 'dev02-technical-archives') --original-notices (Join-Path $env:RUNNER_TEMP 'dev02-technical-notices') --platform windows-x86_64 --source-sha $SourceSha --windows-root $install --windows-desktop-sha $sha
  if ($LASTEXITCODE -ne 0) { throw 'Installed resource verification failed' }
  if (@(Get-ChildItem -LiteralPath $install -Recurse -File -Filter windows_recovery_service.exe).Count -ne 0) { throw 'Unexpected recovery service in technical install' }
  if (@(Get-Process -Name clangd,rust-analyzer,ruff,lldb-dap -ErrorAction SilentlyContinue).Count -ne 0) { throw 'Unexpected provider process baseline' }
  # The independently verified exact NSIS-transformed desktop hash is
  # retained for the native UI witness; no installed-only hash acceptance.
  if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -cne $sha) { throw 'Installed executable changed after verification' }
  & 'ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1' -InstalledExecutable $exe -ExpectedSha256 $sha -Scenario ProjectReadOnly -FixtureRoot $fixture
  if (!$? -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -cne $before) { throw 'Native read-only UI failed' }
  if (@(Get-Process -Name clangd,rust-analyzer,ruff,lldb-dap -ErrorAction SilentlyContinue).Count -ne 0) { throw 'Unexpected provider process' }
} finally {
  if (Test-Path -LiteralPath $install) {
    $u=@(Get-ChildItem -LiteralPath $install -File -Filter '*uninstall*.exe')
    if ($u.Count -ne 1) { throw 'Missing or ambiguous actual NSIS uninstaller' }
    $p=Start-Process -FilePath $u[0].FullName -ArgumentList '/S' -Wait -PassThru
    if ($p.ExitCode -ne 0 -or (Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath (Join-Path $install 'technical-lsp'))) { throw 'Native uninstall incomplete' }
  }
}
Write-Host 'DEV02_TECHNICAL_NSIS_INSTALLED_UI_RESOURCES_UNINSTALL=PASS'
