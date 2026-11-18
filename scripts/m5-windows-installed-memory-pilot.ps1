# M5-03: genuine installed Lu memory replay and TWO real GGUF model sessions.
# Read-only evidence of the installed executable, OS/UIA and worker processes;
# prompts, model outputs and source IDs are user-observed, never fake UIA proof.
# DOES NOT install, download, grant tool authority or alter personal databases.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExeSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$EngineSha256,
  [Parameter(Mandatory=$true)][string]$ModelA,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ModelASha256,
  [Parameter(Mandatory=$true)][string]$ModelB,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ModelBSha256,
  [switch]$RealInteractiveWindowsAndOwnerConsent,
  [switch]$DedicatedDisposableTestProfileConfirmed,
  [ValidateRange(15,120)][int]$WindowTimeoutSeconds=40,
  [string]$EvidenceDir=(Join-Path $env:TEMP ("Lu-M5-physical-"+[guid]::NewGuid().ToString("N")))
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $RealInteractiveWindowsAndOwnerConsent -or
  -not $DedicatedDisposableTestProfileConfirmed){
  throw 'Needs explicit owner consent on a real, interactive Windows desktop with a dedicated disposable Lu test profile'
}
if(-not [Environment]::UserInteractive){
  throw 'Headless CI runner is not a physical interactive acceptance witness'
}
function File-HashIdentity([string]$candidate,[string]$expected,[string]$extension){
  if(-not [IO.Path]::IsPathRooted($candidate)){throw "Review an absolute path"}
  $f=Get-Item -LiteralPath $candidate -Force -ErrorAction Stop
  if($f.PSIsContainer -or $f.Length -lt 1 -or $f.Extension -ine $extension -or
     (($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw "File must be a normal nonempty reviewed $extension file"
  }
  $sha=(Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash
  if($sha -ine $expected){throw "Reviewed file SHA-256 mismatch"}
  return $f.FullName
}
$app=File-HashIdentity $InstalledExe $ExeSha256 '.exe'
if([IO.Path]::GetFileName($app) -notin @('Lu.exe','lu-desktop.exe')){
  throw 'The reviewed EXE must be an installed Lu native binary'
}
$appDir=Split-Path -Parent $app
$engine=File-HashIdentity (Join-Path $appDir 'lu-inference\llama-server.exe') $EngineSha256 '.exe'
$first=File-HashIdentity $ModelA $ModelASha256 '.gguf'
$second=File-HashIdentity $ModelB $ModelBSha256 '.gguf'
if($first -ieq $second -or $ModelASha256 -ieq $ModelBSha256){
  throw 'Two distinct independently reviewed real GGUF model files are required'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence directory already exists; refuse overwrite'}
$null=New-Item -ItemType Directory -Path $EvidenceDir -ErrorAction Stop
$report=[ordered]@{
  schema='lu-m5-installed-local-model-acceptance-v1'
  observedUtc=[DateTime]::UtcNow.ToString('o')
  source='physical-interactive-Windows-OS-and-explicit-operator'
  hashes=[ordered]@{
    installedExe=$ExeSha256.ToLowerInvariant()
    engine=$EngineSha256.ToLowerInvariant()
    firstModel=$ModelASha256.ToLowerInvariant()
    replacementModel=$ModelBSha256.ToLowerInvariant()
  }
  operatingSystem=[Environment]::OSVersion.VersionString
  gpuAdapters=@()
  windows=@()
  modelSessions=@()
  scriptErrors=@()
  externalNativeEffectGrant='NEVER_GRANT_BY_THIS_SCRIPT'
  admittedWindowsUserActivity='OWNER_REPORTED_NOT_AUTOMATIC'
  imageAnswerEvidence='OPERATOR_REPORTED_NOT_PROGRAMMATICALLY_ATTESTED'
  installedAcceptance='INCOMPLETE_NOT_ACCEPTED'
}
try{
  $report.gpuAdapters=@(Get-CimInstance Win32_VideoController -ErrorAction Stop |
    ForEach-Object { [ordered]@{
      name=$_.Name
      driver=$_.DriverVersion
      status=$_.Status
    }})
}catch{
  $report.scriptErrors+=("GPU enumeration unavailable: "+$_.Exception.Message)
}
function App-Window($p){
  $limit=[DateTime]::UtcNow.AddSeconds($WindowTimeoutSeconds)
  do{
    $p.Refresh()
    if($p.HasExited){throw 'Real installed Lu exited before its window appeared'}
    if($p.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $limit)
  if($p.MainWindowHandle -eq [IntPtr]::Zero){throw 'Actual Lu UI did not appear'}
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $ui=[System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  if($null -eq $ui -or [int64]$ui.Current.ProcessId -ne [int64]$p.Id){
    throw 'Windows UIA failed independent installed-application process identity'
  }
  return [ordered]@{
    pid=$p.Id
    uiAProcessId=$ui.Current.ProcessId
    uiAName=$ui.Current.Name
    uiAControlType=$ui.Current.ControlType.ProgrammaticName
    processName=$p.ProcessName
    observedUtc=[DateTime]::UtcNow.ToString('o')
  }
}
function Worker-Snapshot{
  $found=@()
  foreach($p in @(Get-CimInstance Win32_Process -Filter "Name='llama-server.exe'" -ErrorAction Stop)){
    if(-not $p.ExecutablePath -or
       [IO.Path]::GetFullPath([string]$p.ExecutablePath) -ine
       [IO.Path]::GetFullPath($engine)){continue}
    if((Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash -ine $EngineSha256){
      throw 'Installed llama worker changed after initial byte-identity check'
    }
    $found+=([ordered]@{
      pid=[int64]$p.ProcessId
      parentPid=[int64]$p.ParentProcessId
      executable=$p.ExecutablePath
      commandLineSha256=if($p.CommandLine){
        $hasher=[Security.Cryptography.SHA256]::Create()
        try{
          $sha=$hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(
            [string]$p.CommandLine))
          [BitConverter]::ToString($sha).Replace("-","").ToLowerInvariant()
        }finally{$hasher.Dispose()}
      }else{''}
      observedUtc=[DateTime]::UtcNow.ToString('o')
    })
  }
  return @($found)
}
function Answer([string]$question){
  $answer=Read-Host ($question+' [y=yes, n=no, s=skipped]')
  if($answer -match '^(?i:y|yes)$'){return 'OPERATOR_REPORTED_PASS'}
  if($answer -match '^(?i:n|no)$'){return 'OPERATOR_REPORTED_FAIL'}
  return 'NOT_OBSERVED'
}
function Note([string]$label){
  $reply=Read-Host ("Enter "+$label+" (short nonsecret text, blank means not observed)")
  if($reply.Length -gt 600){throw 'Operator evidence input is too long'}
  return $reply
}
function Stop-Lu($process){
  if(-not $process.CloseMainWindow()){
    throw 'Installed Lu window refused graceful close'
  }
  if(-not $process.WaitForExit(40000)){
    throw 'Installed Lu graceful shutdown exceeded 40s'
  }
}
function Run-Session([string]$title,[string]$model,[string]$modelSha){
  $p=$null
  $session=[ordered]@{
    phase=$title
    reviewedModelSha256=$modelSha.ToLowerInvariant()
    windowObservation=$null
    enginePidsAtModelOutput=@()
    enginePidsSurvivingAppClose=@()
    observedStartupMs=$null
    reportedSourceId=''
    reportedModelAnswer=''
    reportedReferenceAccuracy='NOT_EVALUATED'
    scenarios=[ordered]@{}
    osEngineObserved='NOT_OBSERVED'
    osEngineReaped='NOT_OBSERVED'
  }
  try{
    $start=[Diagnostics.Stopwatch]::StartNew()
    $p=Start-Process -FilePath $app -WorkingDirectory $appDir -PassThru
    $session.windowObservation=App-Window $p
    $start.Stop()
    $session.observedStartupMs=[Math]::Round($start.Elapsed.TotalMilliseconds,2)
    Write-Host ("In ACTUAL installed Lu, load the reviewed model from "+$model)
    Write-Host 'Exercise an original sourced task in a new chat; record source ID and model answer.'
    Write-Host 'Do not paste credentials or private chat content into the evidence.'
    $session.reportedSourceId=Note('original ANSF source ID (if visible)')
    $session.reportedModelAnswer=Note('observed answer to repeated task')
    $session.scenarios['realReplyFromLocalModel']=Answer 'Observed genuine non-placeholder model response?'
    $session.scenarios['sourceHistoryAfterRestart']=Answer 'Original ANSF source and correction survived this Lu startup?'
    $session.scenarios['reportedVsVerified']=Answer 'Unverified claim labelled as reported rather than verified?'
    $session.scenarios['conflictingAdvice']=Answer 'Contradictory historical claims flagged for human review?'
    $session.scenarios['staleApplicationVersion']=Answer 'Stale version rejected as current truth?'
    $session.scenarios['hotScrollDuringGeneration']=Answer 'History hot scroll remained responsive during generation?'
    $session.scenarios['interruptedEffectRecovery']=Answer 'Interrupted native effect held uncertain without automatic retry?'
    $session.scenarios['revokedToolAuthority']=Answer 'Revoked or previous-task tool approval could NOT be reused?'
    $session.scenarios['CCMGoalChange']=Answer 'Changed task/goal reviewed without suppressing source evidence?'
    $workers=@(Worker-Snapshot)
    $session.enginePidsAtModelOutput=@($workers | ForEach-Object {$_.pid})
    if($workers.Count -gt 0){$session.osEngineObserved='OS_REVIEWED_PROCESS_SEEN'}
    Stop-Lu $p
    Start-Sleep -Milliseconds 700
    $still=@(Worker-Snapshot | Where-Object {
      $session.enginePidsAtModelOutput -contains $_.pid
    })
    $session.enginePidsSurvivingAppClose=@($still|ForEach-Object {$_.pid})
    if($workers.Count -gt 0 -and $still.Count -eq 0){
      $session.osEngineReaped='OS_PRIOR_WORKERS_NOT_PRESENT'
    }elseif($still.Count -gt 0){
      $session.osEngineReaped='OS_WORKER_LEAK_DETECTED'
    }
  }finally{
    $report.modelSessions+=@($session)
  }
}
try{
  if(@(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue).Count -ne 0){
    throw 'Close any existing Lu desktop sessions before the dedicated M5 test'
  }
  Run-Session 'primary-before-restart' $first $ModelASha256
  Run-Session 'same-primary-after-restart' $first $ModelASha256
  Run-Session 'replacement-after-model-swap' $second $ModelBSha256
  $good=$report.modelSessions.Count -eq 3
  foreach($session in $report.modelSessions){
    if($session.osEngineObserved -ne 'OS_REVIEWED_PROCESS_SEEN' -or
       $session.osEngineReaped -ne 'OS_PRIOR_WORKERS_NOT_PRESENT' -or
       $session.reportedSourceId -notmatch '^ansf:v1:' -or
       [string]::IsNullOrWhiteSpace($session.reportedModelAnswer)){
      $good=$false
    }
    foreach($outcome in $session.scenarios.Values){
      if($outcome -ne 'OPERATOR_REPORTED_PASS'){$good=$false}
    }
  }
  if($good){
    $report.installedAcceptance='OPERATOR_REPORTED_AND_OS_WITNESSED_REQUIRES_INDEPENDENT_REVIEW'
  }
}catch{
  $report.scriptErrors+=($_.Exception.Message)
  $report.installedAcceptance='INCOMPLETE_NOT_ACCEPTED'
}finally{
  $out=Join-Path $EvidenceDir 'm5-physical-windows.json'
  $report|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $out -Encoding utf8
  Write-Output ("Actual M5 Windows witness log: "+$out)
  Write-Output 'Operator text is explicitly NOT machine-verified model correctness; install+Vulkan acceptance still separately required.'
}
if($report.installedAcceptance -ne
  'OPERATOR_REPORTED_AND_OS_WITNESSED_REQUIRES_INDEPENDENT_REVIEW'){exit 2}
