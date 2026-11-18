# PERF-01 P0/P0D, original physical Windows owner-operated OS sampling.
# Read-only and explicit: does not start/kill Lu, load GGUF, run tools or grant permissions.
# Captures real process CPU deltas + working set; optional Windows GPU Engine
# performance counters remain separate, NOT whole-GPU utilization or Vulkan proof.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidateRange(1,2147483647)][int]$LuPid,
  [Parameter(Mandatory=$true)][ValidateRange(1,2147483647)][int]$EnginePid,
  [Parameter(Mandatory=$true)][string]$InstalledLuExe,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
  [ValidateRange(5,120)][int]$Samples=10,
  [string]$OriginalHostReceipts
)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){ throw 'PERF01_NATIVE_UNAVAILABLE: requires physical Windows' }
$repo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$head=(& git -C $repo rev-parse HEAD).Trim()
if($LASTEXITCODE -ne 0 -or $head -notmatch '^[a-f0-9]{40}$'){
  throw 'PERF01_SOURCE_HEAD_UNAVAILABLE'
}
$lock=Join-Path $repo 'source-tree.lock.json'
& node (Join-Path $repo 'scripts/verify-source-tree.mjs') | Out-Null
if($LASTEXITCODE -ne 0){throw 'PERF01_SOURCE_INVENTORY_MISMATCH'}
$binary=(Get-Item -LiteralPath $InstalledLuExe -ErrorAction Stop)
if($binary.PSIsContainer -or ($binary.Attributes -band [IO.FileAttributes]::ReparsePoint)
   -or $binary.Extension -ne '.exe'){throw 'PERF01_INSTALLED_EXECUTABLE_UNSAFE'}
