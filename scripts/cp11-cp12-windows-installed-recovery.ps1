# CP-11A/11B + CP-12A: native Windows installed recovery evidence on a disposable CI desktop.
# Uses the separately qualified C6.2B installer unchanged. Never writes Lu journals,
# injects frontend commands, adjusts the clock, grants model/tool authority or changes production policy.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$AppSha256,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not [Environment]::UserInteractive -or -not $DisposableWindowsConfirmed){
  throw 'A confirmed, interactive disposable native Windows desktop is required'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Refuse existing evidence directory'}
$artifact=Get-Item -LiteralPath $InstallerPath -Force -ErrorAction Stop
if($artifact.PSIsContainer -or $artifact.Extension -ine '.exe' -or ($artifact.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
  (Get-FileHash -LiteralPath $artifact.FullName -Algorithm SHA256).Hash -ine $InstallerSha256){
  throw 'Installer must be the exact original regular SHA-256 verified NSIS executable'
}
if(@(Get-Process -Name Lu,lu-desktop -ErrorAction SilentlyContinue).Count -ne 0){throw 'Another Lu process is running'}
$profile=Join-Path $env:APPDATA 'org.lu.current.devpreview'
if(Test-Path -LiteralPath $profile){throw 'Refuse existing user profile; destructive acceptance prohibited'}
$target=Join-Path $env:TEMP ('lu-cp11-cp12-installed-'+[guid]::NewGuid().ToString('N'))
$marker='CP11-WINDOWS-TEST-'+[guid]::NewGuid().ToString('N')
$null=New-Item -ItemType Directory -Path $EvidenceDir
$report=[ordered]@{
  schema='lu-cp11-cp12-installed-recovery-v1'
  originalInstallerSha256=$InstallerSha256.ToLowerInvariant()
  installedExeSha256=$AppSha256.ToLowerInvariant()
  result='INCOMPLETE'; phases=[ordered]@{}; limitations=@(
    'Disposable hosted Windows, not owner-physical Radeon/real GGUF',
    'No model-based scheduled task, diagnostic peer IPC or two-model recovery was exercised',
    'Only real installed UIA consent and read-only native journal evidence can establish these phases'
  ); errors=@()
}
$script:process=$null
$script:window=$null
$installed=$false
$priorArgs=[Environment]::GetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','Process')
function Record([string]$name,[string]$state){
  $report.phases[$name]=$state
  Write-Output ("CP11_12_PHASE "+$name+" "+$state)
}
function Invoke-NSIS([string]$exe,[string[]]$arguments){
  $p=Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -ErrorAction Stop
  if(-not $p.WaitForExit(120000)){$p.Kill($true);$null=$p.WaitForExit(10000);throw 'NSIS timed out'}
  $p.Refresh()
  if($p.ExitCode -ne 0){throw ('NSIS native exit '+$p.ExitCode)}
}
function Read-Journal([string]$kind){
  $output=@(& python (Join-Path $PSScriptRoot 'cp11-cp12-windows-readonly-journal.py') --profile $profile --kind $kind --tag $marker)
  if($LASTEXITCODE -ne 0 -or $output.Count -ne 1){throw ('Read-only '+$kind+' journal witness unavailable')}
  return ConvertFrom-Json -InputObject $output[0]
}
function Assert([bool]$test,[string]$message){if(-not $test){throw $message}}
# Enumerate the actual WebView2 UIAutomation providers rooted at the installed Win32 HWND.
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuCpRecoveryWin32 {
  public delegate bool ChildCallback(IntPtr hwnd,IntPtr data);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr owner,ChildCallback callback,IntPtr data);
  [DllImport("user32.dll")] public static extern bool IsChild(IntPtr parent,IntPtr candidate);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
}
'@
function Elements([System.Windows.Automation.AutomationElement]$owner){
  $hwnd=[IntPtr]$owner.Current.NativeWindowHandle
  Assert ($hwnd -ne [IntPtr]::Zero) 'Installed owner HWND absent'
  $handles=[System.Collections.Generic.List[IntPtr]]::new()
  $callback=[LuCpRecoveryWin32+ChildCallback]{
    param([IntPtr]$h,[IntPtr]$unused)
    if($handles.Count -lt 128){$handles.Add($h)}
    return $true
  }
  $null=[LuCpRecoveryWin32]::EnumChildWindows($hwnd,$callback,[IntPtr]::Zero)
  $roots=[System.Collections.Generic.List[System.Windows.Automation.AutomationElement]]::new()
  $roots.Add($owner)
  foreach($h in $handles){
    if(-not [LuCpRecoveryWin32]::IsChild($hwnd,$h)){continue}
    try{$child=[System.Windows.Automation.AutomationElement]::FromHandle($h);if($child){$roots.Add($child)}}catch{}
  }
  $found=[System.Collections.Generic.List[System.Windows.Automation.AutomationElement]]::new()
  $seen=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
  foreach($root in $roots){
    try{
      $all=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
      foreach($el in $all){
        $key=($el.GetRuntimeId() -join '.')
        if($key -and $seen.Add($key)){$found.Add($el)}
        if($found.Count -ge 4096){return @($found.ToArray())}
      }
    }catch{}
  }
  return @($found.ToArray())
}
function Wait-Control([string]$name,[bool]$button=$false,[int]$seconds=25){
  $limit=[DateTime]::UtcNow.AddSeconds($seconds)
  do{
    foreach($el in @(Elements $script:window)){
      try{
        if($el.Current.Name -eq $name -and (-not $button -or $el.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button)){
          return $el
        }
      }catch{}
    }
    Start-Sleep -Milliseconds 175
  }while([DateTime]::UtcNow -lt $limit)
  throw ("Installed WebView2 missing authentic UIA control: "+$name)
}
function Click-Control([System.Windows.Automation.AutomationElement]$el,[System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $el -or $null -eq $owner){throw 'UIA action without reviewed native owner'}
  $pat=$null
  if($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pat)){
    ([System.Windows.Automation.InvokePattern]$pat).Invoke()
    return
  }
  Assert (-not $el.Current.IsOffscreen) 'Refuse offscreen UIA action'
  $p=$el.GetClickablePoint();$r=$el.Current.BoundingRectangle;$wr=$owner.Current.BoundingRectangle
  Assert ($r.Width -ge 3 -and $r.Height -ge 3 -and $p.X -ge $r.Left -and $p.X -lt $r.Right -and
    $p.Y -ge $r.Top -and $p.Y -lt $r.Bottom -and $p.X -ge $wr.Left -and
    $p.X -lt $wr.Right -and $p.Y -ge $wr.Top -and $p.Y -lt $wr.Bottom) 'UIA click outside installed owner'
  Assert ([LuCpRecoveryWin32]::SetCursorPos([int]$p.X,[int]$p.Y)) 'Cannot focus genuine UIA element'
  [LuCpRecoveryWin32]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [LuCpRecoveryWin32]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
}
function Press([string]$label){
  $deadline=[DateTime]::UtcNow.AddSeconds(25)
  do{
    $el=Wait-Control $label $true
    try{
      if($el.Current.IsEnabled){Click-Control $el $script:window;return}
    }catch{}
    Start-Sleep -Milliseconds 150
  }while([DateTime]::UtcNow -lt $deadline)
  throw ('Installed WebView button stayed disabled: '+$label)
}
function Set-Field([string]$label,[string]$value){
  $el=Wait-Control $label
  $pat=$null
  if($el.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pat)){
    ([System.Windows.Automation.ValuePattern]$pat).SetValue($value);return
  }
  Click-Control $el $script:window
  [System.Windows.Forms.SendKeys]::SendWait('^a')
  [System.Windows.Forms.SendKeys]::SendWait($value)
}
# The existing installed Windows GGUF witness documents the genuine TaskDialog
# CCPushButton provider: its native commands can appear as Pane, not Button.
# Accept that exception only for the original exact names and native command IDs.
# Never broaden ordinary WebView selection or select an unowned generic Pane.
function Test-NativeChoiceIdentity($current,[string]$choice,[int]$ownerPid,[IntPtr]$ownerHwnd){
  if($current.Name -cne $choice -or [int]$current.ProcessId -ne $ownerPid -or
     [int64]$current.NativeWindowHandle -eq 0 -or -not $current.IsEnabled -or
     $current.IsOffscreen -or
     -not [LuCpRecoveryWin32]::IsChild($ownerHwnd,[IntPtr][int64]$current.NativeWindowHandle)){
    return $false
  }
  if($current.ControlType -eq [System.Windows.Automation.ControlType]::Button){return $true}
  $expectedId=switch -CaseSensitive ($choice){
    'Run checks'{'CommandButton_1000'}
    'Schedule reminder'{'CommandButton_1000'}
    'Cancel'{'CommandButton_1001'}
    'Decline'{'CommandButton_1001'}
    default{return $false}
  }
  return ($current.ControlType -eq [System.Windows.Automation.ControlType]::Pane -and
    $current.ClassName -ceq 'CCPushButton' -and
    $current.AutomationId -ceq $expectedId)
}
function Native-Review([string]$title,[string]$choice){
  $limit=[DateTime]::UtcNow.AddSeconds(20)
  $dlg=$null
  do{
    $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
    foreach($candidate in $top){
      try{
        if($candidate.Current.Name -ceq $title -and [int]$candidate.Current.ProcessId -eq [int]$script:process.Id -and
          $candidate.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
          $candidate.Current.NativeWindowHandle -ne 0 -and
          [int64]$candidate.Current.NativeWindowHandle -ne [int64]$script:window.Current.NativeWindowHandle -and
          -not $candidate.Current.IsOffscreen){
          $dlg=$candidate;break
        }
      }catch{}
    }
    if($dlg){break}
    Start-Sleep -Milliseconds 150
  }while([DateTime]::UtcNow -lt $limit)
  if(-not $dlg){throw ('Native owner-bound approval dialog absent: '+$title)}
  $ownerHwnd=[IntPtr][int64]$dlg.Current.NativeWindowHandle
  $chosen=$null
  $choiceLimit=[DateTime]::UtcNow.AddSeconds(10)
  do{
    # Enumerate the original native child HWND providers, as in the already
    # qualified installed Windows UIA smoke. Exact names and OS ownership
    # remain mandatory; a missing consent button still fails acceptance.
    $matches=[System.Collections.Generic.List[System.Windows.Automation.AutomationElement]]::new()
    foreach($el in @(Elements $dlg)){
      try{
        if(Test-NativeChoiceIdentity $el.Current $choice $script:process.Id $ownerHwnd){
          $matches.Add($el)
        }
      }catch{}
    }
    if($matches.Count -gt 1){throw ('Ambiguous original native approval choice: '+$choice)}
    if($matches.Count -eq 1){$chosen=$matches[0];break}
    Start-Sleep -Milliseconds 150
  }while([DateTime]::UtcNow -lt $choiceLimit)
  if(-not $chosen){throw ('Original native approval choice unavailable: '+$choice)}
  if(-not(Test-NativeChoiceIdentity $chosen.Current $choice $script:process.Id $ownerHwnd)){
    throw 'Native approval control identity changed before input'
  }
  Click-Control $chosen $dlg
}
function Start-Lu(){
  $script:process=Start-Process -FilePath $script:app -WorkingDirectory $target -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(45)
  do{
    $script:process.Refresh()
    Assert (-not $script:process.HasExited) 'Installed Lu exited before GUI'
    if($script:process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 200
  }while([DateTime]::UtcNow -lt $deadline)
  Assert ($script:process.MainWindowHandle -ne [IntPtr]::Zero) 'Installed Lu did not create a real window'
  $script:window=[System.Windows.Automation.AutomationElement]::FromHandle($script:process.MainWindowHandle)
  Assert ($null -ne $script:window -and [int]$script:window.Current.ProcessId -eq [int]$script:process.Id) 'Installed GUI owner mismatch'
  try{$script:window.SetFocus()}catch{}
  $null=Wait-Control 'Settings' $true
}
function Stop-Lu([bool]$force){
  if($null -eq $script:process){return}
  $script:process.Refresh()
  if(-not $script:process.HasExited){
    if($force){$script:process.Kill($true)}
    elseif(-not $script:process.CloseMainWindow()){$script:process.Kill($true)}
    if(-not $script:process.WaitForExit(15000)){$script:process.Kill($true);$null=$script:process.WaitForExit(10000)}
  }
  $script:process=$null;$script:window=$null
}
function Privacy(){Press 'Settings';Press 'Privacy & Observe';$null=Wait-Control 'Run checks…' $true}
function Wait-Check([string]$desired,[int]$seconds=75){
  $deadline=[DateTime]::UtcNow.AddSeconds($seconds)
  do{
    $s=Read-Journal 'check'
    if($s.present -and $s.state -eq $desired){return $s}
    Start-Sleep -Milliseconds 100
  }while([DateTime]::UtcNow -lt $deadline)
  throw ('Read-only installed check did not reach state '+$desired)
}
try{
  if(-not [string]::IsNullOrEmpty($priorArgs)){throw 'Unexpected WebView2 argument override'}
  [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','--force-renderer-accessibility','Process')
  Invoke-NSIS $artifact.FullName @('/S',('/D='+$target))
  $installed=$true
  $script:app=Join-Path $target 'Lu.exe'
  if(-not(Test-Path -LiteralPath $script:app)){$script:app=Join-Path $target 'lu-desktop.exe'}
  $exe=Get-Item -LiteralPath $script:app -Force -ErrorAction Stop
  Assert (-not $exe.PSIsContainer -and ($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and
    (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ieq $AppSha256) 'Installed app differs from pinned C6.2B binary'
  Start-Lu
  Assert ((Read-Journal 'check').present -eq $false) 'Unexpected pre-existing check'
  Record 'fresh_installed_process_and_empty_diagnostic' 'PASS'
  Privacy
  Press 'Run checks…'
  Native-Review 'Lu — Check Lu' 'Cancel'
  Assert ((Read-Journal 'check').present -eq $false) 'Declined check persisted authority or report'
  Record 'native_decline_no_automatic_checks' 'PASS'
  Press 'Run checks…'
  Native-Review 'Lu — Check Lu' 'Run checks'
  $finished=Wait-Check 'complete'
  Assert ($finished.expanded -eq $true -and $finished.modelSuite -eq $false -and $finished.count -eq 5) 'Incorrect actual installed sample suite'
  Assert ((@($finished.kinds) -join ',') -ceq 'reading,saved_history,search_speed,document_windows,search_scale') 'Installed sample order mismatch'
  Assert ((@($finished.verified) -join ',') -ceq '3,4,16,16,64') 'Sample independent comparison count mismatch'
  Assert (@($finished.passed | Where-Object {$_ -ne $true}).Count -eq 0) 'Unverified installed result'
  Record 'native_approved_synthetic_suite_exact_report' 'PASS'
  # A new one-shot check may replace a complete record; only its own run can be interrupted.
  Press 'Run checks…'
  Native-Review 'Lu — Check Lu' 'Run checks'
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  $inflight=$null
  do{
    $v=Read-Journal 'check'
    if($v.present -and $v.state -eq 'running' -and $v.count -ge 1){
      $inflight=$v;break
    }
    Start-Sleep -Milliseconds 25
  }while([DateTime]::UtcNow -lt $deadline)
  if(-not $inflight){throw 'No authentic running sample prefix; do not manufacture abrupt recovery'}
  Stop-Lu $true
  $before=Read-Journal 'check'
  Assert ($before.runHash -ceq $inflight.runHash -and $before.state -eq 'running') 'Interruption race completed before exact original process death'
  Record 'real_process_kill_during_saved_check_prefix' 'PASS'
  Start-Lu
  $recovered=Wait-Check 'interrupted'
  Assert ($recovered.runHash -ceq $inflight.runHash -and $recovered.prefixHash -ceq $before.prefixHash -and
    $recovered.count -eq $before.count) 'Cold restart fabricated or lost check prefix'
  Start-Sleep -Seconds 2
  $noReplay=Read-Journal 'check'
  Assert ($noReplay.state -eq 'interrupted' -and $noReplay.prefixHash -ceq $recovered.prefixHash) 'Unapproved check replay after cold startup'
  Record 'cold_check_recovery_exact_prefix_no_replay' 'PASS'
  Assert (@(Get-Process -Name llama-server -ErrorAction SilentlyContinue).Count -eq 0) 'Cold recovery started an unauthorized model worker'
  Privacy
  $null=Wait-Control 'Last check was interrupted. Unfinished checks were not verified. You can approve a fresh check; earlier sample files are preserved.'
  Press 'Review recent response issues…'
  $null=Wait-Control 'Local troubleshooting report ready.'
  $diagnostic=Read-Journal 'traces'
  Assert ($diagnostic.present -eq $true -and $diagnostic.count -ge 0) 'Original diagnostic archive unavailable after cold recovery'
  Record 'plain_diagnostic_status_after_cold_recovery' 'PASS_READONLY_LOCAL'
  Press 'Chat'
  Press 'New Chat'
  Press 'Tasks'
  Press 'Reminders'
  $null=Wait-Control 'Reminder conversation'
  Set-Field 'Reminder text' $marker
  Set-Field 'Reminder delay in minutes' '1'
  Press 'Review reminder…'
  Native-Review 'Lu — Approve reminder' 'Decline'
  Assert ((Read-Journal 'reminder').present -eq $false) 'Refused reminder persisted a schedule'
  Record 'native_reminder_refusal_does_not_schedule' 'PASS'
  Press 'Review reminder…'
  Native-Review 'Lu — Approve reminder' 'Schedule reminder'
  $deadline=[DateTime]::UtcNow.AddSeconds(12)
  $pending=$null
  do{
    $v=Read-Journal 'reminder'
    if($v.present -and $v.state -eq 'ACTIVE'){$pending=$v;break}
    Start-Sleep -Milliseconds 125
  }while([DateTime]::UtcNow -lt $deadline)
  if(-not $pending -or $pending.firedCount -ne 0 -or $pending.unreadCount -ne 0 -or
    $pending.repeatSeconds -ne $null){throw 'Actual approved one-shot reminder not independently verified'}
  Record 'native_approved_reminder_atomic_owner_schedule' 'PASS'
  Stop-Lu $false
  $closed=Read-Journal 'reminder'
  Assert ($closed.idHash -ceq $pending.idHash -and $closed.firedCount -eq 0) 'Schedule changed after Lu closed'
  $limit=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
  Assert (($pending.nextDue-$limit) -gt 0 -and ($pending.nextDue-$limit) -le 70) 'Unexpected reminder due interval'
  $remaining=[int]($pending.nextDue-$limit+2)
  if($remaining -gt 0){Start-Sleep -Seconds $remaining}
  $stillClosed=Read-Journal 'reminder'
  Assert ($stillClosed.state -eq 'ACTIVE' -and $stillClosed.firedCount -eq 0) 'Closed Lu executed a recurring background service'
  Record 'closed_application_did_not_fire_reminder' 'PASS'
  Start-Lu
  $deadline=[DateTime]::UtcNow.AddSeconds(25)
  $due=$null
  do{
    $v=Read-Journal 'reminder'
    if($v.present -and $v.state -eq 'COMPLETED'){$due=$v;break}
    Start-Sleep -Milliseconds 200
  }while([DateTime]::UtcNow -lt $deadline)
  Assert ($null -ne $due -and $due.idHash -ceq $pending.idHash -and
    $due.ownerHash -ceq $pending.ownerHash -and $due.nextDue -eq $pending.nextDue -and
    $due.firedCount -eq 1 -and $due.unreadCount -eq 1 -and $due.lastDue -eq $pending.nextDue) 'Cold recovery missed reminder or duplicated occurrence'
  $null=Wait-Control 'Reminders due (1)' $true
  Assert (@(Get-Process -Name llama-server -ErrorAction SilentlyContinue).Count -eq 0) 'Inert reminder started a model worker'
  Record 'missed_reminder_single_fire_on_cold_restart' 'PASS'
  Stop-Lu $false
  Start-Lu
  $again=Read-Journal 'reminder'
  Assert ($again.idHash -ceq $due.idHash -and $again.firedCount -eq 1 -and
    $again.unreadCount -eq 1 -and $again.state -eq 'COMPLETED') 'Second restart duplicated delivered reminder'
  Record 'second_restart_durable_exactly_once_notice' 'PASS'
  $report.result='PASS_SCOPED_HOSTED_WINDOWS_CP11A_CP11B_CP12A'
}catch{
  $report.errors=@($report.errors)+@($_.Exception.Message)
  Write-Warning ('Installed CP11/12 acceptance is INCOMPLETE: '+$_.Exception.Message)
}finally{
  try{Stop-Lu $true}catch{$report.errors+=@('Could not reap original installed Lu process')}
  [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS',$priorArgs,'Process')
  if($installed){
    $uninstall=Join-Path $target 'UninstallLuCurrentPreview.exe'
    if(Test-Path -LiteralPath $uninstall){
      try{
        Invoke-NSIS $uninstall @('/S')
        $report.phases['original_uninstaller']='PASS_EXIT_ZERO'
      }catch{$report.errors+=@('Original NSIS uninstall failed');$report.result='INCOMPLETE'}
    }else{$report.errors+=@('Original NSIS uninstaller missing');$report.result='INCOMPLETE'}
  }
  $report|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $EvidenceDir 'cp11-cp12-windows-installed-recovery.json') -Encoding UTF8
}
if($report.result -ne 'PASS_SCOPED_HOSTED_WINDOWS_CP11A_CP11B_CP12A'){exit 2}
exit 0
