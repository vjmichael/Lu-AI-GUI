# M4-P05: physical installed Windows memory/revision/model-swap acceptance.
# NEVER run automatically or on a personal installation without explicit opt-in.
# Native process/hash/UIA observations are distinct from operator reports.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$AppSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$EngineSha256,
  [Parameter(Mandatory=$true)][string]$PrimaryGGUF,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$PrimarySha256,
  [Parameter(Mandatory=$true)][string]$ReplacementGGUF,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ReplacementSha256,
  [switch]$PhysicalInteractiveWindowsConfirmed,
  [switch]$ManualMemoryTrialConfirmed,
  [string]$EvidenceDir=(Join-Path $env:TEMP ('Lu-M4-physical-'+[guid]::NewGuid().ToString('N')))
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $PhysicalInteractiveWindowsConfirmed -or
  -not $ManualMemoryTrialConfirmed){
  throw 'M4 installed acceptance requires explicit opt-in on a REAL interactive Windows desktop'
}
function Review([string]$path,[string]$digest,[string]$ext){
  if(-not [IO.Path]::IsPathRooted($path)){throw 'Absolute reviewed path required'}
  $item=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($item.PSIsContainer -or $item.Extension -ine $ext -or $item.Length -le 0 -or
    (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'Expected genuine nonempty ordinary reviewed file'
  }
  if((Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash -ine $digest){
    throw 'Genuine installed file or GGUF hash mismatch'
  }
  return $item.FullName
}
$app=Review $InstalledExe $AppSha256 '.exe'
if([IO.Path]::GetFileName($app) -notin @('Lu.exe','lu-desktop.exe')){
  throw 'Use actual installed native Lu exe'
}
$engine=Review (Join-Path (Split-Path -Parent $app) 'lu-inference\llama-server.exe') $EngineSha256 '.exe'
$m1=Review $PrimaryGGUF $PrimarySha256 '.gguf'
$m2=Review $ReplacementGGUF $ReplacementSha256 '.gguf'
if($m1 -ieq $m2){throw 'Distinct real model files required for model-swap recovery'}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence folder must be new'}
$null=New-Item -ItemType Directory -Path $EvidenceDir -Force
$fields=@('sourcePreservedAcrossRestart','reportedClaimNotVerified',
 'contradictionAndHumanCorrection','versionChangedStale',
 'modelSwapRetainsOriginalEvidence','interruptedWorkerRecovery',
 'historicalPermissionNotInherited','CCMReconciliation')
$receipt=[ordered]@{
  schema='lu-m4-physical-windows-v1'
  appSha256=$AppSha256.ToLowerInvariant()
  engineSha256=$EngineSha256.ToLowerInvariant()
  primaryModelSha256=$PrimarySha256.ToLowerInvariant()
  replacementModelSha256=$ReplacementSha256.ToLowerInvariant()
  observedAtUtc=[DateTime]::UtcNow.ToString('o')
  firstUiWindow='NOT_TESTED'
  firstUiProcessIdentity='NOT_TESTED'
  restartUiWindow='NOT_TESTED'
  workerObserved='NOT_TESTED'
  workerReapedAfterClose='NOT_TESTED'
  replacementWorkerObserved='NOT_TESTED'
  replacementWorkerReaped='NOT_TESTED'
  results=[ordered]@{}
  operatorWasSourceOfManualAssertions=$true
  installedAcceptance='NOT_ACCEPTED'
  errors=@()
}
foreach($key in $fields){$receipt.results[$key]='NOT_TESTED'}
function Open-App(){
  $p=Start-Process -FilePath $app -WorkingDirectory (Split-Path -Parent $app) -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(40)
  do{
    $p.Refresh()
    if($p.HasExited){throw 'Real installed Lu process exited before window creation'}
    if($p.MainWindowHandle -ne [IntPtr]::Zero){return $p}
    Start-Sleep -Milliseconds 350
  }while([DateTime]::UtcNow -lt $deadline)
  throw 'No native Lu window appeared'
}
function Attest-Window($process){
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $element=[System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
  if($null -eq $element -or [int64]$element.Current.ProcessId -ne [int64]$process.Id){
    throw 'OS UIA ownership does not match the reviewed native executable'
  }
}
function Engine-Pids(){
  $rows=@(Get-CimInstance Win32_Process -Filter "Name='llama-server.exe'" -ErrorAction Stop)
  $found=@()
  foreach($p in $rows){
    if(-not $p.ExecutablePath){continue}
    if([IO.Path]::GetFullPath([string]$p.ExecutablePath) -ine
      [IO.Path]::GetFullPath($engine)){continue}
    if((Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash -ine $EngineSha256){
      throw 'Observed engine bytes no longer match reviewed provenance'
    }
    $found+=([int64]$p.ProcessId)
  }
  return @($found)
}
function Close-App($p){
  if(-not $p.CloseMainWindow()){throw 'Real Lu window rejected graceful close'}
  if(-not $p.WaitForExit(30000)){throw 'Lu did not exit cleanly within 30 seconds'}
}
function Ask-Actual-Operator([string]$field,[string]$question){
  $answer=Read-Host ($question+' [y=yes,n=failed,s=not tested]')
  if($answer -match '^(?i:y|yes)$'){$receipt.results[$field]='OPERATOR_REPORTED_PASS'}
  elseif($answer -match '^(?i:n|no)$'){$receipt.results[$field]='OPERATOR_REPORTED_FAIL'}
  else{$receipt.results[$field]='INCONCLUSIVE_UNTESTED'}
}
$first=$null
$second=$null
try{
  $already=@(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue)
  if($already.Count){throw 'Close every existing Lu instance first'}
  $first=Open-App
  $receipt.firstUiWindow='PASS_OS_WINDOW'
  Attest-Window $first
  $receipt.firstUiProcessIdentity='PASS_UIA_PROCESS_MATCH'
  Write-Host 'Use Lu UI only. Load the primary reviewed GGUF. Enter source-linked reported/contradictory claims and observe actual generation.'
  Ask-Actual-Operator 'reportedClaimNotVerified' 'Local model identifies a reported claim as unverified?'
  Ask-Actual-Operator 'contradictionAndHumanCorrection' 'Conflict shown, then newer original human correction handled?'
  Ask-Actual-Operator 'versionChangedStale' 'Changing application version labels old claim stale?'
  Ask-Actual-Operator 'interruptedWorkerRecovery' 'Cancellation/interruption returns uncertainty without repeating work?'
  Ask-Actual-Operator 'historicalPermissionNotInherited' 'History did not authorize a new tool action?'
  Ask-Actual-Operator 'CCMReconciliation' 'Checkpoint/context change detected and asked for review?'
  $seen=@(Engine-Pids)
  if($seen.Count){$receipt.workerObserved='PASS_IMAGE_AND_SHA256_AT_SAMPLE'}
  else{$receipt.workerObserved='INCONCLUSIVE_NO_PINNED_WORKER_AT_SAMPLE'}
  Close-App $first
  Start-Sleep -Milliseconds 900
  $survived=@(Engine-Pids|Where-Object {$seen -contains $_})
  $receipt.workerReapedAfterClose=if($seen.Count -and !$survived.Count){
    'PASS_OS_REAP_OBSERVED'
  }else{'INCONCLUSIVE_OR_FAILED_REAP'}
  if($survived.Count){throw 'Reviewed model child survived graceful Lu shutdown'}
  $second=Open-App
  Attest-Window $second
  $receipt.restartUiWindow='PASS_UIA_PROCESS_MATCH'
  Write-Host 'Verify prior original source and correction still exist; then switch to the separately reviewed replacement GGUF.'
  Ask-Actual-Operator 'sourcePreservedAcrossRestart' 'Exact original text and source IDs persisted after restart?'
  Ask-Actual-Operator 'modelSwapRetainsOriginalEvidence' 'After replacing the model, correction and status still remain?'
  $secondSeen=@(Engine-Pids)
  $receipt.replacementWorkerObserved=if($secondSeen.Count){
    'PASS_IMAGE_AND_SHA256_AT_SAMPLE'
  }else{'INCONCLUSIVE_NO_PINNED_WORKER_AT_SAMPLE'}
  Close-App $second
  Start-Sleep -Milliseconds 900
  $secondSurvived=@(Engine-Pids|Where-Object {$secondSeen -contains $_})
  $receipt.replacementWorkerReaped=if($secondSeen.Count -and !$secondSurvived.Count){
    'PASS_OS_REAP_OBSERVED'
  }else{'INCONCLUSIVE_OR_FAILED_REAP'}
  if($secondSurvived.Count){throw 'Replacement-model worker survived Lu restart/exit'}
  $ready=($receipt.workerObserved -eq 'PASS_IMAGE_AND_SHA256_AT_SAMPLE' -and
    $receipt.workerReapedAfterClose -eq 'PASS_OS_REAP_OBSERVED' -and
    $receipt.replacementWorkerObserved -eq 'PASS_IMAGE_AND_SHA256_AT_SAMPLE' -and
    $receipt.replacementWorkerReaped -eq 'PASS_OS_REAP_OBSERVED')
  foreach($key in $fields){
    if($receipt.results[$key] -ne 'OPERATOR_REPORTED_PASS'){$ready=$false}
  }
  $receipt.installedAcceptance=if($ready){
    'EVIDENCE_READY_FOR_INDEPENDENT_REVIEW_NOT_AUTO_APPROVED'
  }else{'INCOMPLETE_NOT_ACCEPTED'}
}catch{
  $receipt.errors=@($receipt.errors)+@($_.Exception.Message)
  $receipt.installedAcceptance='NOT_ACCEPTED'
  Write-Warning ('Lu M4 physical acceptance is open: '+$_.Exception.Message)
}finally{
  $out=Join-Path $EvidenceDir 'm4-installed-memory-evidence.json'
  $receipt|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $out -Encoding UTF8
  Write-Output ('Physical Windows evidence: '+$out)
  Write-Output 'Manual confirmations are OPERATOR_REPORTED; no installer or GGUF accepted by assertion alone.'
}
if($receipt.installedAcceptance -ne
  'EVIDENCE_READY_FOR_INDEPENDENT_REVIEW_NOT_AUTO_APPROVED'){exit 2}