$lu=Get-Process -Id $LuPid -ErrorAction Stop
$engine=Get-Process -Id $EnginePid -ErrorAction Stop
$observedLuPath=$null
try{$observedLuPath=$lu.MainModule.FileName}catch{}
if(-not $observedLuPath -or
   -not [string]::Equals(
     [IO.Path]::GetFullPath($observedLuPath),
     [IO.Path]::GetFullPath($binary.FullName),
     [StringComparison]::OrdinalIgnoreCase)){
  throw 'PERF01_LU_PROCESS_NOT_BOUND_TO_EXACT_INSTALLED_EXECUTABLE'
}
$startedLu=$lu.StartTime.ToUniversalTime().ToString('o')
$startedEngine=$engine.StartTime.ToUniversalTime().ToString('o')
$rows=@()
for($i=0;$i -lt $Samples;$i++){
  # Fail closed if either observed process exits/restarts/recycles its PID.
  $lu=Get-Process -Id $LuPid -ErrorAction Stop
  $engine=Get-Process -Id $EnginePid -ErrorAction Stop
  if($lu.StartTime.ToUniversalTime().ToString('o') -ne $startedLu -or
     $engine.StartTime.ToUniversalTime().ToString('o') -ne $startedEngine){
    throw 'PERF01_PROCESS_IDENTITY_CHANGED_DURING_SAMPLE'
  }
  $before=Get-Date
  $luMs=$lu.TotalProcessorTime.TotalMilliseconds
  $engineMs=$engine.TotalProcessorTime.TotalMilliseconds
  $luWorking=[long]$lu.WorkingSet64
  $engineWorking=[long]$engine.WorkingSet64
  $gpuSamples=@()
  $gpuState='OS_GPU_ENGINE_COUNTER_UNAVAILABLE'
  try{
    $c=Get-Counter -Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop
    foreach($v in $c.CounterSamples){
      if($v.Path -match ("(?i)pid_{0}_" -f $EnginePid)){
        $gpuSamples+=@{
          counterPath=$v.Path
          measuredUtilizationPercentage=[double]$v.CookedValue
        }
      }
    }
    $gpuState=if($gpuSamples.Count -gt 0){
      'PDH_GPU_ENGINE_INSTANCE_COUNTERS_OBSERVED'
    }else{'NO_MATCHING_ENGINE_PID_COUNTER'}
  }catch{$gpuState='OS_GPU_ENGINE_COUNTER_UNAVAILABLE'}
  Start-Sleep -Milliseconds 1000
  $luAfter=Get-Process -Id $LuPid -ErrorAction Stop
  $engineAfter=Get-Process -Id $EnginePid -ErrorAction Stop
  if($luAfter.StartTime.ToUniversalTime().ToString('o') -ne $startedLu -or
     $engineAfter.StartTime.ToUniversalTime().ToString('o') -ne $startedEngine){
    throw 'PERF01_PROCESS_IDENTITY_CHANGED_DURING_SAMPLE'
  }
  $actualWallMs=((Get-Date)-$before).TotalMilliseconds
  $rows+=@{
    sampleNumber=$i+1
    sampledUtc=$before.ToUniversalTime().ToString('o')
    observedWallMs=[math]::Round($actualWallMs,2)
    luHostCpuMs=[math]::Round(($luAfter.TotalProcessorTime.TotalMilliseconds-$luMs),2)
    engineCpuMs=[math]::Round(($engineAfter.TotalProcessorTime.TotalMilliseconds-$engineMs),2)
    luWorkingSetBytes=$luWorking
    engineWorkingSetBytes=$engineWorking
    gpuEngineCounterStatus=$gpuState
    gpuEngineInstances=$gpuSamples
  }
}
$hostStartCounts=@()
$hostReceiptsSha256=$null
if(-not [string]::IsNullOrWhiteSpace($OriginalHostReceipts)){
  $file=Get-Item -LiteralPath $OriginalHostReceipts -ErrorAction Stop
  if($file.PSIsContainer -or $file.Length -gt 2MB){throw 'PERF01_HOST_RECEIPTS_OVERSIZE'}
  $hostReceiptsSha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  $receipts=@(Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json)
  foreach($r in $receipts){
    if($r.sourceHead -ne $head){throw 'PERF01_HOST_RECEIPT_SOURCE_HEAD_MISMATCH'}
    if($null -ne $r.observedEngineStartsSinceHostStart){
      $hostStartCounts+=@{
        generationId=$r.generationId
        engineStartCountSinceHostStart=[long]$r.observedEngineStartsSinceHostStart
        engineStartsDuringTurn=$r.observedEngineStartsDuringTurn
      }
    }
  }
}
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$output=Join-Path (Resolve-Path -LiteralPath $EvidenceDirectory).Path 'perf01-windows-native-observed.PENDING.json'
if(Test-Path -LiteralPath $output){throw 'PERF01_REFUSE_OVERWRITE_EXISTING_ORIGINAL'}
$proof=[ordered]@{
  schema='lu-perf01-windows-native-os-metrics-v1'
  sourceHead=$head
  sourceTreeLockSha256=(Get-FileHash -LiteralPath $lock -Algorithm SHA256).Hash.ToLowerInvariant()
  installedExeSha256=(Get-FileHash -LiteralPath $binary.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  installedBinarySourceAttested=$false
  luPid=$LuPid;enginePid=$EnginePid
  luOriginalStartUtc=$startedLu;engineOriginalStartUtc=$startedEngine
  os='Windows_NT'
  cpuMeasure='Original OS process CPU milliseconds per observed wall interval'
  gpuMeasure='Original PDH per-engine process counters, not device-wide GPU utilization'
  originalHostReceiptSha256=$hostReceiptsSha256
  originalHostEngineStartEvents=$hostStartCounts
  samples=$rows
  physicalHumanAcceptance='PENDING_OWNER_AND_INDEPENDENT_REVIEW'
  genuineModelRunVerified=$false
  comparativePerformanceClaim=$false
}
$proof|ConvertTo-Json -Depth 12|
  Set-Content -LiteralPath $output -Encoding utf8 -NoNewline
Write-Output ('PERF01_WINDOWS_ORIGINAL_OS_EVIDENCE='+$output)
Write-Output 'PERF01_ACCEPTANCE=PENDING_OWNER_AND_INDEPENDENT_REVIEW'
