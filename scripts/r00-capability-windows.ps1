# R00 hosted Windows release-lifecycle gate for exact-source test-only Lu.
# Does NOT prove physical model/GPU inference or versioned rollback.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{40}$')]
  [string]$SourceSha
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Native Windows 2025 runner required'}
if((git rev-parse HEAD).Trim() -cne $SourceSha){throw 'Wrong exact-head source'}
& node scripts/verify-source-tree.mjs
if($LASTEXITCODE -ne 0){throw 'Frozen source SHA mismatch'}
$out=Join-Path $env:RUNNER_TEMP 'lu-r00-capability-nsis'
$summary=Join-Path $PWD 'r00-capability-evidence'
New-Item -ItemType Directory -Force -Path $summary|Out-Null
$compiler=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'NSIS\makensis.exe'
if(-not (Test-Path -LiteralPath $compiler)){
  & choco install nsis --version=3.11.0 --yes --no-progress --limit-output
  if($LASTEXITCODE -ne 0){throw 'Pinned NSIS 3.11 unavailable'}
}
if(-not (Test-Path -LiteralPath $compiler)){throw 'NSIS compiler missing'}
& $compiler /VERSION
if($LASTEXITCODE -ne 0){throw 'NSIS compiler startup failed'}
& scripts/build-agent01g-physical-windows-nsis.ps1 -SourceSha $SourceSha -OutputDirectory $out
if($LASTEXITCODE -ne 0){throw 'Pinned source CPU engine/current desktop NSIS build failed'}
$manifest=Join-Path $out 'installer-checksums.json'
if(-not(Test-Path -LiteralPath $manifest)){throw 'Missing immutable installer receipt'}
$meta=Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
if($meta.sourceSha -cne $SourceSha -or $meta.genuineInstallerProduced -ne $true){
  throw 'Installer source/creation provenance denied'
}
$exe=Join-Path $out $meta.installerName
if((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant() -cne $meta.installerSha256){
  throw 'Installer byte identity mismatch'
}
$cycleArgs=@{
  InstallerPath=$exe
  InstallerSha256=$meta.installerSha256
  ExpectedLuSha256=$meta.appSha256
  ExpectedEngineSha256=$meta.engineSha256
  UninstallerName='UninstallLuAGENT01GPhysicalTest.exe'
  DisposableWindowsConfirmed=$true
  EvidenceDir=(Join-Path $out 'native-cycle')
}
& scripts/m3-windows-nsis-cycle.ps1 @cycleArgs
if($LASTEXITCODE -ne 0){throw 'Actual disposable native install/uninstall failed'}
$receipt=Get-Content -LiteralPath (Join-Path $out 'native-cycle/m3-nsis-cycle.json') -Raw|ConvertFrom-Json
if($receipt.installer -cne 'PASS_REAL_NATIVE_PROCESS_EXIT' -or
   $receipt.installedApp -cne 'PASS_SHA256' -or
   $receipt.installedRealEngine -cne 'PASS_SHA256' -or
   $receipt.packageRemoval -cne 'PASS_BINARIES_ABSENT' -or
   $receipt.profilePreservation -cne 'PASS_UNTOUCHED_APPDATA_SENTINEL' -or
   $receipt.versionedRollback -cne 'NOT_TESTED_REQUIRES_TWO_DISTINCT_INSTALLERS'){
  throw 'Incomplete or exaggerated native lifecycle receipt'
}
Copy-Item -LiteralPath $manifest -Destination (Join-Path $summary 'installer-checksums.json')
Copy-Item -LiteralPath (Join-Path $out 'native-cycle/m3-nsis-cycle.json') -Destination (Join-Path $summary 'native-nsis-cycle.json')
"Windows native installed NSIS x64 CPU build, install/remove and disposable profile sentinel: PASS" >> $env:GITHUB_STEP_SUMMARY
"Distinct-version rollback, 7B+/MMProj, GPU Vulkan and installed AI model/tool dialogs: OPEN" >> $env:GITHUB_STEP_SUMMARY
