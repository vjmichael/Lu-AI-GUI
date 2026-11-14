# FIN-04W2 native Windows paired-SCM loss/cancellation acceptance.
# ONE fresh original human-authorized debug Job per invocation. This harness
# NEVER creates approval, changes process ACL, installs a service or WFP rule.
# It intentionally stops/kills only one of the TWO fixed reviewed SCM services
# while the separately-owned target and LLDB are alive, then observes both
# original process identities and their native descendants. This is NOT the
# full original six-phase release/installed/replay proof.
[CmdletBinding()]
param(
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$ExactHead,
  [Parameter(Mandatory)][ValidateSet('LuIdeDebuggerBroker','LuIdeDebugTargetBroker')]
  [string]$ServiceToLose,
  [Parameter(Mandatory)][ValidateSet('SCMStop','HardKill')]
  [string]$LossMode,
  [Parameter(Mandatory)][ValidateRange(1,2147483647)][int]$TargetPid,
  [Parameter(Mandatory)][ValidateRange(1,2147483647)][int]$LldbPid,
  [Parameter(Mandatory)][string]$TargetImage,
  [Parameter(Mandatory)][string]$DebuggerImage,
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$TargetImageSha256,
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$DebuggerImageSha256,
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$OriginalRunningReceiptSha256,
  [Parameter(Mandatory)][string]$EvidenceDirectory,
  [Parameter(Mandatory)][switch]$ExecuteOneUseDestructiveFailure
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$code){throw ('FIN04W2_SERVICE_LOSS_DENIED:'+$code)}
if(!$ExecuteOneUseDestructiveFailure){Deny 'explicit_destructive_test_gate_missing'}
if($env:OS -ne 'Windows_NT'){Deny 'native_Windows_required'}
$head=(& git rev-parse HEAD 2>$null)
if($LASTEXITCODE -ne 0 -or $head.Trim() -ine $ExactHead){Deny 'exact_git_head_mismatch'}
$p=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(!$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Deny 'independent_elevated_operator_required'
}
if($TargetPid -eq $LldbPid){Deny 'shared_target_debugger_pid'}
function NativeIdentity([int]$pid,[string]$expected,[string]$sha){
  $proc=Get-CimInstance Win32_Process -Filter ("ProcessId="+$pid)
  if(!$proc -or !$proc.ExecutablePath){Deny 'missing_reviewed_child_process'}
  $actual=(Resolve-Path -LiteralPath $proc.ExecutablePath).Path
  $pinned=(Resolve-Path -LiteralPath $expected).Path
  if(![string]::Equals($actual,$pinned,[StringComparison]::OrdinalIgnoreCase)){
    Deny 'child_process_image_changed'
  }
  if((Get-FileHash -LiteralPath $actual -Algorithm SHA256).Hash -ine $sha){
    Deny 'child_binary_SHA_changed'
  }
  return [pscustomobject]@{
    pid=$pid;path=$actual;creation=[string]$proc.CreationDate
  }
}
$target=NativeIdentity $TargetPid $TargetImage $TargetImageSha256
$debugger=NativeIdentity $LldbPid $DebuggerImage $DebuggerImageSha256
$services=@(Get-CimInstance Win32_Service |
  Where-Object {$_.Name -in @('LuIdeDebuggerBroker','LuIdeDebugTargetBroker')})
if($services.Count -ne 2 -or $services[0].ProcessId -eq $services[1].ProcessId -or
    @($services|Where-Object {$_.State -ne 'Running'}).Count -gt 0){
  Deny 'distinct_live_restricted_SCM_services_required'
}
$victim=@($services|Where-Object {$_.Name -eq $ServiceToLose})
if($victim.Count -ne 1 -or $victim[0].ProcessId -le 0 -or
   $victim[0].ProcessId -eq $TargetPid -or
   $victim[0].ProcessId -eq $LldbPid){Deny 'SCM_service_identity_mismatch'}
# Distinct service-owned children: no worker process may be the service
# process itself; this is one of the minimal independently observable facts.
foreach($svc in $services){
  if($svc.ProcessId -eq $TargetPid -or $svc.ProcessId -eq $LldbPid){
    Deny 'debug_worker_running_in_service_process'
  }
}
$oldSvcId=[int]$victim[0].ProcessId
$time=[DateTime]::UtcNow
if($LossMode -eq 'HardKill'){
  # Do not accept a caller-selected PID for termination; derive the hard-
  # kill target solely from the exact fixed SCM name observed before action.
  Stop-Process -Id $oldSvcId -Force -ErrorAction Stop
}else{
  Stop-Service -Name $ServiceToLose -Force -ErrorAction Stop
}
$limit=[DateTime]::UtcNow.AddSeconds(12)
do{
  $current=Get-CimInstance Win32_Process -Filter (
    "ProcessId="+$TargetPid+" OR ProcessId="+$LldbPid
  )
  if(!@($current)){break}
  Start-Sleep -Milliseconds 100
}while([DateTime]::UtcNow -lt $limit)
if(@(Get-CimInstance Win32_Process -Filter (
  "ProcessId="+$TargetPid+" OR ProcessId="+$LldbPid
)).Count -ne 0){Deny 'separate_kernel_Job_trees_not_terminated'}
$after=Get-CimInstance Win32_Service -Filter ("Name='"+$ServiceToLose+"'")
if(!$after -or ($LossMode -eq 'HardKill' -and $after.ProcessId -eq $oldSvcId
  -and $after.State -eq 'Running')){Deny 'original_SCM_service_loss_not_observed'}
$dir=New-Item -ItemType Directory -Path $EvidenceDirectory -Force
$out=Join-Path $dir.FullName 'fin04w2-one-service-loss.json'
$record=[ordered]@{
  schema='fin04w2.native-service-loss-process-observation.v1'
  exactSourceHead=$ExactHead;service=$ServiceToLose;mode=$LossMode
  originalRunningReceiptSha256=$OriginalRunningReceiptSha256
  observedBefore=[ordered]@{target=$target;debugger=$debugger;servicePid=$oldSvcId}
  observedLossUtc=$time.ToString('o')
  bothOriginalProcessPidsExitedWithin12s=$true
  nativeJobKernelAccountingReceiptVerified=$false
  independentHostRecoveryJournalVerified=$false
  rejectedRestartReplayVerified=$false
  secondServiceLossFreshAuthorizationVerified=$false
  installedPhysicalCreatorAcceptance=$false
}
[IO.File]::WriteAllText($out,($record|ConvertTo-Json -Depth 6),
  [Text.UTF8Encoding]::new($false))
Write-Host 'FIN04W2_ONE_SCM_LOSS_PROCESS_DEATH=OBSERVED_NATIVE'
Write-Host 'FIN04W2_FULL_RECOVERY_AND_REPLAY_ACCEPTANCE=OPEN'
