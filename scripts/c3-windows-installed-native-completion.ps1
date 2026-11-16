# C3 original installed Windows Task Board, actual OS completion dialogs.
# This tests a disposable automated UIA operator and a synthetic original
# source fixture. It NEVER claims a real person's positive completion approval
# or that a GGUF executed inside this installed test.
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$InstalledExe,
 [Parameter(Mandatory=$true)][string]$ExpectedSha256,
 [Parameter(Mandatory=$true)][string]$ProfileDir,
 [Parameter(Mandatory=$true)][string]$SeedReceipt,
 [Parameter(Mandatory=$true)][string]$EvidenceDir,
 [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -cne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
 throw 'C3 requires a disposable real interactive Windows session'
}
$expected=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
if([IO.Path]::GetFullPath($ProfileDir).TrimEnd('\') -cne [IO.Path]::GetFullPath($expected).TrimEnd('\')){
 throw 'Refuse to read or modify a non-disposable owner ANSF profile'
}
if(-not(Test-Path -LiteralPath $SeedReceipt -PathType Leaf) -or (Test-Path $EvidenceDir)){
 throw 'Missing seed or preexisting receipt destination'
}
New-Item -ItemType Directory -Path $EvidenceDir|Out-Null
$exe=Get-Item -LiteralPath $InstalledExe -ErrorAction Stop
if($exe.Name -cne 'lu-desktop.exe' -or $exe.PSIsContainer -or
 (($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) -or
 (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ine $ExpectedSha256){
 throw 'Not a source-pinned genuine installed Windows Lu executable'
}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
$script:app=$null;$script:main=$null;$script:board=$null;$script:dialog=$null
$script:report=[ordered]@{
 schema='lu-c3-installed-windows-native-decision-negative-v1'
 installedSourceExeSha256=$ExpectedSha256.ToLowerInvariant()
 testFixtureSyntheticNotRealGguf=$true
 scriptedCiOperatorNotLiveHuman=$true
 actualNativeTaskBoard='NOT_TESTED'
 incompletePositiveRefused='NOT_TESTED'
 cancelledNativeDialogNoDecision='NOT_TESTED'
 originalBlockedDecision='NOT_TESTED'
 originalReplanDecision='NOT_TESTED'
 afterRestartNewReviewRequired='NOT_TESTED'
 actualHumanCompletion='NOT_TESTED'
 acceptance='NOT_ACCEPTED'
 errors=@()
}
function All([System.Windows.Automation.AutomationElement]$owner){
 return $owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,
   [System.Windows.Automation.Condition]::TrueCondition)
}
function CaptureOriginalUIATree([string]$label,[System.Windows.Automation.AutomationElement]$root){
 if($null -eq $root){return}
 try{
  $sample=@()
  $items=All $root
  for($i=0;$i -lt [Math]::Min(450,$items.Count);$i++){
   try{
    $item=$items[$i]
    $sample+=@([ordered]@{
      name=$item.Current.Name
      type=$item.Current.ControlType.ProgrammaticName
      processId=$item.Current.ProcessId
      offscreen=$item.Current.IsOffscreen
      enabled=$item.Current.IsEnabled
      hwnd=$item.Current.NativeWindowHandle
    })
   }catch{}
  }
  $file=Join-Path $EvidenceDir ('uia-'+$label+'.json')
  [ordered]@{
    rootProcessId=$root.Current.ProcessId
    rootHwnd=$root.Current.NativeWindowHandle
    appProcessId=$script:app.Id
    originalElements=$sample
    truncated=($items.Count -gt 450)
  }|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $file -Encoding UTF8
 }catch{
  $script:report.errors+=@('UIA diagnostic '+$label+': '+$_.Exception.Message)
 }
}
# Windows WebView2 DOM UIA elements often have a child process PID, but are
# still descendants of the *verified* Tauri app-owned native window. Reject
# all controls outside the already authenticated native window subtrees.
function InOwnedNativeTree([System.Windows.Automation.AutomationElement]$target){
 foreach($root in @($script:main,$script:board,$script:dialog)){
  if($null -eq $root){continue}
  try{
   if($root.Current.ProcessId -ne $script:app.Id -or
      $root.Current.NativeWindowHandle -eq 0){continue}
   if([System.Windows.Automation.AutomationElement]::Compare($root,$target)){return $true}
   foreach($candidate in (All $root)){
    if([System.Windows.Automation.AutomationElement]::Compare($candidate,$target)){
     return $true
    }
   }
  }catch{}
 }
 return $false
}
function Visible([System.Windows.Automation.AutomationElement]$x){
 if(-not $x.Current.IsOffscreen){return $true}
 $pat=$null
 if($x.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern,[ref]$pat)){
  ([System.Windows.Automation.ScrollItemPattern]$pat).ScrollIntoView()
  Start-Sleep -Milliseconds 250
 }
 return (-not $x.Current.IsOffscreen)
}
function Find([System.Windows.Automation.AutomationElement]$owner,[string]$name,[int]$seconds=30){
 if(-not (InOwnedNativeTree $owner)){throw 'UIA search owner is not in Lu native window ancestry'}
 $limit=[DateTime]::UtcNow.AddSeconds($seconds)
 do{
  foreach($x in (All $owner)){
   try{
    if($x.Current.Name -cne $name){continue}
    if(Visible $x){return $x}
   }catch{}
  }
  Start-Sleep -Milliseconds 250
 }while([DateTime]::UtcNow -lt $limit)
 CaptureOriginalUIATree ('missing-'+($name -replace '[^a-zA-Z0-9]','-')) $owner
 throw ('Real UIA control missing/offscreen: '+$name)
}
function Click([System.Windows.Automation.AutomationElement]$x){
 if(-not (InOwnedNativeTree $x) -or -not (Visible $x) -or
    -not $x.Current.IsEnabled){throw 'Refuse disabled/foreign/offscreen UIA action'}
 $p=$null
 if($x.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$p)){
  ([System.Windows.Automation.InvokePattern]$p).Invoke()
 }else{
  if(-not $x.Current.IsKeyboardFocusable){throw 'No native click or keyboard focus'}
  $x.SetFocus();[System.Windows.Forms.SendKeys]::SendWait(' ')
 }
}
function Native([string]$title){
 $limit=[DateTime]::UtcNow.AddSeconds(35)
 do{
  $roots=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    [System.Windows.Automation.Condition]::TrueCondition)
  foreach($w in $roots){
   try{
    if($w.Current.Name -cne $title -or $w.Current.ProcessId -ne $script:app.Id -or
       $w.Current.NativeWindowHandle -eq 0 -or $w.Current.IsOffscreen -or
       $w.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window){continue}
    $script:dialog=$w
    return $w
   }catch{}
  }
  Start-Sleep -Milliseconds 250
 }while([DateTime]::UtcNow -lt $limit)
 CaptureOriginalUIATree 'missing-installed-native-dialog' $script:board
 throw ('No independent OS-owned installed native dialog: '+$title)
}
function Journal(){
 $db=Join-Path $ProfileDir 'ansf\library.db'
 $py=@'
import sys,sqlite3,hashlib,json
db=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)
if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='ansf_project_task_completion_decisions'").fetchone():
 rows=db.execute("SELECT sequence,event_sha256,event_json,no_execution_authority FROM ansf_project_task_completion_decisions ORDER BY sequence").fetchall()
 out=[]
 for seq,sha,raw,authority in rows:
  if authority!=1 or hashlib.sha256(raw).hexdigest()!=sha: raise ValueError("unverified source ledger")
  v=json.loads(raw)
  out.append({"sequence":seq,"decision":v["decision"],"version":v["taskVersion"],
   "completed":v["taskCompletedAtObservation"],
   "humanAuthorized":v["humanAuthorized"],
   "executionAuthorized":v["executionAuthorized"],"eventId":v["eventId"]})
 print(json.dumps(out))
