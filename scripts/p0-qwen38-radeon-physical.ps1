# Physical Radeon/Qwen installed acceptance. Operator-initiated and foreground.
# No downloads, shell tool grants, process attachment, model startup or hidden recording.
# Never claims GPU model tensor loading percentage from process residency.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledLuExe,
  [Parameter(Mandatory=$true)][string]$ModelGGUF,
  [string]$MmprojGGUF='',
  [ValidateRange(20,900)][int]$DurationSeconds=180,
  [ValidateRange(1,10)][int]$SampleEverySeconds=2,
  [string]$EvidenceDir=(Join-Path $env:USERPROFILE ('Lu-Qwen38-Physical-'+[guid]::NewGuid().ToString('N'))),
  [switch]$PhysicalWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $PhysicalWindowsConfirmed){
  throw 'This recorder requires your physical Windows machine and explicit confirmation'
}
function Read-LocalFile([string]$path,[string]$suffix){
  if(-not([IO.Path]::IsPathRooted($path))){throw "Absolute $suffix path required"}
  $item=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($item.PSIsContainer -or $item.Extension -ine $suffix -or $item.Length -le 0 -or
      ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){
    throw "Reviewed $suffix must be a nonempty ordinary file"
  }
  return $item
}
$app=Read-LocalFile $InstalledLuExe '.exe'
if($app.Name -notin @('Lu.exe','lu-desktop.exe')){throw 'Expected native Lu executable'}
$model=Read-LocalFile $ModelGGUF '.gguf'
$projector=$null
if($MmprojGGUF){$projector=Read-LocalFile $MmprojGGUF '.gguf'}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence directory already exists'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$installRoot=$app.DirectoryName
$cpuFile=Join-Path $installRoot 'lu-inference\llama-server.exe'
$vkFile=Join-Path $installRoot 'lu-inference\llama-server-vulkan.exe'
$cpu=Read-LocalFile $cpuFile '.exe'
$vk=Read-LocalFile $vkFile '.exe'
$expected=@{}
$expected[$cpu.FullName.ToLowerInvariant()]=(Get-FileHash -LiteralPath $cpu.FullName -Algorithm SHA256).Hash
$expected[$vk.FullName.ToLowerInvariant()]=(Get-FileHash -LiteralPath $vk.FullName -Algorithm SHA256).Hash
$gpus=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,AdapterRAM,PNPDeviceID)
$report=[ordered]@{
  schema='lu-p0-radeon-qwen38-physical-observation-v1'
  startedUtc=[DateTime]::UtcNow.ToString('o')
  machine=[Environment]::MachineName
  physicalWindowsOperatorConfirmed=$true
  deviceDisplayAdapters=$gpus
  installedLuName=$app.Name
  installedLuSha256=(Get-FileHash -LiteralPath $app.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  cpuEngineSha256=$expected[$cpu.FullName.ToLowerInvariant()].ToLowerInvariant()
  vulkanEngineSha256=$expected[$vk.FullName.ToLowerInvariant()].ToLowerInvariant()
  modelGGUFName=$model.Name
  modelGGUFBytes=[int64]$model.Length
  modelGGUFSha256=(Get-FileHash -LiteralPath $model.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  mmprojGGUFName=if($projector){$projector.Name}else{'NOT_SUPPLIED'}
  mmprojGGUFBytes=if($projector){[int64]$projector.Length}else{$null}
  mmprojGGUFSha256=if($projector){(Get-FileHash -LiteralPath $projector.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}else{$null}
  vramSource='Windows GPU Process Memory PDH; may be unavailable; includes runtime allocations'
  ramSource='Windows OS process WorkingSet64 and PrivateMemorySize64'
  ggufTensorLoadedPercent='NOT_MEASURED_BACKEND_TENSOR_TELEMETRY_REQUIRED'
  ownedEngineSamples=0
  samples=@()
  humanWitness=@()
  acceptance='NOT_ACCEPTED_REQUIRES_REVIEW'
}
function Find-OwnedEngine {
  $owned=@()
  $cim=@(Get-CimInstance Win32_Process -Filter "Name='llama-server.exe' OR Name='llama-server-vulkan.exe'" -ErrorAction SilentlyContinue)
  foreach($p in $cim){
    if(-not $p.ExecutablePath){continue}
    $key=[IO.Path]::GetFullPath([string]$p.ExecutablePath).ToLowerInvariant()
    if(-not $expected.ContainsKey($key)){continue}
    if((Get-FileHash -LiteralPath $p.ExecutablePath -Algorithm SHA256).Hash -ine $expected[$key]){continue}
    $owned+=@([pscustomobject]@{pid=[int]$p.ProcessId;backend=[IO.Path]::GetFileName($p.ExecutablePath);started=[string]$p.CreationDate})
  }
  return @($owned)
}
function Gpu-Pid-Bytes([int]$pid){
  $dedicated=$null;$shared=$null
  try{
    $list=@((Get-Counter -Counter '\GPU Process Memory(*)\Local Usage','\GPU Process Memory(*)\Non Local Usage' -ErrorAction Stop).CounterSamples)
    $d=0.0;$v=0.0;$n=0
    foreach($x in $list){
      if($x.Path -notmatch "(?i)pid_$pid(?:_|\\\\)"){continue}
      if($x.CookedValue -lt 0){continue}
      if($x.Path -match '(?i)\\local usage$'){$d+=$x.CookedValue;$n++}
      if($x.Path -match '(?i)\\non local usage$'){$v+=$x.CookedValue;$n++}
    }
    if($n){$dedicated=[int64]$d;$shared=[int64]$v}
  }catch{} # Explicit null/unknown, never infer from total VRAM.
  return @{dedicated=$dedicated;shared=$shared}
}
Write-Host "Lu physical acceptance; evidence: $EvidenceDir"
Write-Host 'Please start Lu, load your Qwen3.8-27B GGUF with the Vulkan backend, and exercise your normal conversations WHILE the foreground sampler runs.'
Write-Host 'Captured evidence is local-only and may contain system hardware/process metadata. Nothing is uploaded.'
$end=[DateTime]::UtcNow.AddSeconds($DurationSeconds)
while([DateTime]::UtcNow -lt $end){
  $now=[DateTime]::UtcNow.ToString('o')
  $rows=@()
  foreach($w in @(Find-OwnedEngine)){
    $native=Get-Process -Id $w.pid -ErrorAction SilentlyContinue
    if($null -eq $native){continue}
    $mem=Gpu-Pid-Bytes $w.pid
    $rows+=@([ordered]@{
      pid=$w.pid;backend=$w.backend;started=$w.started
      workingSetRamBytes=[int64]$native.WorkingSet64
      privateRamBytes=[int64]$native.PrivateMemorySize64
      dedicatedGpuProcessBytes=$mem.dedicated
      sharedGpuProcessBytes=$mem.shared
      measuredTensorPercent=$null
    })
  }
  if($rows.Count){$report.ownedEngineSamples += $rows.Count}
  $report.samples+=@([ordered]@{utc=$now;engineProcesses=@($rows)})
  Start-Sleep -Seconds $SampleEverySeconds
}
$questions=@(
  @('nativeClock','Asked current time; Lu used the native device clock rather than guessing'),
  @('nativeFolder','Asked Lu to inspect a local folder; a native picker and genuine inspected file results appeared'),
  @('reviewedGit','Asked for real Git HEAD; got independently checked SHA with a native tool receipt'),
  @('reviewedMedia','Asked for WAV duration/codec; got independently checked FFprobe result'),
  @('supervisedBrowse','Asked to read an HTTPS page; saw native consent, real browser content and an execution receipt'),
  @('multimodalVision','Attached a test image with compatible MMProj; model identified its actual content'),
  @('queueMessages','Queued messages and observed correct independent completion'),
  @('modelEjection','Ejected Qwen and verified owned inference process terminated')
)
foreach($q in $questions){
  $value=Read-Host ($q[1]+' [y=pass/n=fail/s=not tested]')
  $result=if($value -match '^(?i:y|yes)$'){'OPERATOR_REPORTED_PASS'}
    elseif($value -match '^(?i:n|no)$'){'OPERATOR_REPORTED_FAIL'}
    else{'INCONCLUSIVE_NOT_TESTED'}
  $report.humanWitness+=@([ordered]@{name=$q[0];result=$result;source='human report, not automated oracle'})
}
$report.finishedUtc=[DateTime]::UtcNow.ToString('o')
if($report.ownedEngineSamples -eq 0){$report.acceptance='INCOMPLETE_NO_REVIEWED_ENGINE_OBSERVED'}
$report|ConvertTo-Json -Depth 9|Set-Content -LiteralPath (Join-Path $EvidenceDir 'qwen38-radeon-evidence.json') -Encoding UTF8
Write-Host "Saved local observation: $(Join-Path $EvidenceDir 'qwen38-radeon-evidence.json')"
Write-Host 'No GPU tensor percentage is claimed. Share the redacted receipt/screen captures for independent review.'
