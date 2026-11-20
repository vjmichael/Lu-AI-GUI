# CP-12B-R7-R1. Local two-instance WINDOWS GRAPHICAL acceptance runner.
# This is an observation tool, not an acceptance certificate or a remote-control agent.
# Run only in a disposable Windows test profile with the source-locked evaluation build.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedExeSha256,
  [Parameter(Mandatory=$true)][string]$ModelFile,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedModelSha256,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [switch]$DisposableTestProfileConfirmed,
  [switch]$AllowControlledCrash
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableTestProfileConfirmed){
  throw 'Requires an explicitly confirmed disposable physical Windows test profile.'
}
if(-not [Environment]::UserInteractive){throw 'This test requires an interactive Windows desktop.'}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence directory must not already exist.'}
if(-not(Test-Path -LiteralPath $InstalledExe -PathType Leaf)){throw 'Installed Lu executable missing.'}
if(-not(Test-Path -LiteralPath $ModelFile -PathType Leaf)){throw 'Local GGUF missing.'}
$exe=(Resolve-Path -LiteralPath $InstalledExe).ProviderPath
$model=(Resolve-Path -LiteralPath $ModelFile).ProviderPath
$exeHash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
$modelHash=(Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash.ToLowerInvariant()
if($exeHash -ne $ExpectedExeSha256.ToLowerInvariant()){throw 'Executable hash mismatch; no process started.'}
if($modelHash -ne $ExpectedModelSha256.ToLowerInvariant()){throw 'GGUF hash mismatch; no process started.'}
if(([IO.File]::GetAttributes($exe) -band [IO.FileAttributes]::ReparsePoint) -ne 0){throw 'Executable must not be a symlink.'}
$preexisting=@(Get-CimInstance Win32_Process -Filter "Name='lu-desktop.exe'" -ErrorAction Stop)
if($preexisting.Count -ne 0){throw 'Close existing Lu sessions before running this isolated test.'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$observations=New-Object 'System.Collections.Generic.List[object]'
$windows=@()
$schema='lu-cp12b-r7-r1-physical-acceptance-v1'
$testStarted=(Get-Date).ToUniversalTime().ToString('o')
function Note([string]$phase,[string]$result,[string]$detail){
  $script:observations.Add([pscustomobject]@{phase=$phase;result=$result;detail=$detail})
  Write-Host "[$result] $phase - $detail"
}
function Observe([string]$phase,[string]$instruction){
  Write-Host "`n$instruction"
  $answer=Read-Host 'Observed on the real Lu UI? (y=yes / n=no / s=not tested)'
  if($answer -match '^[Yy]$'){
    Note $phase 'USER_REPORTED' 'Interactive observation; not an independently automated pass.'
  }elseif($answer -match '^[Nn]$'){
    Note $phase 'FAIL' 'Interactive observer reported failure.'
  }else{
    Note $phase 'NOT_TESTED' 'No reliable observation.'
  }
}
function Get-Processes(){
  @(Get-CimInstance Win32_Process -ErrorAction Stop |
    Where-Object {$_.Name -in @('lu-desktop.exe','llama-server.exe')})
}
# The process inventory is reduced to counts and parent relations. Never save
# command lines, account SIDs, invitation codes, model paths or chat contents.
function Snapshot([string]$phase){
  $rows=@(Get-Processes)
  $ids=@($script:windows | ForEach-Object {$_.Id})
  $apps=@($rows | Where-Object {$_.Name -eq 'lu-desktop.exe' -and $_.ProcessId -in $ids})
  $workers=@($rows | Where-Object {$_.Name -eq 'llama-server.exe'})
  $direct=@($workers | Where-Object {$_.ParentProcessId -in $ids})
  $separate=($direct.Count -ge 2 -and @($direct | Select-Object -ExpandProperty ParentProcessId -Unique).Count -ge 2)
  [pscustomobject]@{phase=$phase;applications=$apps.Count;workerCount=$workers.Count;
      distinctDirectParentWorkers=$separate;exactTwoLaunchedApplications=($apps.Count -eq 2)}
}
# Verify a genuine Windows GUI executable, not the standalone transport test EXE.
function Check-PE([string]$path){
  $stream=[IO.File]::OpenRead($path)
  try{
    $br=New-Object IO.BinaryReader($stream)
    if($br.ReadUInt16() -ne 0x5A4D){throw 'Missing MZ header.'}
    $stream.Position=0x3C;$pe=[int64]$br.ReadUInt32()
    if($pe -lt 64 -or $pe -gt ($stream.Length - 256)){throw 'Invalid PE offset.'}
    $stream.Position=$pe
    if($br.ReadUInt32() -ne 0x4550){throw 'Missing PE header.'}
    $machine=$br.ReadUInt16()
    $stream.Position=$pe+24;$format=$br.ReadUInt16()
    $stream.Position=$pe+24+68;$subsystem=$br.ReadUInt16()
    if($machine -ne 0x8664 -or $format -ne 0x20B -or $subsystem -ne 2){
      throw 'Expected Windows x64 GUI executable; standalone console test binary is not accepted.'
    }
  } finally {$stream.Dispose()}
}
$report=[ordered]@{schema=$schema;task='CP-12B-R7-R1';sourceSha='UNPROVIDED';
  executableSha256=$exeHash;modelSha256=$modelHash;startedUtc=$testStarted;
  installedGui='NOT_TESTED';nativeTwoModel='NOT_TESTED';physicalAccountIsolation='NOT_TESTED';
  conclusion='NOT_ACCEPTED';observations=@();processSnapshots=@();limitations=@(
    'Manual observations are not independently verified passes.',
    'Worker ancestry is a process witness, not proof of correct model output.',
    'This runner does not approve a release or enable a feature in a normal build.'
  )}
try{
  Check-PE $exe
  Note 'exact_installed_identity' 'PASS' 'Pinned SHA-256 and x64 GUI PE header verified.'
  $windows=@((Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru),
             (Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru))
  Start-Sleep -Seconds 3
  foreach($w in $windows){$w.Refresh()}
  if(@($windows | Where-Object {$_.HasExited}).Count -ne 0 -or $windows[0].Id -eq $windows[1].Id){
    Note 'two_independent_lu_processes' 'FAIL' 'One or both instances exited or shared a process.'
    throw 'Two installed Lu processes were not retained.'
  }
  $first=Snapshot 'before_model_loading';$report.processSnapshots=@($first)
  if($first.exactTwoLaunchedApplications){Note 'two_independent_lu_processes' 'PASS' 'Two distinct installed Lu process identities observed.'}
  else{Note 'two_independent_lu_processes' 'FAIL' 'The native process inventory did not retain both launched Lu processes.'}
  Observe 'graphical_privacy_controls' 'In EACH window open Settings > Privacy & Observe. Verify the review controls appear, remain off until requested, and do not expose technical inventory.'
  Observe 'model_load_A_and_B' 'Use the ordinary Models controls to import and explicitly load the pinned local GGUF in BOTH sessions. No automatic model load is permitted.'
  $loaded=Snapshot 'after_explicit_model_loading';$report.processSnapshots+=@($loaded)
  if($loaded.distinctDirectParentWorkers){Note 'two_native_worker_processes' 'PASS' 'Distinct llama-server workers have distinct Lu parent PIDs.'}
  else{Note 'two_native_worker_processes' 'INCOMPLETE' 'Two independent direct model-worker parents not verified; inspect supervised descendant process trees locally.'}
  Observe 'sender_consent_refusal' 'In A request stage review, REFUSE the native permission dialog. Check that no invitation appears.'
  Observe 'receiver_consent_refusal' 'With a fresh sender invitation, REFUSE the receiver dialog in B. Check that the sender invitation remains one-use and no model review runs in B.'
  Observe 'two_model_verified_stage' 'Create a NEW stage invitation in A and accept it in B. Confirm both loaded models performed inference and the checked stage finding is accepted. DO NOT save or paste the invitation in this report.'
  Observe 'sender_review_cancellation' 'Start another stage review in A, press Stop review while inference is active, and confirm no new invitation was published.'
  Observe 'receiver_review_cancellation' 'Start another valid invitation in A, accept it in B, then stop the receiver model review; confirm no result was published.'
  Observe 'revocation_and_replay' 'Create a fresh invitation, stop sharing, and try to receive/replay it in B. Verify both operations are rejected without model inference.'
  Observe 'consumed_invitation_replay' 'After a successful exchange, attempt to reuse the consumed invitation. Verify rejection without inference.'
  if($AllowControlledCrash){
    $confirm=Read-Host 'Force-terminate A for isolated crash recovery? Unsaved test work will be lost (type CRASH)'
    if($confirm -ceq 'CRASH'){
      $a=$windows[0];$a.Refresh()
      if(-not $a.HasExited){Stop-Process -Id $a.Id -Force -ErrorAction Stop}
      Note 'sender_forced_crash' 'PASS' 'Only the originally launched test process was terminated.'
      $windows[0]=Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
      Observe 'sender_crash_stale_invitation' 'After restart, check that a pre-crash invitation cannot be received and that Lu did not auto-load a model.'
      Observe 'sender_explicit_reload_fresh_exchange' 'Explicitly reload model A and perform a NEW, independently approved review with B.'
    }else{Note 'sender_forced_crash' 'NOT_TESTED' 'Crash permission was not given.'}
    $confirm=Read-Host 'Force-terminate B for isolated crash recovery? (type CRASH)'
    if($confirm -ceq 'CRASH'){
      $b=$windows[1];$b.Refresh()
      if(-not $b.HasExited){Stop-Process -Id $b.Id -Force -ErrorAction Stop}
      Note 'receiver_forced_crash' 'PASS' 'Only the originally launched test process was terminated.'
      $windows[1]=Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
      Observe 'receiver_crash_stale_invitation' 'Confirm old and consumed invitations are rejected after receiver restart.'
      Observe 'receiver_explicit_reload_fresh_exchange' 'Explicitly reload model B, then complete another new, independently approved stage review.'
    }else{Note 'receiver_forced_crash' 'NOT_TESTED' 'Crash permission was not given.'}
  }else{
    Note 'crash_recovery' 'NOT_TESTED' 'No consent to force-terminate either test process.'
  }
  Observe 'other_account_real_process_denial' 'Separately use a disposable SECOND Windows account and a real other-user process to attempt receipt of a CURRENT invitation. Confirm kernel denial and no inference; never enter credentials in this runner.'
  $report.processSnapshots+=@(Snapshot 'final')
  $report.installedGui='USER_REPORTED_ONLY'
  $report.nativeTwoModel='USER_REPORTED_ONLY'
  $report.physicalAccountIsolation='USER_REPORTED_ONLY'
  Note 'acceptance_boundary' 'INCOMPLETE' 'Native GUI model outcomes require independent receipts; this runner never self-certifies acceptance.'
} catch {
  Note 'execution' 'FAIL' $_.Exception.Message
} finally {
  $report.observations=@($observations.ToArray())
  $report.completedUtc=(Get-Date).ToUniversalTime().ToString('o')
  $report | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $EvidenceDir 'cp12br7r1-observations.json') -Encoding UTF8
  Write-Host 'Observation receipt saved. Do not share personal messages, invitation codes or GGUF files.'
}
