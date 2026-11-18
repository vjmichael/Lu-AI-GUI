# PLAN-05 actual installed Windows Tauri/Win32 human-action acceptance.
# Only for an explicit disposable account with a native ANSF fixture.
# Refuse success when the OS dialogs/installed UI are not observable.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
  [Parameter(Mandatory=$true)][string]$ProfileDir,
  [Parameter(Mandatory=$true)][string]$SeedReceipt,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
  throw 'PLAN05 installed interactive acceptance requires reviewed disposable Windows'
}
$expectedProfile=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
if([IO.Path]::GetFullPath($ProfileDir).TrimEnd('\') -cne
  [IO.Path]::GetFullPath($expectedProfile).TrimEnd('\')){
  throw 'Refuse non-isolated ANSF profile'
}
$exe=Get-Item -LiteralPath $InstalledExe -Force -ErrorAction Stop
if($exe.PSIsContainer -or (($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) -or
  $exe.Name -cne 'lu-desktop.exe' -or
  (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ine $ExpectedSha256){
  throw 'PLAN05 installed actual Windows executable SHA or identity mismatch'
}
$seed=Get-Content -LiteralPath $SeedReceipt -Raw -Encoding UTF8|ConvertFrom-Json
if($seed.schema -ne 'lu-plan05-installed-seed-fixture-v1' -or
  $seed.source -ne 'TEST_ONLY_SYNTHETIC_MODEL_RAW_JSON_NOT_GGUF' -or
  $seed.initialTaskVersion -ne 1){
  throw 'PLAN05 test-only original source fixture absent'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Refuse to overwrite original installed UI evidence'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Plan05NativeClick {
  [DllImport("user32.dll",SetLastError=true)]
  public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll",SetLastError=true)]
  public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr e);
}
'@
$report=[ordered]@{
  schema='lu-plan05-installed-human-review-v1'
  exactLuSha256=$ExpectedSha256.ToLowerInvariant()
  testFixtureNotGguf=$true
  exactInstalledProcess='NOT_TESTED'
  nativeSettingsWindow='NOT_TESTED'
  savedRegisterUia='NOT_TESTED'
  refusalNativeDialog='NOT_TESTED'
  refusalNoDatabaseWrite='NOT_TESTED'
  approvedNativeDialog='NOT_TESTED'
  approvedCorrectionPersisted='NOT_TESTED'
  secondaryRefusal='NOT_TESTED'
  restartOriginalSameSha='NOT_TESTED'
  restartSettingsReadback='NOT_TESTED'
  noExecutionAuthority='NOT_TESTED'
  realGgufInference='NOT_TESTED_TEST_FIXTURE_ONLY'
  acceptance='NOT_ACCEPTED'
  errors=@()
  evidence=@()
}
$script:process=$null
$script:main=$null
$script:settings=$null
function Read-Original([string]$phase){
  $output=Join-Path $EvidenceDir ("ansf-"+$phase+".json")
  $probeOutput=@(& python scripts/plan05-readonly-ansf-witness.py --profile $ProfileDir --seed $SeedReceipt --output $output)
  if($LASTEXITCODE -ne 0 -or -not(Test-Path $output -PathType Leaf)){
    throw "PLAN05 independent on-disk readback failed: $phase"
  }
  $script:report.evidence+=@($output)
  return (Get-Content -LiteralPath $output -Raw -Encoding UTF8|ConvertFrom-Json)
}
function Save-UIATree([string]$phase,[System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $owner){return}
  try{
    $elements=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition)
    $samples=@()
    for($i=0;$i -lt [Math]::Min(400,$elements.Count);$i++){
      try{
        $a=$elements[$i]
        $samples+=@([ordered]@{
          name=$a.Current.Name;type=$a.Current.ControlType.ProgrammaticName
          processId=[int64]$a.Current.ProcessId
          offscreen=[bool]$a.Current.IsOffscreen
          bounds=[string]$a.Current.BoundingRectangle
        })
      }catch{}
    }
    $path=Join-Path $EvidenceDir ("uia-"+$phase+".json")
    [ordered]@{phase=$phase;owner=$owner.Current.Name;elements=$samples}|
      ConvertTo-Json -Depth 6|Set-Content -LiteralPath $path -Encoding UTF8
    $script:report.evidence+=@($path)
  }catch{ $script:report.errors+=@("UIA diagnostic: "+$_.Exception.Message) }
}
function Find-Named([System.Windows.Automation.AutomationElement]$owner,
    [string]$name,[string]$type='',[int]$timeout=30){
  $deadline=[DateTime]::UtcNow.AddSeconds($timeout)
  do{
    $all=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition)
    foreach($el in $all){
      try{
        if($el.Current.Name -cne $name){continue}
        if($type -and $el.Current.ControlType.ProgrammaticName -cne $type){continue}
        return $el
      }catch{}
    }
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  Save-UIATree ("missing-"+($name -replace '[^a-zA-Z0-9_-]','-')) $owner
  throw "Real installed UIA element missing: $name ($type)"
}
function Find-NativeWindow([string]$title,[int]$timeout=25){
  $deadline=[DateTime]::UtcNow.AddSeconds($timeout)
  do{
    $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Children,
      [System.Windows.Automation.Condition]::TrueCondition)
    foreach($w in $top){
      try{
        if($w.Current.Name -cne $title -or
          $w.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window -or
          [int64]$w.Current.ProcessId -ne [int64]$script:process.Id -or
          [int64]$w.Current.NativeWindowHandle -eq 0 -or
          $w.Current.IsOffscreen){continue}
        return $w
      }catch{}
    }
    Start-Sleep -Milliseconds 200
  }while([DateTime]::UtcNow -lt $deadline)
  Save-UIATree ("missing-"+($title -replace '[^a-zA-Z0-9_-]','-')) $script:settings
  throw "No independently authenticated native Win32 dialog: $title"
}
function Click-Control([System.Windows.Automation.AutomationElement]$control,
    [System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $control -or $null -eq $owner -or
     [int64]$owner.Current.ProcessId -ne [int64]$script:process.Id){
    throw 'PLAN05 cannot click non-owned OS/WebView control'
  }
  $pattern=$null
  if($control.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){
    ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
    return 'UIA_INVOKE'
  }
  $scroll=$null
  if($control.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern,[ref]$scroll)){
    ([System.Windows.Automation.ScrollItemPattern]$scroll).ScrollIntoView()
  }
  try{$control.SetFocus()}catch{}
  if($control.Current.IsOffscreen){throw 'PLAN05 verified UIA control is offscreen'}
  $point=$control.GetClickablePoint()
  $r=$owner.Current.BoundingRectangle
  if($point.X -lt $r.Left -or $point.X -ge $r.Right -or
     $point.Y -lt $r.Top -or $point.Y -ge $r.Bottom){
    throw 'PLAN05 pointer target escapes inspected native window'
  }
  if(-not [Plan05NativeClick]::SetCursorPos([int]$point.X,[int]$point.Y)){
    throw 'PLAN05 failed to position pointer at genuine UIA control'
  }
  [Plan05NativeClick]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [Plan05NativeClick]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
  return 'OS_MOUSE_CLICK_UIA_GROUNDED'
}
function Set-Reviewed-Text([System.Windows.Automation.AutomationElement]$owner,
    [string]$label,[string]$value){
  $el=Find-Named $owner $label 'ControlType.Edit'
  # ValuePattern.SetValue does not reliably fire input/change events in
  # controlled React textboxes, so a visible value can differ from the
  # state actually submitted to Rust. Require real focus + OS keystrokes.
  # The current fixture uses only conservative alphanumeric/underscore text.
  if($el.Current.IsOffscreen){throw "Installed input not reachable: $label"}
  if($value -cnotmatch '^[A-Z0-9_]+
}
function Wait-No-Dialog([string]$title){
  Start-Sleep -Milliseconds 700
  $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    [System.Windows.Automation.Condition]::TrueCondition)
  foreach($w in $top){
    try{
      if($w.Current.Name -ceq $title -and
        [int64]$w.Current.ProcessId -eq [int64]$script:process.Id){
        throw "Previously closed OS modal dialog remains active: $title"
      }
    }catch{
      if($_.Exception.Message -like 'Previously closed*'){throw}
    }
  }
}
function Start-Lu(){
  $script:process=Start-Process -FilePath $exe.FullName -WorkingDirectory $exe.DirectoryName -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(50)
  do{
    $script:process.Refresh()
    if($script:process.HasExited){throw 'Genuine installed Lu process exited before UI window'}
    if($script:process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  if($script:process.MainWindowHandle -eq [IntPtr]::Zero){
    throw 'Installed Lu process failed to create native window'
  }
  $script:main=[System.Windows.Automation.AutomationElement]::FromHandle(
    $script:process.MainWindowHandle)
  if($null -eq $script:main -or
    [int64]$script:main.Current.ProcessId -ne [int64]$script:process.Id){
    throw 'Installed native main UIA process identity mismatch'
  }
  $script:report.exactInstalledProcess='PASS_EXACT_EXE_WIN32_UIA'
}
function Open-Settings(){
  $button=Find-Named $script:main 'Settings' 'ControlType.Button'
  $null=Click-Control $button $script:main
  $script:settings=Find-NativeWindow 'Lu — Settings'
  if($script:settings.Current.NativeWindowHandle -eq $script:main.Current.NativeWindowHandle){
    throw 'Settings is an overlay, not separate native Tauri window'
  }
  try{$script:settings.SetFocus()}catch{}
  $null=Click-Control (Find-Named $script:settings 'Tools & permissions' 'ControlType.Button') $script:settings
  $script:report.nativeSettingsWindow='PASS_DISTINCT_SETTINGS_HWND'
  $null=Click-Control (Find-Named $script:settings 'Reload saved task registers' 'ControlType.Button') $script:settings
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button' 35
  $script:report.savedRegisterUia='PASS_ACTUAL_ANSF_LIST_REOPENED'
}
function Close-Lu(){
  $script:process.Refresh()
  if($script:process.HasExited){throw 'Installed Lu crashed before graceful close'}
  $null=Click-Control (Find-Named $script:settings 'Close settings' 'ControlType.Button') $script:settings
  if(-not $script:process.CloseMainWindow()){throw 'Installed Lu rejected WM_CLOSE'}
  if(-not $script:process.WaitForExit(20000)){throw 'Installed Lu never exited after graceful close'}
  $script:process.Refresh()
  if($script:process.ExitCode -ne 0){throw "Installed Lu exited nonzero: $($script:process.ExitCode)"}
  $script:main=$null;$script:settings=$null;$script:process=$null
}
try{
  $before=Read-Original 'before-installed-start'
  if($before.rowCount -ne 1 -or $before.taskVersion -ne 1 -or
    $before.latestSha256 -cne $seed.initialRevisionSha256){
    throw 'Installed profile not backed by original source-verified fixture revision'
  }
  Start-Lu
  Open-Settings
  Save-UIATree 'first-settings' $script:settings
  $null=Click-Control (Find-Named $script:settings 'Correct task…' 'ControlType.Button') $script:settings
  Set-Reviewed-Text $script:settings 'Reason for correction' 'PLAN05_OPERATOR_REVIEWED_INSTALL_TEST'
  $null=Click-Control (Find-Named $script:settings 'Confirm correction in native dialog…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Confirm human task correction'
  Save-UIATree 'native-correction-refusal-dialog' $dialog
  $null=Click-Control (Find-Named $dialog 'Cancel' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Confirm human task correction'
  $afterRefusal=Read-Original 'after-refused-native-dialog'
  if($afterRefusal.rowCount -ne 1 -or $afterRefusal.latestSha256 -cne $before.latestSha256){
    throw 'OS native refusal changed durable ANSF history'
  }
  $report.refusalNativeDialog='PASS_REAL_OS_CANCEL_CLICK'
  $report.refusalNoDatabaseWrite='PASS_BYTE_IDENTICAL_REGISTER_HEAD'
  $null=Click-Control (Find-Named $script:settings 'Confirm correction in native dialog…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Confirm human task correction'
  Save-UIATree 'native-correction-approval-dialog' $dialog
  $null=Click-Control (Find-Named $dialog 'Confirm review only' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Confirm human task correction'
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  do{
    Start-Sleep -Milliseconds 350
    $approved=Read-Original ("approval-probe-"+([guid]::NewGuid().ToString('N')))
    if($approved.rowCount -eq 2){break}
  }while([DateTime]::UtcNow -lt $deadline)
  if($approved.rowCount -ne 2 -or $approved.taskVersion -ne 2 -or
     $approved.latestEventKind -ne 'human_correction' -or
     $approved.latestCorrectionReason -cne 'PLAN05_OPERATOR_REVIEWED_INSTALL_TEST' -or
     $approved.executionAuthorized){
    throw 'Human approved native dialog did not produce exactly one audited revision'
  }
  $report.approvedNativeDialog='PASS_REAL_OS_HUMAN_REVIEW_CLICK'
  $report.approvedCorrectionPersisted='PASS_VERSION_2_ORIGINALS_PRESERVED'
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button' 15
  $null=Click-Control (Find-Named $script:settings 'Reconcile against selected source…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Recheck original project'
  $null=Click-Control (Find-Named $dialog 'Cancel' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Recheck original project'
  $afterSecondRefusal=Read-Original 'after-refused-source-reconciliation'
  if($afterSecondRefusal.rowCount -ne 2 -or
     $afterSecondRefusal.latestSha256 -cne $approved.latestSha256){
    throw 'Native source review refusal changed existing acceptance history'
  }
  $report.secondaryRefusal='PASS_REAL_OS_CANCEL_NO_SOURCE_REWRITE'
  Close-Lu
  $afterClose=Read-Original 'after-graceful-app-close'
  if($afterClose.latestSha256 -cne $approved.latestSha256){
    throw 'Closing installed app changed original ANSF history'
  }
  Start-Lu
  Open-Settings
  $afterRestart=Read-Original 'after-restart-independent-readback'
  if($afterRestart.rowCount -ne 2 -or $afterRestart.taskVersion -ne 2 -or
     $afterRestart.latestSha256 -cne $approved.latestSha256){
    throw 'True installed-app restart failed to preserve exact immutable task revisions'
  }
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button'
  Save-UIATree 'restarted-settings-register' $script:settings
  $report.restartOriginalSameSha='PASS_ORIGINAL_SQLITE_SHA_CHAIN'
  $report.restartSettingsReadback='PASS_REAL_SETTINGS_UIA_AFTER_RESTART'
  $report.noExecutionAuthority='PASS_ORIGINAL_AND_REVISED_NO_GRANT'
  Close-Lu
  $report.acceptance='PASS_INSTALLED_HUMAN_ACTIONS_AND_RESTART'
}catch{
  $report.errors+=@($_.Exception.ToString())
  Write-Warning ('PLAN05 genuine installed acceptance incomplete: '+$_.Exception.Message)
}finally{
  if($null -ne $script:process){
    try{
      $script:process.Refresh()
      if(-not $script:process.HasExited){
        if(-not $script:process.CloseMainWindow() -or
           -not $script:process.WaitForExit(12000)){
          $script:process.Kill($true)
          $null=$script:process.WaitForExit(5000)
        }
      }
    }catch{$report.errors+=@('Owned Lu process cleanup failed: '+$_.Exception.Message)}
  }
  $report|ConvertTo-Json -Depth 8|
    Set-Content -LiteralPath (Join-Path $EvidenceDir 'plan05-installed-ui.json') -Encoding UTF8
}
if($report.acceptance -ne 'PASS_INSTALLED_HUMAN_ACTIONS_AND_RESTART'){exit 2}
exit 0
){
    throw "Refuse unsafe test keystroke payload for $label"
  }
  $el.SetFocus()
  [System.Windows.Forms.SendKeys]::SendWait('^a')
  [System.Windows.Forms.SendKeys]::SendWait($value)
  Start-Sleep -Milliseconds 350
  $p=$null
  if(-not $el.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$p) -or
     ([System.Windows.Automation.ValuePattern]$p).Current.Value -cne $value){
    throw "Independent UIA readback of OS-typed $label unavailable"
  }
}
function Wait-No-Dialog([string]$title){
  Start-Sleep -Milliseconds 700
  $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    [System.Windows.Automation.Condition]::TrueCondition)
  foreach($w in $top){
    try{
      if($w.Current.Name -ceq $title -and
        [int64]$w.Current.ProcessId -eq [int64]$script:process.Id){
        throw "Previously closed OS modal dialog remains active: $title"
      }
    }catch{
      if($_.Exception.Message -like 'Previously closed*'){throw}
    }
  }
}
function Start-Lu(){
  $script:process=Start-Process -FilePath $exe.FullName -WorkingDirectory $exe.DirectoryName -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(50)
  do{
    $script:process.Refresh()
    if($script:process.HasExited){throw 'Genuine installed Lu process exited before UI window'}
    if($script:process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  if($script:process.MainWindowHandle -eq [IntPtr]::Zero){
    throw 'Installed Lu process failed to create native window'
  }
  $script:main=[System.Windows.Automation.AutomationElement]::FromHandle(
    $script:process.MainWindowHandle)
  if($null -eq $script:main -or
    [int64]$script:main.Current.ProcessId -ne [int64]$script:process.Id){
    throw 'Installed native main UIA process identity mismatch'
  }
  $script:report.exactInstalledProcess='PASS_EXACT_EXE_WIN32_UIA'
}
function Open-Settings(){
  $button=Find-Named $script:main 'Settings' 'ControlType.Button'
  $null=Click-Control $button $script:main
  $script:settings=Find-NativeWindow 'Lu — Settings'
  if($script:settings.Current.NativeWindowHandle -eq $script:main.Current.NativeWindowHandle){
    throw 'Settings is an overlay, not separate native Tauri window'
  }
  try{$script:settings.SetFocus()}catch{}
  $null=Click-Control (Find-Named $script:settings 'Tools & permissions' 'ControlType.Button') $script:settings
  $script:report.nativeSettingsWindow='PASS_DISTINCT_SETTINGS_HWND'
  $null=Click-Control (Find-Named $script:settings 'Reload saved task registers' 'ControlType.Button') $script:settings
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button' 35
  $script:report.savedRegisterUia='PASS_ACTUAL_ANSF_LIST_REOPENED'
}
function Close-Lu(){
  $script:process.Refresh()
  if($script:process.HasExited){throw 'Installed Lu crashed before graceful close'}
  $null=Click-Control (Find-Named $script:settings 'Close settings' 'ControlType.Button') $script:settings
  if(-not $script:process.CloseMainWindow()){throw 'Installed Lu rejected WM_CLOSE'}
  if(-not $script:process.WaitForExit(20000)){throw 'Installed Lu never exited after graceful close'}
  $script:process.Refresh()
  if($script:process.ExitCode -ne 0){throw "Installed Lu exited nonzero: $($script:process.ExitCode)"}
  $script:main=$null;$script:settings=$null;$script:process=$null
}
try{
  $before=Read-Original 'before-installed-start'
  if($before.rowCount -ne 1 -or $before.taskVersion -ne 1 -or
    $before.latestSha256 -cne $seed.initialRevisionSha256){
    throw 'Installed profile not backed by original source-verified fixture revision'
  }
  Start-Lu
  Open-Settings
  Save-UIATree 'first-settings' $script:settings
  $null=Click-Control (Find-Named $script:settings 'Correct task…' 'ControlType.Button') $script:settings
  Set-Reviewed-Text $script:settings 'Reason for correction' 'PLAN05_OPERATOR_REVIEWED_INSTALL_TEST'
  $null=Click-Control (Find-Named $script:settings 'Confirm correction in native dialog…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Confirm human task correction'
  Save-UIATree 'native-correction-refusal-dialog' $dialog
  $null=Click-Control (Find-Named $dialog 'Cancel' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Confirm human task correction'
  $afterRefusal=Read-Original 'after-refused-native-dialog'
  if($afterRefusal.rowCount -ne 1 -or $afterRefusal.latestSha256 -cne $before.latestSha256){
    throw 'OS native refusal changed durable ANSF history'
  }
  $report.refusalNativeDialog='PASS_REAL_OS_CANCEL_CLICK'
  $report.refusalNoDatabaseWrite='PASS_BYTE_IDENTICAL_REGISTER_HEAD'
  $null=Click-Control (Find-Named $script:settings 'Confirm correction in native dialog…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Confirm human task correction'
  Save-UIATree 'native-correction-approval-dialog' $dialog
  $null=Click-Control (Find-Named $dialog 'Confirm review only' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Confirm human task correction'
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  do{
    Start-Sleep -Milliseconds 350
    $approved=Read-Original ("approval-probe-"+([guid]::NewGuid().ToString('N')))
    if($approved.rowCount -eq 2){break}
  }while([DateTime]::UtcNow -lt $deadline)
  if($approved.rowCount -ne 2 -or $approved.taskVersion -ne 2 -or
     $approved.latestEventKind -ne 'human_correction' -or
     $approved.latestCorrectionReason -cne 'PLAN05_OPERATOR_REVIEWED_INSTALL_TEST' -or
     $approved.executionAuthorized){
    throw 'Human approved native dialog did not produce exactly one audited revision'
  }
  $report.approvedNativeDialog='PASS_REAL_OS_HUMAN_REVIEW_CLICK'
  $report.approvedCorrectionPersisted='PASS_VERSION_2_ORIGINALS_PRESERVED'
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button' 15
  $null=Click-Control (Find-Named $script:settings 'Reconcile against selected source…' 'ControlType.Button') $script:settings
  $dialog=Find-NativeWindow 'Lu — Recheck original project'
  $null=Click-Control (Find-Named $dialog 'Cancel' 'ControlType.Button') $dialog
  Wait-No-Dialog 'Lu — Recheck original project'
  $afterSecondRefusal=Read-Original 'after-refused-source-reconciliation'
  if($afterSecondRefusal.rowCount -ne 2 -or
     $afterSecondRefusal.latestSha256 -cne $approved.latestSha256){
    throw 'Native source review refusal changed existing acceptance history'
  }
  $report.secondaryRefusal='PASS_REAL_OS_CANCEL_NO_SOURCE_REWRITE'
  Close-Lu
  $afterClose=Read-Original 'after-graceful-app-close'
  if($afterClose.latestSha256 -cne $approved.latestSha256){
    throw 'Closing installed app changed original ANSF history'
  }
  Start-Lu
  Open-Settings
  $afterRestart=Read-Original 'after-restart-independent-readback'
  if($afterRestart.rowCount -ne 2 -or $afterRestart.taskVersion -ne 2 -or
     $afterRestart.latestSha256 -cne $approved.latestSha256){
    throw 'True installed-app restart failed to preserve exact immutable task revisions'
  }
  $null=Find-Named $script:settings 'Correct task…' 'ControlType.Button'
  Save-UIATree 'restarted-settings-register' $script:settings
  $report.restartOriginalSameSha='PASS_ORIGINAL_SQLITE_SHA_CHAIN'
  $report.restartSettingsReadback='PASS_REAL_SETTINGS_UIA_AFTER_RESTART'
  $report.noExecutionAuthority='PASS_ORIGINAL_AND_REVISED_NO_GRANT'
  Close-Lu
  $report.acceptance='PASS_INSTALLED_HUMAN_ACTIONS_AND_RESTART'
}catch{
  $report.errors+=@($_.Exception.ToString())
  Write-Warning ('PLAN05 genuine installed acceptance incomplete: '+$_.Exception.Message)
}finally{
  if($null -ne $script:process){
    try{
      $script:process.Refresh()
      if(-not $script:process.HasExited){
        if(-not $script:process.CloseMainWindow() -or
           -not $script:process.WaitForExit(12000)){
          $script:process.Kill($true)
          $null=$script:process.WaitForExit(5000)
        }
      }
    }catch{$report.errors+=@('Owned Lu process cleanup failed: '+$_.Exception.Message)}
  }
  $report|ConvertTo-Json -Depth 8|
    Set-Content -LiteralPath (Join-Path $EvidenceDir 'plan05-installed-ui.json') -Encoding UTF8
}
if($report.acceptance -ne 'PASS_INSTALLED_HUMAN_ACTIONS_AND_RESTART'){exit 2}
exit 0
