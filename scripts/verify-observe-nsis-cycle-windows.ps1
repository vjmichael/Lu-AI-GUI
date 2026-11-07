# P06 native NSIS installed acceptance. Explicitly opt in on a clean,
# disposable Windows test account/VM, NEVER on an existing personal Lu.
# No internet download, fake installer, or self-declared UI/P06 PASS.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$LuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$WorkerSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9 _-]+\.exe$')][string]$UninstallerName,
  [switch]$DisposableMachineConfirmed,
  [switch]$ExerciseNativeUi,
  [ValidateRange(30,1800)][int]$StageTimeoutSeconds=600
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableMachineConfirmed){
  throw 'Explicit disposable native Windows environment approval is required'
}
if($UninstallerName -match '^(?i:Lu|lu-observe-capture-worker)\.exe$'){
  throw 'App or capture worker cannot be supplied as an uninstaller'
}
function Regular-Exe([string]$path){
  if(-not [IO.Path]::IsPathRooted($path)){throw 'Absolute reviewed EXE path required'}
  $f=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($f.PSIsContainer -or $f.Extension -ine '.exe' -or
    (($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'Ordinary native EXE is required (no symlink or reparse)'
  }
  return $f.FullName
}
function Match-Sha([string]$path,[string]$expected){
  $sha=(Get-FileHash -Algorithm SHA256 -LiteralPath $path -ErrorAction Stop).Hash
  if($sha -ine $expected){throw 'Actual native file differs from independently reviewed SHA-256'}
}
$installer=Regular-Exe $InstallerPath
Match-Sha $installer $InstallerSha256
function Invoke-BoundedReviewedExe([string]$path,[string[]]$arguments){
  # Wait on the original exact OS process object, not a name/PID reused
  # later. A hung installer/uninstaller cannot run indefinitely.
  $child=Start-Process -FilePath $path -ArgumentList $arguments -PassThru -ErrorAction Stop
  if(-not $child.WaitForExit($StageTimeoutSeconds*1000)){
    # NSIS may have spawned helpers. Kill the owned process tree; preserve
    # filesystem and registry state as UNCERTAIN for operator inspection.
    try{
      if(-not $child.HasExited){$child.Kill($true)}
      $null=$child.WaitForExit(10000)
    }catch{
      Write-Warning 'Native installer tree termination uncertain'
    }
    throw 'OS-installed package operation TIMED OUT; partial writes are uncertain'
  }
  $child.Refresh()
  if($child.ExitCode -ne 0){throw "Reviewed native package operation exit: $($child.ExitCode)"}
  return $child.ExitCode
}
$registryLocations=@(
  'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
function Lu-Registrations {
  return @(Get-ItemProperty -Path $registryLocations -ErrorAction SilentlyContinue |
    Where-Object {
      $_.PSObject.Properties['DisplayName'] -and
      $_.DisplayName -match '(?i)^Lu(?:\s|$)'
    })
}
if(@(Lu-Registrations).Count -gt 0 -or
   @(Get-Process -Name Lu -ErrorAction SilentlyContinue).Count -gt 0){
  throw 'Existing Lu installation/process detected; use a clean disposable Windows fixture'
}
$target=Join-Path ([IO.Path]::GetTempPath()) ('Lu-P06-NSIS-'+[Guid]::NewGuid().ToString('N'))
if(Test-Path -LiteralPath $target){throw 'Refusing existing install directory'}
$report=[ordered]@{
  schema='Lu-P06-native-nsis-cycle-v1'
  installerSha256=$InstallerSha256.ToLowerInvariant()
  appSha256=$LuSha256.ToLowerInvariant()
  workerSha256=$WorkerSha256.ToLowerInvariant()
  installer='NOT_TESTED'
  installedBinaries='NOT_TESTED'
  uiAutomation='NOT_TESTED'
  uninstaller='NOT_TESTED'
  removal='NOT_TESTED'
  uninstallRegistration='NOT_TESTED'
  wgcPicker='NOT_TESTED'
  pixelRedaction='NOT_TESTED'
  gpuHardwareQuota='NOT_TESTED'
  processTimeoutSeconds=$StageTimeoutSeconds
  reviewedStageProcessExit='NOT_TESTED'
  installationStateUncertain=$false
}
$stage='preflight'
try {
  $stage='installation'
  # Standard NSIS /S, with final /D argument selecting a new test path.
  # If the installer ignores /D, exact target files are absent and fail.
  $null=Invoke-BoundedReviewedExe $installer @('/S',("/D="+$target))
  $report.reviewedStageProcessExit='INSTALL_PROCESS_EXITED'
  $report.installer='PASS'
  $app=Regular-Exe (Join-Path $target 'Lu.exe')
  $worker=Regular-Exe (Join-Path $target 'lu-observe-capture-worker.exe')
  Match-Sha $app $LuSha256
  Match-Sha $worker $WorkerSha256
  $report.installedBinaries='PASS'
  if($ExerciseNativeUi){
    $smoke=Join-Path $PSScriptRoot 'verify-observe-installed-windows.ps1'
    & $smoke -InstalledExe $app -LuSha256 $LuSha256 -WorkerSha256 $WorkerSha256 -InspectObserveUi
    if(-not $?){throw 'Native OS UIA installed smoke did not complete'}
    $report.uiAutomation='INDEPENDENT_RECEIPT_SEPARATE'
  }
  $stage='uninstallation'
  $uninstaller=Regular-Exe (Join-Path $target $UninstallerName)
  $null=Invoke-BoundedReviewedExe $uninstaller @('/S')
  $report.reviewedStageProcessExit='BOTH_REVIEWED_STAGE_PROCESSES_EXITED'
  $report.uninstaller='PASS'
  if((Test-Path -LiteralPath $app) -or (Test-Path -LiteralPath $worker) -or
    @(Get-Process -Name Lu -ErrorAction SilentlyContinue).Count -gt 0){
    throw 'Native Lu binaries or running app survived uninstall'
  }
  $report.removal='PASS'
  if(@(Lu-Registrations).Count -gt 0){throw 'Lu uninstall registry entry remained'}
  $report.uninstallRegistration='PASS'
} catch {
  $report.failureStage=$stage
  $report.failureCategory='NATIVE_INSTALL_CYCLE_NOT_ACCEPTED'
  # A timeout or a returned error after any installer writes is uncertain;
  # never auto-remove an operator's files or imply uninstall succeeded.
  $report.installationStateUncertain=($stage -ne 'preflight')
  throw
} finally {
  # Never auto-delete a failed installation or registry entry. The exact
  # evidence remains for the operator to inspect and clean explicitly.
  $receipt=Join-Path ([IO.Path]::GetTempPath()) (
    'lu-p06-nsis-cycle-'+[Guid]::NewGuid().ToString('N')+'.json')
  $report|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $receipt -Encoding utf8 -NoNewline
  Write-Output "Native NSIS cycle receipt: $receipt"
  Write-Output 'Installer acceptance does not certify WGC, model vision, GPU quota or sensitive-data redaction.'
}
