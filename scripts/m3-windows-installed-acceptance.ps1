# Lu M3 installed-app acceptance: REAL local Windows observation, never a
# simulated WebView, fake model, or GitHub Actions compilation result.
# Requires an already installed, hash-reviewed Lu and BOTH real local GGUFs.
# The script does not download, modify private chat databases, or execute tools.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$LuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$EngineSha256,
  [Parameter(Mandatory=$true)][string]$AssistantModel,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$AssistantSha256,
  [Parameter(Mandatory=$true)][string]$EmbeddingModel,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$EmbeddingSha256,
  [switch]$PhysicalInteractiveWindowsConfirmed,
  [switch]$ExerciseManualScenarios,
  [string]$EvidenceDir = (Join-Path $env:TEMP ('Lu-M3-acceptance-'+[guid]::NewGuid().ToString('N')))
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $PhysicalInteractiveWindowsConfirmed){
  throw 'Real interactive Windows with explicit operator approval is required'
}
function VerifiedLocalFile([string]$candidate,[string]$expected,[string]$extension) {
  if(-not [IO.Path]::IsPathRooted($candidate)){throw 'Absolute locally chosen path required'}
  $item=Get-Item -LiteralPath $candidate -Force -ErrorAction Stop
  if($item.PSIsContainer -or $item.Extension -ine $extension -or
    (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) -or
    $item.Length -le 0){throw 'Expected ordinary nonempty file, no reparse points'}
  $hash=(Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash
  if($hash -ine $expected){throw 'Independently reviewed SHA-256 mismatch'}
  return $item.FullName
}
$lu=VerifiedLocalFile $InstalledExe $LuSha256 '.exe'
if([IO.Path]::GetFileName($lu) -notin @('Lu.exe','lu-desktop.exe')){
  throw 'The installed native Lu executable is required'
}
$installation=Split-Path -Parent $lu
$engine=VerifiedLocalFile (Join-Path $installation 'lu-inference\llama-server.exe') $EngineSha256 '.exe'
$assistant=VerifiedLocalFile $AssistantModel $AssistantSha256 '.gguf'
$embedding=VerifiedLocalFile $EmbeddingModel $EmbeddingSha256 '.gguf'
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence directory must not pre-exist'}
$null=New-Item -ItemType Directory -Path $EvidenceDir -Force
$report=[ordered]@{
  schema='lu-m3-installed-windows-evidence-v1'
  observedAtUtc=[DateTime]::UtcNow.ToString('o')
  physicalMachineConfirmed=[bool]$PhysicalInteractiveWindowsConfirmed
  luSha256=$LuSha256.ToLowerInvariant()
  engineSha256=$EngineSha256.ToLowerInvariant()
  assistantModelSha256=$AssistantSha256.ToLowerInvariant()
  embeddingModelSha256=$EmbeddingSha256.ToLowerInvariant()
  installedNativeWindow='NOT_TESTED'
  windowsUIAutomationIdentity='NOT_TESTED'
  historyLibraryUIA='NOT_TESTED'
  genuineEngineProcess='NOT_TESTED'
  hotScrollTwoThousandTurns='NOT_TESTED'
  streamingWhilePaging='NOT_TESTED'
  semanticParaphraseRecall='NOT_TESTED'
  modelTurnAndCancellation='NOT_TESTED'
  restartHistoryAndCCM='NOT_TESTED'
  nativeWorkerReaped='NOT_TESTED'
  sourceAndEffectsAuthority='NOT_TESTED'
  appClosedGracefully='NOT_TESTED'
  acceptance='NOT_ACCEPTED'
  errors=@()
}
$child=$null
$observedEngine=@()
function Get-EngineProcesses {
  $matches=@()
  $rows=@(Get-CimInstance Win32_Process -Filter "Name='llama-server.exe'" -ErrorAction Stop)
  foreach($row in $rows){
    if(-not $row.ExecutablePath){continue}
    $path=[IO.Path]::GetFullPath([string]$row.ExecutablePath)
    if($path -ine [IO.Path]::GetFullPath($engine)){continue}
    $hash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if($hash -ine $EngineSha256){throw 'An observed executable was modified after review'}
    $matches+=([pscustomobject]@{
      pid=[int64]$row.ProcessId
      parentPid=[int64]$row.ParentProcessId
      creationDate=[string]$row.CreationDate
    })
  }
  return @($matches)
}
function Open-Lu {
  $p=Start-Process -FilePath $lu -WorkingDirectory $installation -PassThru -ErrorAction Stop
  $deadline=[DateTime]::UtcNow.AddSeconds(40)
  do{
    $p.Refresh()
    if($p.HasExited){throw 'Real installed Lu terminated before creating a window'}
    if($p.MainWindowHandle -ne [IntPtr]::Zero){return $p}
    Start-Sleep -Milliseconds 300
  }while([DateTime]::UtcNow -lt $deadline)
  throw 'No OS-native top-level Lu window within 40 seconds'
}
function Check-UIA($process){
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $element=[System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
  if($null -eq $element -or [int64]$element.Current.ProcessId -ne [int64]$process.Id){
    throw 'Native UIA window process identity mismatch'
  }
  $report.windowsUIAutomationIdentity='PASS'
  $condition=[System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::NameProperty,'History library')
  $btn=$element.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
  if($null -eq $btn){
    $report.historyLibraryUIA='INCONCLUSIVE_ELEMENT_NOT_EXPOSED'
    return
  }
  # Locating the button via Win32 UIA is a real accessibility-tree observation.
  # Presence does NOT prove months-long scrolling, an actual click or screen reader success.
  $report.historyLibraryUIA='BUTTON_OBSERVED_ONLY'
}
function Close-Lu($process){
  $process.Refresh()
  if($process.HasExited){throw 'Lu unexpectedly crashed before graceful close'}
  if(-not $process.CloseMainWindow()){
    throw 'Lu window did not accept a graceful WM_CLOSE'
  }
  if(-not $process.WaitForExit(30000)){
    throw 'Lu did not exit after graceful close; manual inspection required'
  }
}
function Ask-Operator([string]$label,[string]$field){
  Write-Host ''
  $value=Read-Host ($label+' [y=observed pass,n=observed failure,s=not tested]')
  if($value -match '^(?i:y|yes)$'){$report[$field]='OPERATOR_REPORTED_PASS'}
  elseif($value -match '^(?i:n|no)$'){$report[$field]='OPERATOR_REPORTED_FAIL'}
  else{$report[$field]='INCONCLUSIVE_NOT_TESTED'}
}
try {
  $existing=@(Get-CimInstance Win32_Process -ErrorAction Stop |
    Where-Object { $_.ExecutablePath -and [string]$_.ExecutablePath -ieq $lu })
  if($existing.Count -gt 0){throw 'Close other Lu instances before running independently'}
  $child=Open-Lu
  $report.installedNativeWindow='PASS'
  Check-UIA $child
  if($ExerciseManualScenarios){
    Write-Host 'Use the EXISTING installed Lu window. Do not share private prompts or source contents.'
    Write-Host 'A) Load the reviewed assistant GGUF and obtain a nonempty inference response; cancel mid-generation.'
    Ask-Operator 'Real assistant GGUF text and cancellation both worked' 'modelTurnAndCancellation'
    Write-Host 'B) Through History library, load the reviewed embedding GGUF, index originals and test paraphrase recall.'
    Ask-Operator 'Original-source semantic paraphrase recall works' 'semanticParaphraseRecall'
    Write-Host 'C) Open a 2,048+ turn real ANSF chat and scroll older/newer pages; preserve source anchor and draft.'
    Ask-Operator '2,048+ original-turn variable-height hot scroll is smooth' 'hotScrollTwoThousandTurns'
    Write-Host 'D) Stream while paging and switching chat; verify canceled stream does not overwrite older rows.'
    Ask-Operator 'Streaming, scrolling and switching remained responsive' 'streamingWhilePaging'
    Write-Host 'E) Resume a prior task in a NEW chat; inspect CCM and explicit fresh permissions.'
    Ask-Operator 'Verified CCM/recovery and no inherited tool grant' 'sourceAndEffectsAuthority'
  }
  $observedEngine=@(Get-EngineProcesses)
  if($observedEngine.Count -gt 0){
    $report.genuineEngineProcess='OS_IMAGE_PATH_AND_SHA256_VERIFIED'
  }else{
    $report.genuineEngineProcess='INCONCLUSIVE_NO_WORKER_ALIVE_AT_SAMPLE'
  }
  Close-Lu $child
  $report.appClosedGracefully='PASS'
  Start-Sleep -Milliseconds 1000
  $survivors=@(Get-EngineProcesses | Where-Object {
    $id=[int64]$_.pid
    @($observedEngine | Where-Object {$_.pid -eq $id}).Count -gt 0
  })
  if($survivors.Count -gt 0){
    $report.nativeWorkerReaped='FAIL_SURVIVING_REVIEWED_ENGINE'
    throw 'Observed local inference child survived graceful Lu exit'
  }
  $report.nativeWorkerReaped=if($observedEngine.Count){'PASS_OS_REAP'}else{'INCONCLUSIVE_NO_WORKER_OBSERVED'}
  $child=Open-Lu
  Check-UIA $child
  if($ExerciseManualScenarios){
    Ask-Operator 'After restart: original history, anchors and checkpoint/CCM still present' 'restartHistoryAndCCM'
  }
  Close-Lu $child
  $report.appClosedGracefully='PASS'
  $report.acceptance='EVIDENCE_READY_FOR_INDEPENDENT_REVIEW_NOT_AUTO_APPROVED'
}catch{
  $report.errors=@($report.errors)+@($_.Exception.Message)
  $report.acceptance='NOT_ACCEPTED'
  Write-Warning ('M3 installed-app acceptance failed or incomplete: '+$_.Exception.Message)
}finally{
  $report | ConvertTo-Json -Depth 9 |
    Set-Content -LiteralPath (Join-Path $EvidenceDir 'm3-windows-installed-evidence.json') -Encoding UTF8
  Write-Output ('Evidence file: '+(Join-Path $EvidenceDir 'm3-windows-installed-evidence.json'))
  Write-Output 'Manual answers are explicitly USER-REPORTED, never automated CI PASS.'
}
if($report.acceptance -eq 'NOT_ACCEPTED'){exit 2}
