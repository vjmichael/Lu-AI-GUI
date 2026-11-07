# P06 independent native UIA contract checker. The app/renderer and AI
# cannot supply evidence, select UI actions or mark their own tests passed.
# Read-only: compares only reviewer-pinned button names or Automation IDs;
# never enumerates arbitrary text/values, clicks or captures screenshots.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][int]$ProcessId,
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ArtifactSha256,
  [Parameter(Mandatory=$true)][string]$PlanPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$PlanSha256
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or $ProcessId -le 0){throw 'Native Windows PID required'}
function Checked-File([string]$path,[string]$sha){
  if(-not [IO.Path]::IsPathRooted($path)){throw 'Absolute reviewed file required'}
  $item=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($item.PSIsContainer -or
    (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'Regular hash-reviewed file required'
  }
  if((Get-FileHash -Algorithm SHA256 -LiteralPath $item.FullName).Hash -ine $sha){
    throw 'Artifact/test-plan SHA-256 differs from reviewer approval'
  }
  return $item.FullName
}
$app=Checked-File $InstalledExe $ArtifactSha256
$planFile=Checked-File $PlanPath $PlanSha256
if([IO.Path]::GetFileName($app) -cne 'Lu.exe'){throw 'Expected installed native Lu.exe'}
$plan=Get-Content -LiteralPath $planFile -Raw|ConvertFrom-Json
if($plan.sourceCommit -cnotmatch '^[A-Fa-f0-9]{40}$' -or
   $plan.artifactSha256 -ine $ArtifactSha256){
  throw 'Reviewed source commit and installed binary must match plan'
}
if($plan.schema -ne 1 -or $plan.appId -cne 'lu-desktop' -or
   @($plan.checks).Count -lt 1 -or @($plan.checks).Count -gt 32){
  throw 'Invalid or unbounded independent GUI plan'
}
$ids=@()
foreach($check in @($plan.checks)){
  $hasAutomationId=$null -ne $check.PSObject.Properties['automationId']
  $hasButtonName=$null -ne $check.PSObject.Properties['buttonName']
  if($hasAutomationId -eq $hasButtonName -or
     ($hasAutomationId -and $check.automationId -cnotmatch '^[A-Za-z0-9_-]{1,128}$') -or
     ($hasButtonName -and $check.buttonName -cnotmatch '^[A-Za-z0-9 _-]{1,128}$') -or
     $check.id -cnotmatch '^[A-Za-z0-9_-]{1,64}$' -or
     $check.expectedEnabled -isnot [bool] -or
     $check.expectedOffscreen -isnot [bool]){
    throw 'Invalid reviewed Automation ID or expected UIA properties'
  }
  $ids+=([string]$check.id)
}
if(@($ids|Select-Object -Unique).Count -ne $ids.Count){throw 'Duplicate UIA check IDs'}
$receipt=[ordered]@{
  schema='Lu-P06-native-independent-uia-v1'
  observedAtUtc=[DateTime]::UtcNow.ToString('o')
  artifactSha256=$ArtifactSha256.ToLowerInvariant()
  planSha256=$PlanSha256.ToLowerInvariant()
  sourceCommit=([string]$plan.sourceCommit).ToLowerInvariant()
  verdict='INCONCLUSIVE'
  checks=@()
  visualPixelProof=$false
  nativeEffectProof=$false
  modelImageReceived=$false
}
try {
  $process=Get-Process -Id $ProcessId -ErrorAction Stop
  $birth=$process.StartTime.ToUniversalTime().ToFileTimeUtc()
  if([IO.Path]::GetFullPath([string]$process.Path) -ine [IO.Path]::GetFullPath($app)){
    throw 'Native process executable not the reviewed Lu artifact'
  }
  if($process.MainWindowHandle -eq [IntPtr]::Zero){throw 'No native Lu window'}
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $root=[System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
  if($null -eq $root -or [int]$root.Current.ProcessId -ne $ProcessId){
    throw 'OS UIA root does not belong to reviewed Lu PID'
  }
  $missing=0
  $mismatched=0
  foreach($check in @($plan.checks)){
    $byName=$null -ne $check.PSObject.Properties['buttonName']
    $property=if($byName){
      [System.Windows.Automation.AutomationElement]::NameProperty
    }else{
      [System.Windows.Automation.AutomationElement]::AutomationIdProperty
    }
    $expected=if($byName){[string]$check.buttonName}else{[string]$check.automationId}
    $condition=[System.Windows.Automation.PropertyCondition]::new($property,$expected)
    $found=$root.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,$condition)
    if($found.Count -ne 1){
      $missing++
      $receipt.checks+=@{id=$check.id;status='INCONCLUSIVE_OS_UIA_MISSING_OR_DUPLICATE'}
      continue
    }
    $element=$found.Item(0)
    if([int]$element.Current.ProcessId -ne $ProcessId){
      throw 'Cross-process UIA descendant cannot be verified'
    }
    if($byName -and $element.Current.ControlType.Id -ne
       [System.Windows.Automation.ControlType]::Button.Id){
      $missing++
      $receipt.checks+=@{id=$check.id;status='INCONCLUSIVE_NOT_NATIVE_BUTTON'}
      continue
    }
    if([bool]$element.Current.IsOffscreen -ne [bool]$check.expectedOffscreen -or
       [bool]$element.Current.IsEnabled -ne [bool]$check.expectedEnabled){
      $mismatched++
      $receipt.checks+=@{id=$check.id;status='UIA_ATTRIBUTE_MISMATCH'}
    }else{
      $receipt.checks+=@{id=$check.id;status='UIA_ATTRIBUTE_MATCH'}
    }
  }
  $after=Get-Process -Id $ProcessId -ErrorAction Stop
  if($after.StartTime.ToUniversalTime().ToFileTimeUtc() -ne $birth -or
     [IO.Path]::GetFullPath([string]$after.Path) -ine [IO.Path]::GetFullPath($app)){
    throw 'Native OS source instance changed during independent UIA verification'
  }
  $null=Checked-File $app $ArtifactSha256
  $receipt.verdict=if($mismatched -gt 0){'FAIL_ACCESSIBILITY_CONTRACT'}
    elseif($missing -gt 0){'INCONCLUSIVE'}
    else{'PASS_ACCESSIBILITY_ONLY'}
} catch {
  $receipt.failureCategory='NATIVE_OS_UIA_EVIDENCE_UNAVAILABLE'
  throw
} finally {
  $out=Join-Path ([IO.Path]::GetTempPath()) (
    'lu-p06-independent-uia-'+[Guid]::NewGuid().ToString('N')+'.json')
  $receipt|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $out -Encoding UTF8 -NoNewline
  Write-Output "Native independent UIA evidence: $out"
  Write-Output 'UIA provider evidence is not a screenshot, visual interpretation, native side effect or model PASS.'
}
