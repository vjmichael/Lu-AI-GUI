# FIN-04W2 effective WFP native acceptance. This is a deliberately strict
# observational test, not an installer or execution grant. Must run after
# approved target AND patched LLDB have really attempted TCP IPv4+IPv6.
# Absent/disabled Filtering Platform auditing or missing positive host
# connectivity is a failure, not a misleading PASS.
[CmdletBinding()]
param(
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$ExactHead,
  [Parameter(Mandatory)][int]$TargetPid,
  [Parameter(Mandatory)][int]$LldbPid,
  [Parameter(Mandatory)][string]$TargetExe,
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$TargetSha256,
  [Parameter(Mandatory)][string]$LldbExe,
  [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$LldbSha256,
  [Parameter(Mandatory)][DateTime]$StartUtc,
  [Parameter(Mandatory)][DateTime]$EndUtc,
  [Parameter(Mandatory)][Net.IPAddress]$IPv4Destination,
  [Parameter(Mandatory)][Net.IPAddress]$IPv6Destination,
  [Parameter(Mandatory)][ValidateRange(1,65535)][int]$Port,
  [Parameter(Mandatory)][string]$EvidenceDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$why){throw ('FIN04W2_EFFECTIVE_WFP_DENIED:'+ $why)}
if($env:OS -ne 'Windows_NT'){Deny 'Windows_required'}
$head=(& git rev-parse HEAD 2>$null)
if($LASTEXITCODE -ne 0 -or $head.Trim() -ine $ExactHead){Deny 'source_head_mismatch'}
$principal=[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if(!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Deny 'independent_admin_audit_required'
}
if($TargetPid -le 0 -or $LldbPid -le 0 -or $TargetPid -eq $LldbPid){
  Deny 'invalid_distinct_processes'
}
if($EndUtc -le $StartUtc -or ($EndUtc-$StartUtc).TotalMinutes -gt 5){
  Deny 'audit_window_invalid'
}
if($IPv4Destination.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork -or
   $IPv6Destination.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetworkV6){
  Deny 'both_IP_families_required'
}
function Observe([int]$pid,[string]$exe,[string]$sha){
  $native=Get-CimInstance Win32_Process -Filter ("ProcessId="+$pid)
  if(!$native -or !$native.ExecutablePath){Deny 'native_Process_missing'}
  $actual=(Resolve-Path -LiteralPath $native.ExecutablePath).Path
  $wanted=(Resolve-Path -LiteralPath $exe).Path
  if(![string]::Equals($actual,$wanted,[StringComparison]::OrdinalIgnoreCase)){
    Deny 'native_process_image_mismatch'
  }
  if((Get-FileHash -LiteralPath $actual -Algorithm SHA256).Hash -ine $sha){
    Deny 'native_image_SHA_mismatch'
  }
  return [IO.Path]::GetFileName($actual)
}
$targetName=Observe $TargetPid $TargetExe $TargetSha256
$lldbName=Observe $LldbPid $LldbExe $LldbSha256
$scm=@(Get-CimInstance Win32_Service |
  Where-Object {$_.Name -in @('LuIdeDebuggerBroker','LuIdeDebugTargetBroker')})
if($scm.Count -ne 2 -or @($scm|Where-Object {$_.State -ne 'Running'}).Count -gt 0 -or
   $scm[0].ProcessId -eq $scm[1].ProcessId){
  Deny 'distinct_running_SCM_pair_missing'
}
function CanConnect([Net.IPAddress]$address,[int]$port){
  $client=New-Object Net.Sockets.TcpClient($address.AddressFamily)
  try{
    $job=$client.ConnectAsync($address,$port)
    if(!$job.Wait(4000) -or !$client.Connected){return $false}
    return $true
  }catch{return $false}finally{$client.Dispose()}
}
if(!(CanConnect $IPv4Destination $Port) -or !(CanConnect $IPv6Destination $Port)){
  Deny 'host_positive_baseline_missing'
}
$events=@(Get-WinEvent -FilterHashtable @{
  LogName='Security';Id=5157;
  StartTime=$StartUtc.ToLocalTime();EndTime=$EndUtc.ToLocalTime()
} -ErrorAction Stop)
$observed=@()
foreach($event in $events){
  [xml]$xml=$event.ToXml()
  $d=@{}
  foreach($item in @($xml.Event.EventData.Data)){
    if($item.Name){$d[[string]$item.Name]=[string]$item.'#text'}
  }
  $raw=[string]$d['ProcessID']
  if($raw -notmatch '^(0x[0-9a-fA-F]+|[0-9]+)$'){continue}
  $pid=if($raw.StartsWith('0x')){[Convert]::ToInt64($raw.Substring(2),16)}
       else{[int64]$raw}
  $role=if($pid -eq $TargetPid){'target'}elseif($pid -eq $LldbPid){'lldb'}else{''}
  if(!$role){continue}
  $name=if($role -eq 'target'){$targetName}else{$lldbName}
  if(!([string]$d['Application']).EndsWith($name,[StringComparison]::OrdinalIgnoreCase) -or
     [string]$d['Protocol'] -ne '6' -or
     [string]$d['DestPort'] -ne [string]$Port -or
     !$d['FilterRTID'] -or [string]$d['FilterRTID'] -eq '0'){continue}
  $ip=[Net.IPAddress]::None
  if(![Net.IPAddress]::TryParse([string]$d['DestAddress'],[ref]$ip)){continue}
  $family=if($ip.Equals($IPv4Destination)){'IPv4'}
          elseif($ip.Equals($IPv6Destination)){'IPv6'}else{''}
  if(!$family){continue}
  $observed+= [pscustomobject]@{
    role=$role;processId=$pid;family=$family;
    filterRuntimeId=[string]$d['FilterRTID'];
    recordId=$event.RecordId;
    observedUtc=$event.TimeCreated.ToUniversalTime().ToString('o')
  }
}
foreach($role in @('target','lldb')){
  foreach($family in @('IPv4','IPv6')){
    if(@($observed|Where-Object {$_.role -eq $role -and $_.family -eq $family}).Count -lt 1){
      Deny ('real_5157_WFP_denial_missing_'+$role+'_'+$family)
    }
  }
}
$null=Observe $TargetPid $TargetExe $TargetSha256
$null=Observe $LldbPid $LldbExe $LldbSha256
$dir=New-Item -Force -ItemType Directory -Path $EvidenceDirectory
$out=Join-Path $dir.FullName 'fin04w2-effective-wfp.json'
$report=[ordered]@{
  schema='fin04w2.effective-WFP-native.v1';exactHead=$ExactHead;
  hostIPv4andIPv6ConnectionPositive=$true;
  realKernel5157EventsForBothProcessesAndFamilies=$true;
  records=$observed;LLDBSourceLineAccepted=$false;
  serviceDeathRecoveryAccepted=$false;installedAcceptance=$false
}
[IO.File]::WriteAllText($out,($report|ConvertTo-Json -Depth 5),
  [Text.UTF8Encoding]::new($false))
Write-Host 'FIN04W2_EFFECTIVE_WFP=PASS_NATIVE_OBSERVED_NOT_RELEASE'
Write-Host 'FIN04W2_TERMINAL_DEBUGGER_ACCEPTANCE=OPEN'
