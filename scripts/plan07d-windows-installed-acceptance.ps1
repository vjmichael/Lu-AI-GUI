# PLAN-07D: real installed Windows UI Automation for PLAN-07C review decisions.
# Requires a prior genuine NSIS installation and PLAN-06 disposable two-scope
# ANSF fixture. No simulated window/dialog is accepted. A failure stays a failure.
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$InstalledExe,
 [Parameter(Mandatory=$true)][string]$ExpectedSha256,
 [Parameter(Mandatory=$true)][string]$ProfileDir,
 [Parameter(Mandatory=$true)][string]$SeedReceipt,
 [Parameter(Mandatory=$true)][string]$ProjectsDir,
 [Parameter(Mandatory=$true)][string]$EvidenceDir,
 [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -cne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
 throw 'PLAN07D requires a confirmed disposable interactive Windows session'
}
$expected=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
if([IO.Path]::GetFullPath($ProfileDir).TrimEnd('\') -cne [IO.Path]::GetFullPath($expected).TrimEnd('\')){
 throw 'PLAN07D refuses owner or non-disposable ANSF profile'
}
$exe=Get-Item -LiteralPath $InstalledExe -ErrorAction Stop
if($exe.Name -cne 'lu-desktop.exe' -or $exe.PSIsContainer -or
 (($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) -or
 (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ine $ExpectedSha256){
 throw 'PLAN07D installed executable provenance denied'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'PLAN07D refuses overwriting UIA evidence'}
New-Item -Path $EvidenceDir -ItemType Directory -ErrorAction Stop|Out-Null
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
$script:p=$null;$script:main=$null;$script:board=$null
$script:report=[ordered]@{
 schema='lu-plan07d-installed-windows-uia-v1'
 sourceExecutableSha256=$ExpectedSha256.ToLowerInvariant()
 originalFixtureIsSyntheticNotGguf=$true
 genuineInstalledWindow='NOT_TESTED'
 keyboardAndAccessibleNames='NOT_TESTED'
 nativeSourcePicker='NOT_TESTED'
 realRefusal='NOT_TESTED'
 nativeApprovalAndCancelledPicker='NOT_TESTED'
 realHumanReviewApproval='NOT_TESTED'
 originalProjectSwitching='NOT_TESTED'
 processCrashRecovery='NOT_TESTED'
 actualApplicationRestart='NOT_TESTED'
 originalAnsfdurableReadback='NOT_TESTED'
 executionAuthorization='DENIED_BY_CONTRACT'
 acceptance='NOT_ACCEPTED'
 errors=@();evidence=@()
}
function SaveTree([string]$name,[System.Windows.Automation.AutomationElement]$owner){
 if($null -eq $owner){return}
 try{
  $elements=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
  $sample=@()
  for($i=0;$i -lt [Math]::Min($elements.Count,350);$i++){
   try{$e=$elements[$i];$sample+=@([ordered]@{name=$e.Current.Name;type=$e.Current.ControlType.ProgrammaticName;process=$e.Current.ProcessId;focus=$e.Current.IsKeyboardFocusable;visible=(-not $e.Current.IsOffscreen)})}catch{}
  }
  $file=Join-Path $EvidenceDir ('uia-'+$name+'.json')
  @{name=$name;elements=$sample}|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $file -Encoding UTF8
  $script:report.evidence+=@($file)
 }catch{$script:report.errors+=@('UIA dump '+$name+': '+$_.Exception.Message)}
}
function RootWindows(){
 return [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
}
function Window([string]$title,[int]$seconds=35){
 $end=[DateTime]::UtcNow.AddSeconds($seconds)
 do{
  foreach($w in (RootWindows)){
   try{
    if($w.Current.Name -cne $title -or $w.Current.ProcessId -ne $script:p.Id -or
       $w.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window -or
       $w.Current.NativeWindowHandle -eq 0 -or $w.Current.IsOffscreen){continue}
    return $w
   }catch{}
  }
  Start-Sleep -Milliseconds 250
 }while([DateTime]::UtcNow -lt $end)
 SaveTree 'missing-native-dialog' $script:board
 throw ('Missing real installed process-owned dialog: '+$title)
}
function Control([System.Windows.Automation.AutomationElement]$owner,[string]$name,[string]$type='', [int]$seconds=28){
 $end=[DateTime]::UtcNow.AddSeconds($seconds)
 do{
  $elements=$owner.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
  foreach($e in $elements){
   try{
    if($e.Current.Name -cne $name -or $e.Current.IsOffscreen){continue}
    if($type -and $e.Current.ControlType.ProgrammaticName -cne $type){continue}
    return $e
   }catch{}
  }
  Start-Sleep -Milliseconds 300
 }while([DateTime]::UtcNow -lt $end)
 SaveTree ('missing-'+($name -replace '[^a-zA-Z0-9]','-')) $owner
 throw ('Missing named accessible control: '+$name)
}
function Click([System.Windows.Automation.AutomationElement]$item){
 if($item.Current.ProcessId -ne $script:p.Id -or $item.Current.IsOffscreen){throw 'Refusing foreign/offscreen UIA control'}
 $pattern=$null
 if($item.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){
  ([System.Windows.Automation.InvokePattern]$pattern).Invoke();return
 }
 if(-not $item.Current.IsKeyboardFocusable){throw 'UI control has no accessible keyboard/invoke action'}
 $item.SetFocus()
 [System.Windows.Forms.SendKeys]::SendWait(' ')
}
function TextValue([System.Windows.Automation.AutomationElement]$item,[string]$value){
 if($item.Current.ProcessId -ne $script:p.Id){throw 'Foreign UI text field'}
 $pattern=$null
 if($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)){
  ([System.Windows.Automation.ValuePattern]$pattern).SetValue($value);return
 }
 if(-not $item.Current.IsKeyboardFocusable){throw 'Human review note inaccessible'}
 $item.SetFocus()
 [System.Windows.Forms.SendKeys]::SendWait('^a')
 [System.Windows.Forms.SendKeys]::SendWait($value)
}
function Witness([string]$name,[string[]]$phases=@()){
 $file=Join-Path $EvidenceDir ('ansf-'+$name+'.json')
 $args=@('scripts/plan07d-readonly-ansf-witness.py','--profile',$ProfileDir,
   '--seed',$SeedReceipt,'--output',$file)
 foreach($phase in $phases){$args+=@('--require-phase',$phase)}
 & python @args
 if($LASTEXITCODE -ne 0 -or -not(Test-Path -LiteralPath $file)){throw 'Independent original ANSF witness rejected '+$name}
 $script:report.evidence+=@($file)
 return Get-Content -LiteralPath $file -Raw -Encoding UTF8|ConvertFrom-Json
}
function StartLu(){
 $script:p=Start-Process -FilePath $exe.FullName -WorkingDirectory $exe.DirectoryName -PassThru
 $end=[DateTime]::UtcNow.AddSeconds(60)
 do{
  $script:p.Refresh()
  if($script:p.HasExited){throw 'Real installed Lu exited early'}
  if($script:p.MainWindowHandle -ne [IntPtr]::Zero){break}
  Start-Sleep -Milliseconds 400
 }while([DateTime]::UtcNow -lt $end)
 if($script:p.MainWindowHandle -eq [IntPtr]::Zero){throw 'No real application HWND'}
 $script:main=[System.Windows.Automation.AutomationElement]::FromHandle($script:p.MainWindowHandle)
 if($script:main.Current.ProcessId -ne $script:p.Id){throw 'Unverified installed main process'}
 Click (Control $script:main 'Project tasks' 'ControlType.Button')
 $script:board=Window 'Lu — Project Tasks'
 if($script:board.Current.NativeWindowHandle -eq $script:main.Current.NativeWindowHandle){throw 'No independent native task window'}
 $null=Control $script:board 'Project selector'
 $null=Control $script:board 'Human evidence decisions · PLAN-07C'
 $script:report.genuineInstalledWindow='PASS_DISTINCT_NATIVE_INSTALLED_HWND'
 SaveTree 'plan07d-task-list' $script:board
}
function GracefulStop(){
 Click (Control $script:board 'Close project task list' 'ControlType.Button')
 if(-not $script:p.CloseMainWindow() -or -not $script:p.WaitForExit(25000)){throw 'Installed Lu did not gracefully close'}
 $script:p.Refresh()
 if($script:p.ExitCode -ne 0){throw 'Installed Lu nonzero exit'}
 $script:p=$null;$script:main=$null;$script:board=$null
}
function Picker([string]$path,[switch]$Cancel){
 $end=[DateTime]::UtcNow.AddSeconds(35)
 $picker=$null
 do{
  foreach($w in (RootWindows)){
   try{
    if($w.Current.ProcessId -ne $script:p.Id -or $w.Current.NativeWindowHandle -eq 0 -or
       $w.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window -or
       $w.Current.IsOffscreen){continue}
    if($w.Current.NativeWindowHandle -eq $script:main.Current.NativeWindowHandle -or
       $w.Current.NativeWindowHandle -eq $script:board.Current.NativeWindowHandle){continue}
    $picker=$w;break
   }catch{}
  }
  if($null -ne $picker){break}
  Start-Sleep -Milliseconds 250
 }while([DateTime]::UtcNow -lt $end)
 if($null -eq $picker){throw 'Real native folder picker window not found'}
 SaveTree 'native-folder-picker' $picker
 if($Cancel){
  $picker.SetFocus()
  [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
  return
 }
 if(-not(Test-Path -LiteralPath $path -PathType Container)){throw 'Selected original source directory missing'}
 $picker.SetFocus()
 [System.Windows.Forms.SendKeys]::SendWait('^l')
 Start-Sleep -Milliseconds 220
 [System.Windows.Forms.SendKeys]::SendWait($path)
 [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
 Start-Sleep -Milliseconds 750
 try{ Click (Control $picker 'Select Folder' 'ControlType.Button' 5) }
 catch{
  try{Click (Control $picker 'Select folder' 'ControlType.Button' 5)}
  catch{throw 'Original native folder picker did not expose selection confirmation'}
 }
}
function SourceLabel(){
 $els=$script:board.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
 foreach($x in $els){
  try{
   if($x.Current.Name -match 'Review (north|south) source requirement' -and
       $x.Current.ControlType.ProgrammaticName -eq 'ControlType.Button'){return $Matches[1]}
  }catch{}
 }
 throw 'Cannot independently determine displayed original project'
}
function Review([string]$note,[string]$choice,[string]$folder){
 TextValue (Control $script:board 'Reason for human evidence decision') $note
 Click (Control $script:board 'Open native OS approve/refuse decision…' 'ControlType.Button')
 $dlg=Window 'Lu — Human evidence follow-up decision'
 SaveTree ('decision-'+$choice) $dlg
 if($choice -eq 'refuse'){
  Click (Control $dlg 'Refuse evidence review' 'ControlType.Button')
 }elseif($choice -eq 'cancel'){
  Click (Control $dlg 'Approve evidence review only' 'ControlType.Button')
  Picker $folder -Cancel
 }elseif($choice -eq 'approve'){
  Click (Control $dlg 'Approve evidence review only' 'ControlType.Button')
  Picker $folder
 }elseif($choice -eq 'crash'){
  # A prepared record must already be durable before OS prompt was opened.
  $script:p.Kill($true)
  if(-not $script:p.WaitForExit(15000)){throw 'Native process interruption failed'}
  $script:p=$null;$script:board=$null;$script:main=$null
 }else{throw 'Unsupported real review option'}
 Start-Sleep -Milliseconds 1100
}
function DiffPhase([string]$name,[string]$phase,[int]$previous){
 $w=Witness $name @($phase)
 $matching=@($w.decisions|Where-Object {$_.phase -ceq $phase})
 if($matching.Count -le $previous){throw ('No NEW durable PLAN07C '+$phase+' decision')}
 return $w
}
try{
 $baseline=Witness 'original-before-ui'
 if($baseline.decisions.Count -ne 0){throw 'PLAN07D requires clean, previously unreviewed PLAN07C journal'}
 StartLu
 foreach($name in @('Project selector','Human evidence decisions · PLAN-07C',
     'Reopen original decision and restart history…','Evidence-driven task update review')){
  $control=Control $script:board $name
  if($control.Current.IsOffscreen){throw ('Accessibility control offscreen: '+$name)}
 }
 $script:report.keyboardAndAccessibleNames='PASS_REAL_UIA_NAMES_AND_CONTROLS'
 $chosen=SourceLabel
 $folder=Join-Path $ProjectsDir $chosen
 Click (Control $script:board 'Recheck project source using native folder picker…' 'ControlType.Button')
 Picker $folder
 $null=Control $script:board 'Original task for human evidence review' '' 40
 $null=Control $script:board 'Reason for human evidence decision'
 $script:report.nativeSourcePicker='PASS_ACTUAL_OS_SOURCE_SELECTION_AND_RESCAN'
 Review 'Refuse pending completion claims until current independent evidence is checked.' 'refuse' $folder
 $w=DiffPhase 'after-real-refusal' 'refused' 0
 $script:report.realRefusal='PASS_REAL_OS_REFUSAL_DURABLE_NO_AUTHORITY'
 Review 'Approval will be cancelled at the original source picker deliberately.' 'cancel' $folder
 $w=DiffPhase 'after-cancelled-picker' 'cancelled' 0
 $script:report.nativeApprovalAndCancelledPicker='PASS_REAL_NATIVE_PICKER_CANCEL_DURABLE'
 Review 'Approve a separate evidence follow-up without task acceptance or worker authority.' 'approve' $folder
 $w=DiffPhase 'after-real-followup-approval' 'approved_for_review' 0
 $script:report.realHumanReviewApproval='PASS_OS_DIALOG_ORIGINAL_SOURCE_RECHECK'
 $oldIds=@($w.decisions|ForEach-Object {$_.latestSha256})
 # Project switch must be visibly observed after selecting a different source.
 $selector=Control $script:board 'Project selector'
 $selector.SetFocus()
 [System.Windows.Forms.SendKeys]::SendWait('{HOME}{ENTER}')
 Start-Sleep -Milliseconds 600
 $other=SourceLabel
 if($other -ceq $chosen){
  $selector=Control $script:board 'Project selector'
  $selector.SetFocus()
  [System.Windows.Forms.SendKeys]::SendWait('{DOWN}{ENTER}')
  Start-Sleep -Milliseconds 650
  $other=SourceLabel
 }
 if($other -ceq $chosen){throw 'No real original-project switching observed'}
 $script:report.originalProjectSwitching='PASS_TWO_DISTINCT_SOURCE_PROJECTS_VISIBLE'
 GracefulStop
 StartLu
 Click (Control $script:board 'Reopen original decision and restart history…' 'ControlType.Button')
 $after=Witness 'after-graceful-restart' @('refused','cancelled','approved_for_review')
 $now=@($after.decisions|ForEach-Object {$_.latestSha256})
 if((@($now) -join ',') -cne (@($oldIds) -join ',')){
  throw 'Restart changed immutable original review chain'
 }
 $script:report.actualApplicationRestart='PASS_REAL_INSTALLED_EXIT_AND_REOPEN'
 $script:report.originalAnsfdurableReadback='PASS_INDEPENDENT_SQLITE_ORIGINAL_CHAIN'
 # Crash after Prepared, before the real OS decision: no phantom approval.
 $latest=SourceLabel;$folder=Join-Path $ProjectsDir $latest
 Click (Control $script:board 'Recheck project source using native folder picker…' 'ControlType.Button')
 Picker $folder
 $null=Control $script:board 'Original task for human evidence review' '' 40
 Review 'Deliberately interrupt the real installed process before operator confirmation.' 'crash' $folder
 $prepared=Witness 'after-forced-process-interruption'
 if(@($prepared.decisions|Where-Object {$_.phase -ceq 'prepared'}).Count -ne 1){
  throw 'No pending durable prepared record at real interruption'
 }
 StartLu
 Click (Control $script:board 'Reopen original decision and restart history…' 'ControlType.Button')
 $stop=[DateTime]::UtcNow.AddSeconds(15)
 do{
  $recovered=Witness ('recovery-'+[Guid]::NewGuid().ToString('N')) @('refused','cancelled','approved_for_review')
  if(@($recovered.decisions|Where-Object {$_.phase -ceq 'interrupted'}).Count -eq 1){break}
  Start-Sleep -Milliseconds 500
 }while([DateTime]::UtcNow -lt $stop)
 if(@($recovered.decisions|Where-Object {$_.phase -ceq 'interrupted'}).Count -ne 1){
  throw 'Actual process restart failed to seal Prepared as Interrupted'
 }
 $script:report.processCrashRecovery='PASS_NO_REPLAY_NO_PHANTOM_APPROVAL'
 GracefulStop
 $final=Witness 'final-ansf-after-exit' @('refused','cancelled','approved_for_review','interrupted')
 $script:report.acceptance='PASS_PLAN07D_HOSTED_WINDOWS_INSTALLED_INTERACTIONS'
}catch{
 $script:report.errors+=@($_.Exception.ToString())
 Write-Warning ('PLAN07D hosted Windows acceptance OPEN: '+$_.Exception.Message)
}finally{
 if($null -ne $script:p){
  try{$script:p.Refresh();if(-not $script:p.HasExited){$script:p.Kill($true);$null=$script:p.WaitForExit(5000)}}catch{}
 }
 $script:report|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $EvidenceDir 'plan07d-windows-uia.json') -Encoding UTF8
}
if($script:report.acceptance -cne 'PASS_PLAN07D_HOSTED_WINDOWS_INSTALLED_INTERACTIONS'){exit 2}
exit 0