else: print("[]")
'@
 $json=& python -c $py $db
 if($LASTEXITCODE -ne 0){throw 'Independent original SQLite readback failed'}
 return @((($json -join [Environment]::NewLine)|ConvertFrom-Json))
}
function StartInstalled(){
 $script:app=Start-Process -FilePath $exe.FullName -WorkingDirectory $exe.DirectoryName -PassThru
 $limit=[DateTime]::UtcNow.AddSeconds(60)
 do{
  $script:app.Refresh()
  if($script:app.HasExited){throw 'Installed Lu process exited before UIA'}
  if($script:app.MainWindowHandle -ne [IntPtr]::Zero){break}
  Start-Sleep -Milliseconds 250
 }while([DateTime]::UtcNow -lt $limit)
 if($script:app.MainWindowHandle -eq [IntPtr]::Zero){throw 'No real app main HWND'}
 $main=[System.Windows.Automation.AutomationElement]::FromHandle($script:app.MainWindowHandle)
 if($main.Current.ProcessId -ne $script:app.Id){throw 'Wrong installed main process'}
 $script:main=$main
 CaptureOriginalUIATree 'installed-main-before-task-board' $main
 Click (Find $main 'Project tasks')
 $script:board=Native 'Lu — Project Tasks'
 CaptureOriginalUIATree 'installed-native-task-board' $script:board
 if($main.Current.NativeWindowHandle -eq $script:board.Current.NativeWindowHandle){
  throw 'Task Board did not open in a distinct native HWND'
 }
}
function SelectTask(){
 $region=Find $script:board 'Categorized project tasks'
 $limit=[DateTime]::UtcNow.AddSeconds(25)
 do{
  foreach($x in (All $region)){
   try{
    if($x.Current.ControlType.ProgrammaticName -cne 'ControlType.Button' -or
       $x.Current.Name -notmatch 'Review (north|south) source requirement 1'){continue}
    Click $x
    $null=Find $script:board 'Whole task completion disposition'
    $null=Find $script:board 'Whole task human decision reason'
    return
   }catch{}
  }
  Start-Sleep -Milliseconds 300
 }while([DateTime]::UtcNow -lt $limit)
 throw 'Installed native Task Board has no accessible original source task'
}
function Choose([int]$item){
 $x=Find $script:board 'Whole task completion disposition'
 if(-not $x.Current.IsKeyboardFocusable){throw 'Original decision selector is not keyboard accessible'}
 $x.SetFocus()
 [System.Windows.Forms.SendKeys]::SendWait('{HOME}')
 for($i=0;$i -lt $item;$i++){[System.Windows.Forms.SendKeys]::SendWait('{DOWN}')}
 [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
 Start-Sleep -Milliseconds 220
}
function Reason([string]$value){
 $x=Find $script:board 'Whole task human decision reason'
 $p=$null
 if($x.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$p)){
  ([System.Windows.Automation.ValuePattern]$p).SetValue($value)
 }else{
  if(-not $x.Current.IsKeyboardFocusable){throw 'Original reason textarea inaccessible'}
  $x.SetFocus();[System.Windows.Forms.SendKeys]::SendWait('^a')
  [System.Windows.Forms.SendKeys]::SendWait($value)
 }
 Start-Sleep -Milliseconds 250
}
function CompleteButton(){return Find $script:board 'Open native human whole-task decision…'}
function StopInstalled(){
 Click (Find $script:board 'Close project task list')
 if(-not $script:app.CloseMainWindow() -or -not $script:app.WaitForExit(30000)){
  throw 'Installed app failed graceful task-window restart exit'
 }
 if($script:app.ExitCode -ne 0){throw 'Installed app exit nonzero'}
 $script:app=$null;$script:main=$null;$script:board=$null;$script:dialog=$null
}
try{
 StartInstalled
 SelectTask
 if(@(Journal).Count -ne 0){throw 'Nonempty original completion ledger'}
 $script:report.actualNativeTaskBoard='PASS_REAL_INSTALLED_NATIVE_TASK_WINDOW'
 Choose 3
 Reason 'The selected original task lacks any independently approved native worker evidence.'
 if((CompleteButton).Current.IsEnabled){throw 'Incomplete native completion button erroneously active'}
 $script:report.incompletePositiveRefused='PASS_NATIVE_WEBVIEW_POSITIVE_INCOMPLETE_REFUSAL'
 Choose 0
 Reason 'The original requirement has no human-reviewed independent completion evidence.'
 Click (CompleteButton)
 $dialog=Native 'Lu — Explicit whole-task human decision'
 Click (Find $dialog 'Cancel')
 if(@(Journal).Count -ne 0){throw 'Cancel produced an original completion decision'}
 $script:report.cancelledNativeDialogNoDecision='PASS_REAL_OS_CANCEL_NO_JOURNAL'
 Click (CompleteButton)
 $dialog=Native 'Lu — Explicit whole-task human decision'
 Click (Find $dialog 'Record blocked task')
 Start-Sleep -Milliseconds 500
 $one=@(Journal)
 if($one.Count -ne 1 -or $one[0].decision -cne 'BLOCKED' -or
    $one[0].completed -or $one[0].executionAuthorized){
  throw 'Native blocked decision missing or invented task completion'
 }
 $script:report.originalBlockedDecision='PASS_REAL_OS_BLOCKED_NO_EXECUTION'
 Choose 2
 Reason 'A newly human-reviewed original task revision is required after interrupted evidence.'
 Click (CompleteButton)
 $dialog=Native 'Lu — Explicit whole-task human decision'
 Click (Find $dialog 'Require reviewed task replan')
 Start-Sleep -Milliseconds 500
 $two=@(Journal)
 if($two.Count -ne 2 -or $two[1].decision -cne 'REPLAN_REQUIRED' -or
    $two[1].completed -or $two[1].executionAuthorized){
  throw 'Native reviewed replan did not persist exact no-authority disposition'
 }
 $script:report.originalReplanDecision='PASS_REAL_OS_REPLAN_JOURNAL'
 StopInstalled
 StartInstalled
 SelectTask
 Choose 3
 Reason 'A prior negative source decision is not sufficient to complete this task.'
 if((CompleteButton).Current.IsEnabled){throw 'Restart reused stale positive completion admission'}
 $reopened=@(Journal)
 if($reopened.Count -ne 2 -or $reopened[1].eventId -cne $two[1].eventId){
  throw 'Original immutable task decision history lost after real process restart'
 }
 $script:report.afterRestartNewReviewRequired='PASS_FRESH_SOURCE_REVIEW_REQUIRED_AFTER_RESTART'
 StopInstalled
 $script:report.acceptance='PASS_C3_REAL_INSTALLED_NATIVE_NEGATIVE_COMPLETION'
}catch{
 $script:report.errors+=@($_.Exception.ToString())
 Write-Warning ('C3 installed native acceptance FAILED: '+$_.Exception.Message)
}finally{
 if($null -ne $script:app){
  try{$script:app.Refresh();if(-not $script:app.HasExited){
   $script:app.Kill($true);$null=$script:app.WaitForExit(5000)}}catch{}
 }
 $script:report|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (
  Join-Path $EvidenceDir 'c3-windows-native-completion.json') -Encoding UTF8
}
if($script:report.acceptance -cne 'PASS_C3_REAL_INSTALLED_NATIVE_NEGATIVE_COMPLETION'){exit 2}
exit 0
