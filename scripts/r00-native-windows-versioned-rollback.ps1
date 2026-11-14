# R00 two-distinct-version native Windows lifecycle acceptance.
# Two separately built source heads required. This is NOT a GGUF execution,
# profile migration/DB downgrade, human OS picker, or production signed release.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$BaselineManifestPath,
  [Parameter(Mandatory=$true)][string]$CandidateManifestPath,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
  [switch]$DisposableWindowsConfirmed,
  [ValidateRange(60,1800)][int]$TimeoutSeconds=480
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
  throw 'Only an operator-confirmed disposable native Windows test is permitted'
}
function Assert-Hex([string]$v,[int]$length,[string]$name){
  if($null -eq $v -or $v -cnotmatch ('^[0-9a-fA-F]{'+$length+'}$')){throw "Invalid $name"}
}
function Assert-OrdinaryFile([string]$path,[string]$sha,[string]$name){
  Assert-Hex $sha 64 ($name+' SHA')
  if(-not [IO.Path]::IsPathRooted($path)){throw "Expected absolute $name"}
  $f=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($f.PSIsContainer -or (($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw "Invalid or linked $name"
  }
  if((Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash -ine $sha){
    throw "$name changed since review"
  }
  return $f.FullName
}
function Load-Package([string]$path,[string]$label){
  $f=Get-Item -LiteralPath $path -ErrorAction Stop
  if($f.PSIsContainer -or (($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw "Invalid $label manifest"
  }
  $m=Get-Content -LiteralPath $f.FullName -Raw | ConvertFrom-Json
  if($m.schema -cne 'lu-agent01g-built-nsis-sha256-v1' -or $m.genuineInstallerProduced -ne $true){
    throw "Wrong $label manifest schema/attestation"
  }
  Assert-Hex $m.sourceSha 40 "$label source SHA"
  foreach($key in @('installerSha256','appSha256','engineSha256')){
    Assert-Hex $m.$key 64 "$label $key"
  }
  if($m.installerName -cnotmatch '^Lu-AGENT01G-Physical-Windows-CPU-Test-[a-f0-9]{12}\.exe$'){
    throw "Unexpected $label installer identity"
  }
  $installer=Assert-OrdinaryFile (Join-Path $f.DirectoryName $m.installerName) $m.installerSha256 "$label installer"
  return [pscustomobject]@{ Label=$label;Manifest=$m;Installer=$installer }
}
function Run-Exe([string]$exe,[string[]]$arguments){
  $p=Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -ErrorAction Stop
  if(-not $p.WaitForExit($TimeoutSeconds*1000)){
    try{$p.Kill($true);$null=$p.WaitForExit(10000)}catch{}
    throw 'Native installer/uninstaller exceeded bounded timeout; on-disk state uncertain'
  }
  $p.Refresh()
  if($p.ExitCode -ne 0){throw "Native installer/uninstaller exit code $($p.ExitCode)"}
}
function Installed-Version([object]$pkg,[string]$target){
  $null=Assert-OrdinaryFile (Join-Path $target 'lu-desktop.exe') $pkg.Manifest.appSha256 ($pkg.Label+' app')
  $null=Assert-OrdinaryFile (Join-Path $target 'lu-inference\llama-server.exe') $pkg.Manifest.engineSha256 ($pkg.Label+' engine')
}
function Retained-Profile([string]$sentinel,[string]$sha){
  if(-not(Test-Path -LiteralPath $sentinel -PathType Leaf) -or
     (Get-FileHash -LiteralPath $sentinel -Algorithm SHA256).Hash -cne $sha){
    throw 'Isolated profile changed or was deleted'
  }
}
if(Test-Path -LiteralPath $EvidenceDirectory){throw 'Evidence directory already exists'}
$base=Load-Package $BaselineManifestPath 'baseline'
$candidate=Load-Package $CandidateManifestPath 'candidate'
if($base.Manifest.sourceSha -ceq $candidate.Manifest.sourceSha -or
   $base.Manifest.installerSha256 -ceq $candidate.Manifest.installerSha256 -or
   $base.Manifest.appSha256 -ceq $candidate.Manifest.appSha256){
  throw 'Not two distinct source versions with distinct installed executable bytes'
}
if(@(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue).Count -gt 0){
  throw 'Existing Lu process: refuse shared profile and executable collision'
}
$target=Join-Path $env:TEMP ('Lu-R00-versioned-'+[guid]::NewGuid().ToString('N'))
$profile=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
if(Test-Path -LiteralPath $target){throw 'Unexpected already existing install target'}
if(Test-Path -LiteralPath $profile){throw 'Existing test-app profile: refuse overwrite'}
$null=New-Item -ItemType Directory -Path $EvidenceDirectory
$state=[ordered]@{
  schema='lu-r00-two-native-source-versions-v1'
  baselineSourceSha=$base.Manifest.sourceSha
  candidateSourceSha=$candidate.Manifest.sourceSha
  baselineInstallerSha256=$base.Manifest.installerSha256
  candidateInstallerSha256=$candidate.Manifest.installerSha256
  distinctSourcesAndAppBytes=$true
  baselineInstalled='NOT_TESTED';candidateUpgrade='NOT_TESTED'
  rollbackToOriginalBytes='NOT_TESTED';finalUninstall='NOT_TESTED'
  isolatedProfilePreserved='NOT_TESTED'
  realSQLiteSchemaUpgradeAndDowngrade='NOT_TESTED'
  liveGGUFWorkers='NOT_TESTED';physicalGPU='NOT_TESTED'
  stage='preflight';result='FAIL';error=$null;installationStateUncertain=$false
}
try {
  $state.stage='baseline-install'
  Run-Exe $base.Installer @('/S',('/D='+$target))
  Installed-Version $base $target
  $state.baselineInstalled='PASS_REAL_NATIVE_SHA256'
  $null=New-Item -ItemType Directory -Path $profile
  $sentinel=Join-Path $profile 'r00-rollback-test-sentinel.txt'
  [IO.File]::WriteAllText($sentinel,[guid]::NewGuid().ToString('N'))
  $sentinelSha=(Get-FileHash -LiteralPath $sentinel -Algorithm SHA256).Hash
  $state.stage='candidate-upgrade'
  Run-Exe $candidate.Installer @('/S',('/D='+$target))
  Installed-Version $candidate $target
  Retained-Profile $sentinel $sentinelSha
  $state.candidateUpgrade='PASS_REAL_NATIVE_SHA256'
  $state.stage='rollback-uninstall-candidate'
  $uninstaller=Join-Path $target 'UninstallLuAGENT01GPhysicalTest.exe'
  if(-not(Test-Path -LiteralPath $uninstaller -PathType Leaf)){throw 'Missing candidate uninstaller'}
  $current=Get-Item -LiteralPath $uninstaller -Force
  if(($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
    throw 'Unexpected linked uninstaller'
  }
  Run-Exe $uninstaller @('/S')
  if(Test-Path -LiteralPath (Join-Path $target 'lu-desktop.exe')){
    throw 'Candidate application still installed after uninstall'
  }
  Retained-Profile $sentinel $sentinelSha
  $state.stage='restore-original'
  Run-Exe $base.Installer @('/S',('/D='+$target))
  Installed-Version $base $target
  Retained-Profile $sentinel $sentinelSha
  $state.rollbackToOriginalBytes='PASS_REAL_NATIVE_SHA256'
  $state.stage='final-uninstall'
  $originalUninstall=Join-Path $target 'UninstallLuAGENT01GPhysicalTest.exe'
  Run-Exe $originalUninstall @('/S')
  if((Test-Path -LiteralPath (Join-Path $target 'lu-desktop.exe')) -or
     (Test-Path -LiteralPath (Join-Path $target 'lu-inference\llama-server.exe'))){
    throw 'Installed Lu program survived final uninstall'
  }
  Retained-Profile $sentinel $sentinelSha
  $state.finalUninstall='PASS_BINARIES_ABSENT'
  $state.isolatedProfilePreserved='PASS_UNMODIFIED_SHA256'
  $state.result='PASS_TEST_ONLY_TWO_VERSION_APP_BINARY_LIFECYCLE'
}catch {
  $state.error=$_.Exception.Message
  $state.installationStateUncertain=($state.stage -ne 'preflight')
  Write-Warning ('Versioned rollback NOT ACCEPTED: '+$state.error)
}finally {
  $receipt=Join-Path $EvidenceDirectory 'two-version-rollback.json'
  $state|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $receipt -Encoding utf8
  Write-Output ('Native versioned rollback receipt: '+$receipt)
  Write-Output 'Never remove uncertain app installations or test profile automatically'
}
if($state.result -ne 'PASS_TEST_ONLY_TWO_VERSION_APP_BINARY_LIFECYCLE'){exit 2}
