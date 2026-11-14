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
  [Parameter()][int[]]$DescendantPids=@(),
  [Parameter()][switch]$RestartServiceAfterLoss,
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
function NativeIdentity([int]$observedProcessId,[string]$expected,[string]$sha){
  $proc=Get-CimInstance Win32_Process -Filter ("ProcessId="+$observedProcessId)
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
    pid=$observedProcessId;path=$actual;creation=[string]$proc.CreationDate
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
# Descendants must exist BEFORE the destructive test. A supplied PID alone
# proves nothing: walk its actual original kernel-reported parent chain back
# to the one authorized target or trusted LLDB, rejecting cycles and orphans.
if($DescendantPids.Count -gt 32){Deny 'unbounded_descendant_inventory'}
$seen=New-Object 'System.Collections.Generic.HashSet[int]'
$descendants=@()
foreach($childPid in $DescendantPids){
  if($childPid -le 0 -or $childPid -in @($TargetPid,$LldbPid) -or
     !$seen.Add($childPid)){Deny 'invalid_or_duplicate_descendant'}
  $child=Get-CimInstance Win32_Process -Filter ("ProcessId="+$childPid)
  if(!$child -or !$child.ExecutablePath){Deny 'descendant_missing_or_unobservable'}
  $cursor=[int]$child.ParentProcessId
  $visited=New-Object 'System.Collections.Generic.HashSet[int]'
  $linked=$false
  for($depth=0;$depth -lt 16;$depth++){
    if($cursor -eq $TargetPid -or $cursor -eq $LldbPid){$linked=$true;break}
    if($cursor -le 0 -or !$visited.Add($cursor)){break}
    $parent=Get-CimInstance Win32_Process -Filter ("ProcessId="+$cursor)
    if(!$parent){break}
    $cursor=[int]$parent.ParentProcessId
  }
  if(!$linked){Deny 'descendant_not_in_original_worker_ancestry'}
  $descendants+= [pscustomobject]@{
    pid=$childPid;parentPid=[int]$child.ParentProcessId
    creation=[string]$child.CreationDate
    image=(Resolve-Path -LiteralPath $child.ExecutablePath).Path
  }
}
function Get-ObservedAlive {
  foreach($candidate in (@($TargetPid,$LldbPid)+@($DescendantPids))){
    $found=Get-CimInstance Win32_Process -Filter ("ProcessId="+$candidate)
    if($found){$found}
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
# OS-side process exit is only an intermediate observation. Full job-tree
# kill and durable one-use replay must be independently verified by the
# original supervisor/SCM kernel witnesses before any release receipt.
$limit=[DateTime]::UtcNow.AddSeconds(12)
do{
  $current=@(Get-ObservedAlive)
  if($current.Count -eq 0){break}
  Start-Sleep -Milliseconds 100
}while([DateTime]::UtcNow -lt $limit)
if(@(Get-ObservedAlive).Count -ne 0){
  Deny 'original_workers_or_descendants_not_terminated'
}
$after=Get-CimInstance Win32_Service -Filter ("Name='"+$ServiceToLose+"'")
if(!$after -or ($LossMode -eq 'HardKill' -and $after.ProcessId -eq $oldSvcId -and
  $after.State -eq 'Running')){Deny 'original_SCM_service_loss_not_observed'}
# Restart tests only that old workers do not *spontaneously* resurrect.
# A true malicious replay requires a separate original C2 IPC attempt with
# the already-consumed journal; this read-only observer CANNOT certify it.
$restartWithoutAutoReplay=$false
if($RestartServiceAfterLoss){
  $state=Get-CimInstance Win32_Service -Filter ("Name='"+$ServiceToLose+"'")
  if(!$state){Deny 'service_disappeared_after_loss'}
  if($state.State -ne 'Running'){
    Start-Service -Name $ServiceToLose -ErrorAction Stop
  }
  $until=[DateTime]::UtcNow.AddSeconds(15)
  do{
    $fresh=Get-CimInstance Win32_Service -Filter ("Name='"+$ServiceToLose+"'")
    if($fresh -and $fresh.State -eq 'Running' -and
       $fresh.ProcessId -gt 0 -and $fresh.ProcessId -ne $oldSvcId){break}
    Start-Sleep -Milliseconds 100
  }while([DateTime]::UtcNow -lt $until)
  if(!$fresh -or $fresh.State -ne 'Running' -or
     $fresh.ProcessId -le 0 -or $fresh.ProcessId -eq $oldSvcId){
    Deny 'fresh_SCM_service_restart_not_verified'
  }
  if(@(Get-ObservedAlive).Count -ne 0){Deny 'old_worker_resurrected_after_restart'}
  # The original acceptance fixture is exclusive on this host; no new
  # process of either original worker image may silently appear on restart.
  foreach($process in @(Get-CimInstance Win32_Process |
      Where-Object {$_.ExecutablePath -and
        ($_.ExecutablePath -ieq $target.path -or
         $_.ExecutablePath -ieq $debugger.path)})){
    Deny 'new_unapproved_debug_worker_spawned_after_restart'
  }
  $restartWithoutAutoReplay=$true
}
$dir=New-Item -ItemType Directory -Path $EvidenceDirectory -Force
$out=Join-Path $dir.FullName 'fin04w2-one-service-loss.json'
$record=[ordered]@{
  schema='fin04w2.native-service-loss-process-observation.v1'
  exactSourceHead=$ExactHead;service=$ServiceToLose;mode=$LossMode
  originalRunningReceiptSha256=$OriginalRunningReceiptSha256
  observedBefore=[ordered]@{
    target=$target;debugger=$debugger;servicePid=$oldSvcId
    originalDescendantObservations=$descendants
  }
  observedLossUtc=$time.ToString('o')
  bothOriginalProcessPidsExitedWithin12s=$true
  independentlyObservedDescendantPidsExitedWithin12s=($DescendantPids.Count -gt 0)
  freshServiceRestartWithoutSpontaneousWorkerReplay=$restartWithoutAutoReplay
  nativeJobKernelAccountingReceiptVerified=$false
  independentHostRecoveryJournalVerified=$false
  # Only an actual rejected authenticated C2 replay with original fsynced
  # journal can set this; restarting SCM without work is NOT enough.
  rejectedRestartReplayVerified=$false
  secondServiceLossFreshAuthorizationVerified=$false
  installedPhysicalCreatorAcceptance=$false
}
[IO.File]::WriteAllText($out,($record|ConvertTo-Json -Depth 6),
  [Text.UTF8Encoding]::new($false))
Write-Host 'FIN04W2_ONE_SCM_LOSS_PROCESS_DEATH=OBSERVED_NATIVE'
Write-Host 'FIN04W2_FULL_RECOVERY_AND_REPLAY_ACCEPTANCE=OPEN'
