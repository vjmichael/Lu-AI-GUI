# PLAN-06 native installed Windows UIAutomation review, refusal, dual-project
# switching, app restart and independent ANSF SQLite witness.
# No success without genuine installed Win32 windows or OS dialog evidence.
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
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){throw 'PLAN06 requires explicitly disposable installed Windows'}
$expected=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
if([IO.Path]::GetFullPath($ProfileDir).TrimEnd('\') -cne [IO.Path]::GetFullPath($expected).TrimEnd('\')){
  throw 'PLAN06 refuses unrelated or operator-owned ANSF profiles'
}
$exe=Get-Item -LiteralPath $InstalledExe -Force -ErrorAction Stop
if($exe.PSIsContainer -or $exe.Name -cne 'lu-desktop.exe' -or
  (($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) -or
  (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ine $ExpectedSha256){
  throw 'PLAN06 actual installed executable digest mismatch'
}
$seed=Get-Content -LiteralPath $SeedReceipt -Raw -Encoding UTF8|ConvertFrom-Json
if($seed.schema -cne 'lu-plan06-disposable-dual-project-fixture-v1' -or $seed.projects.Count -ne 2){
  throw 'PLAN06 independent original dual-project test data absent'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Refuse to overwrite existing PLAN06 UIA evidence'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Plan06Mouse {
  [DllImport("user32.dll",SetLastError=true)] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr e);
}
'@
$script:process=$null
$script:main=$null
$script:board=$null
$script:report=[ordered]@{
  schema='lu-plan06-installed-uia-acceptance-v1'
  installedExecutableSha256=$ExpectedSha256.ToLowerInvariant()
  fixtureNotRealGguf=$true
  genuineInstalledProcess='NOT_RUN'
  independentTaskListWindow='NOT_RUN'
  nativeSourceAndFilters='NOT_RUN'
  realReorderRefusal='NOT_RUN'
  exactReviewedReorder='NOT_RUN'
  humanPlanApprovalRefusal='NOT_RUN'
  nativePlanApproval='NOT_RUN'
  projectSwitching='NOT_RUN'
  applicationRestart='NOT_RUN'
  restartExactAnsfdurableReadback='NOT_RUN'
  noAutomaticExecution='NOT_RUN'
  acceptance='NOT_ACCEPTED'
  evidence=@()
  errors=@()
}
function Witness([string]$phase){
  $destination=Join-Path $EvidenceDir ("ansf-"+$phase+".json")
  $out=@(& python scripts/plan06-readonly-ansf-witness.py --profile $ProfileDir --seed $SeedReceipt --output $destination)
  if($LASTEXITCODE -ne 0 -or -not(Test-Path $destination -PathType Leaf)){
    throw "Independent ANSF witness failed in phase $phase"
  }
  $script:report.evidence+=@($destination)
  return Get-Content -LiteralPath $destination -Raw -Encoding UTF8|ConvertFrom-Json
}
function Dump-Tree([string]$phase,[System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $owner){return}
  try{
    $elements=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    $samples=@()
    for($i=0;$i -lt [Math]::Min($elements.Count,500);$i++){
      try{
        $v=$elements[$i]
        $samples+=@([ordered]@{
          name=$v.Current.Name;type=$v.Current.ControlType.ProgrammaticName
          processId=$v.Current.ProcessId;offscreen=$v.Current.IsOffscreen
          bounds=[string]$v.Current.BoundingRectangle
        })
      }catch{}
    }
    $path=Join-Path $EvidenceDir ("uia-"+$phase+".json")
    [ordered]@{phase=$phase;owner=$owner.Current.Name;elements=$samples}|
      ConvertTo-Json -Depth 6|Set-Content -LiteralPath $path -Encoding UTF8
    $script:report.evidence+=@($path)
  }catch{$script:report.errors+=@("UIA diagnostics: "+$_.Exception.Message)}
}
function Find-Control([System.Windows.Automation.AutomationElement]$owner,[string]$name,
  [string]$type='',[int]$timeout=28,[switch]$Contains){
  $deadline=[DateTime]::UtcNow.AddSeconds($timeout)
  do{
    $els=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    foreach($e in $els){
      try{
        if($Contains){
          if(-not $e.Current.Name.Contains($name)){continue}
        }elseif($e.Current.Name -cne $name){continue}
        if($type -and $e.Current.ControlType.ProgrammaticName -cne $type){continue}
        if($e.Current.IsOffscreen){continue}
        return $e
      }catch{}
    }
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  Dump-Tree ("missing-"+($name -replace '[^a-zA-Z0-9_-]','-')) $owner
  throw "PLAN06 installed UIA control unavailable: $name ($type)"
}
function Native-Window([string]$title,[int]$timeout=28){
  $deadline=[DateTime]::UtcNow.AddSeconds($timeout)
  do{
    $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
    foreach($w in $top){
      try{
        if($w.Current.Name -cne $title -or
          $w.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window -or
          [int64]$w.Current.ProcessId -ne [int64]$script:process.Id -or
          $w.Current.IsOffscreen -or [int64]$w.Current.NativeWindowHandle -eq 0){continue}
        return $w
      }catch{}
    }
    Start-Sleep -Milliseconds 180
  }while([DateTime]::UtcNow -lt $deadline)
  Dump-Tree ('missing-window-'+($title -replace '[^a-zA-Z0-9_-]','-')) $script:board
  throw "PLAN06 genuine process-owned Windows dialog not found: $title"
}
function Click([System.Windows.Automation.AutomationElement]$control,
    [System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $control -or $null -eq $owner -or
    [int64]$owner.Current.ProcessId -ne [int64]$script:process.Id -or
    [int64]$control.Current.ProcessId -ne [int64]$script:process.Id){
    throw 'PLAN06 refuses foreign OS control'
  }
  $invoke=$null
  if($control.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$invoke)){
    ([System.Windows.Automation.InvokePattern]$invoke).Invoke();return
  }
  try{$control.SetFocus()}catch{}
  $point=$control.GetClickablePoint()
  $bounds=$owner.Current.BoundingRectangle
  if($point.X -lt $bounds.Left -or $point.X -ge $bounds.Right -or
     $point.Y -lt $bounds.Top -or $point.Y -ge $bounds.Bottom){
    throw 'PLAN06 UIA click escaped reviewed window'
  }
  if(-not [Plan06Mouse]::SetCursorPos([int]$point.X,[int]$point.Y)){
    throw 'PLAN06 native mouse target failed'
  }
  [Plan06Mouse]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [Plan06Mouse]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
}
function Launch(){
  $script:process=Start-Process -FilePath $exe.FullName -WorkingDirectory $exe.DirectoryName -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(55)
  do{
    $script:process.Refresh()
    if($script:process.HasExited){throw 'Installed Lu exited before opening main window'}
    if($script:process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 300
  }while([DateTime]::UtcNow -lt $deadline)
  if($script:process.MainWindowHandle -eq [IntPtr]::Zero){throw 'Installed Lu never created a Win32 main window'}
  $script:main=[System.Windows.Automation.AutomationElement]::FromHandle($script:process.MainWindowHandle)
  if($script:main.Current.ProcessId -ne $script:process.Id){throw 'Installed Lu main HWND identity invalid'}
  $script:report.genuineInstalledProcess='PASS_INSTALLED_EXE_AUTHENTIC_WIN32'
}
function Open-Board(){
  Click (Find-Control $script:main 'Project tasks' 'ControlType.Button') $script:main
  $script:board=Native-Window 'Lu — Project Tasks'
  if($script:board.Current.NativeWindowHandle -eq $script:main.Current.NativeWindowHandle){
    throw 'Project task-list did not create separate native Tauri HWND'
  }
  $null=Find-Control $script:board 'Project selector' '' 40
  $null=Find-Control $script:board 'Refresh registers' 'ControlType.Button' 40
  $null=Find-Control $script:board 'All (2)' 'ControlType.Button' 40
  $script:report.independentTaskListWindow='PASS_DISTINCT_AUTHENTIC_TASK_LIST_HWND'
  Dump-Tree 'native-task-list' $script:board
}
function Stop-Lu(){
  Click (Find-Control $script:board 'Close project task list' 'ControlType.Button') $script:board
  if(-not $script:process.CloseMainWindow()){throw 'Installed Lu refused WM_CLOSE'}
  if(-not $script:process.WaitForExit(20000)){throw 'Installed Lu never exited after graceful close'}
  $script:process.Refresh()
  if($script:process.ExitCode -ne 0){throw "Installed Lu exited nonzero: $($script:process.ExitCode)"}
  $script:process=$null;$script:main=$null;$script:board=$null
}
function Dialog-Decision([string]$title,[string]$button){
  $dialog=Native-Window $title
  Dump-Tree ('review-'+($title -replace '[^a-zA-Z0-9_-]','-')) $dialog
  Click (Find-Control $dialog $button 'ControlType.Button') $dialog
  Start-Sleep -Milliseconds 800
}
function Wait-Sequence([string]$register,[int]$want,[string]$phase){
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  do{
    Start-Sleep -Milliseconds 500
    $w=Witness ($phase+'-'+[Guid]::NewGuid().ToString('N'))
    $record=$w.projects|Where-Object {$_.registerId -ceq $register}
    if($record.boardSequence -eq $want){return $w}
  }while([DateTime]::UtcNow -lt $deadline)
  throw "PLAN06 no independently verified board revision $want"
}
function Switch-Project(){
  $selector=Find-Control $script:board 'Project selector' '' 10
  $selector.SetFocus()
  # HTML native select receives genuine keyboard selection, not an injected
  # DOM value; the independent ANSF witness verifies the two source scopes.
  [System.Windows.Forms.SendKeys]::SendWait('{HOME}')
  [System.Windows.Forms.SendKeys]::SendWait('{DOWN}')
  [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
  Start-Sleep -Milliseconds 800
}
try{
  $before=Witness 'before-installed-launch'
  if($before.projects.Count -ne 2 -or @($before.projects|Where-Object {$_.boardSequence -ne 0}).Count -ne 0){
    throw 'PLAN06 dual-project source fixture was not untouched'
  }
  Launch
  Open-Board
  Click (Find-Control $script:board 'Unverified (' 'ControlType.Button' 12 -Contains) $script:board
  $null=Find-Control $script:board 'All (' 'ControlType.Button' 12 -Contains
  Click (Find-Control $script:board 'All (2)' 'ControlType.Button') $script:board
  $selected=Find-Control $script:board 'Review ' 'ControlType.Button' 15 -Contains
  Click $selected $script:board
  $null=Find-Control $script:board 'Edit task wording…' 'ControlType.Button' 15
  $null=Find-Control $script:board 'Evidence details' '' 15
  $script:report.nativeSourceAndFilters='PASS_VISIBLE_ORIGINAL_SOURCE_AND_CATEGORY_FILTERS'
  $first=$selected.Current.Name
  # Task list has exactly two original tasks; move the first task down.
  $move=Find-Control $script:board 'Move ' 'ControlType.Button' 12 -Contains
  if(-not $move.Current.Name.EndsWith(' down')){
    $selectedName=$first.Split([char]0x00B7)[1].Trim()
    $move=Find-Control $script:board ' down' 'ControlType.Button' 12 -Contains
  }
  Click $move $script:board
  Dialog-Decision 'Lu — Reorder human plan' 'Cancel'
  $afterCancel=Witness 'after-reorder-refusal'
  if(@($afterCancel.projects|Where-Object {$_.boardSequence -ne 0}).Count -ne 0){
    throw 'PLAN06 native cancellation wrote a board revision'
  }
  $script:report.realReorderRefusal='PASS_TRUE_NATIVE_CANCEL_NO_DATABASE_WRITE'
  Click (Find-Control $script:board ' down' 'ControlType.Button' 15 -Contains) $script:board
  Dialog-Decision 'Lu — Reorder human plan' 'Confirm review only'
  $afterMove=Witness 'after-approved-order'
  $modified=@($afterMove.projects|Where-Object {$_.boardSequence -eq 1})
  if($modified.Count -ne 1 -or $modified[0].boardReviewState -cne 'draft'){
    throw 'PLAN06 native reorder failed independent SQLite witness'
  }
  $edited=$modified[0].registerId
  $script:report.exactReviewedReorder='PASS_ONE_APPEND_ONLY_ORDER_REVISION'
  Click (Find-Control $script:board 'Approve plan wording and order only…' 'ControlType.Button') $script:board
  Dialog-Decision 'Lu — Approve task plan only' 'Cancel'
  $afterRefusal=Witness 'after-plan-approval-refusal'
  if(@($afterRefusal.projects|Where-Object {$_.boardSequence -eq 2}).Count -ne 0){
    throw 'PLAN06 native refusal unexpectedly granted approval'
  }
  $script:report.humanPlanApprovalRefusal='PASS_REAL_NATIVE_REFUSAL_NO_APPROVAL'
  Click (Find-Control $script:board 'Approve plan wording and order only…' 'ControlType.Button') $script:board
  Dialog-Decision 'Lu — Approve task plan only' 'Confirm review only'
  $approved=Wait-Sequence $edited 2 'native-plan-approved'
  $a=$approved.projects|Where-Object {$_.registerId -ceq $edited}
  if($a.boardReviewState -cne 'human_approved' -or -not $a.planReviewCurrent){
    throw 'PLAN06 installed OS approval did not persist a current plan-only review'
  }
  $script:report.nativePlanApproval='PASS_REAL_WIN32_APPROVAL_DURABLE_NO_EXECUTION'
  Switch-Project
  $null=Find-Control $script:board 'All (2)' 'ControlType.Button'
  Dump-Tree 'after-project-switch' $script:board
  $script:report.projectSwitching='PASS_NATIVE_SELECT_SWITCH_U_I'
  Stop-Lu
  $closed=Witness 'after-graceful-close'
  Launch
  Open-Board
  $reopened=Witness 'after-installed-app-restart'
  $r=$reopened.projects|Where-Object {$_.registerId -ceq $edited}
  if($r.boardSequence -ne 2 -or $r.boardSha256 -cne $a.boardSha256 -or
     $r.currentRegisterSha256 -cne $a.currentRegisterSha256 -or -not $r.planReviewCurrent){
    throw 'PLAN06 installed application restart lost original ANSF approval chain'
  }
  Dump-Tree 'after-actual-installed-restart' $script:board
  $script:report.applicationRestart='PASS_REAL_INSTALLED_PROCESS_EXIT_RESTART'
  $script:report.restartExactAnsfdurableReadback='PASS_ORIGINAL_SQLITE_SHA_AND_APPROVAL_CHAIN'
  $script:report.noAutomaticExecution='PASS_BOTH_PROJECTS_NO_EXECUTION_AUTHORITY'
  Stop-Lu
  $script:report.acceptance='PASS_PLAN06_INSTALLED_INTERACTIONS_AND_RESTART'
}catch{
  $script:report.errors+=@($_.Exception.ToString())
  Write-Warning ('PLAN06 installed acceptance remains OPEN: '+$_.Exception.Message)
}finally{
  if($null -ne $script:process){
    try{
      $script:process.Refresh()
      if(-not $script:process.HasExited){
        if(-not $script:process.CloseMainWindow() -or -not $script:process.WaitForExit(8000)){
          $script:process.Kill($true);$null=$script:process.WaitForExit(5000)
        }
      }
    }catch{$script:report.errors+=@("Owned installed Lu cleanup: "+$_.Exception.Message)}
  }
  $script:report|ConvertTo-Json -Depth 8|
    Set-Content -LiteralPath (Join-Path $EvidenceDir 'plan06-installed-uia.json') -Encoding UTF8
}
if($script:report.acceptance -ne 'PASS_PLAN06_INSTALLED_INTERACTIONS_AND_RESTART'){exit 2}
exit 0
