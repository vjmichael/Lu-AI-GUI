# CP-05E5 installed GGUF acceptance: disposable Windows only, no test RPC.
# Uses genuine UIA controls and independently read-only native SQLite receipts.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$EngineSha256,
  [Parameter(Mandatory=$true)][string]$ModelPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ModelSha256,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [Parameter(Mandatory=$true)][ValidatePattern('^org\.lu\.agent01g\.devtest$')][string]$ProfileIdentifier,
  [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){throw 'Disposable native Windows required'}
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence path must be new'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$profile=Join-Path $env:APPDATA $ProfileIdentifier
$models=Join-Path $profile 'lu-models'
$enginePath=Join-Path (Split-Path -Parent $InstalledExe) 'lu-inference\llama-server.exe'
$oracle=Join-Path $PSScriptRoot 'cp05e5-installed-journal.py'
$report=[ordered]@{
  schema='lu-cp05e5-installed-gguf-v1'; result='NOT_ACCEPTED'
  appSha256=$ExpectedSha256; engineSha256=$EngineSha256; modelSha256=$ModelSha256
  activationMethods=@(); errors=@(); phases=[ordered]@{}; currentPhase='preflight'
  genuineGgufInference='NOT_TESTED'; activeGpuOffload='NOT_TESTED'
  authorization='TEST_OPERATOR_FIXTURE_NOT_PHYSICAL_OWNER'
}
$process=$null;$window=$null;$engine=$null;$consentDialog=$null

function Save-UIA-Failure([System.Windows.Automation.AutomationElement]$owner,[string]$phase){
  if($null -eq $owner){return}
  $safe=($phase -replace '[^a-zA-Z0-9._-]','-')
  try {
    $items=$owner.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    $sample=New-Object 'System.Collections.Generic.List[object]'
    $max=[Math]::Min(500,$items.Count)
    for($i=0;$i -lt $max;$i++){
      try{
        $el=$items[$i]
        $sample.Add([ordered]@{
          name=$el.Current.Name; controlType=[string]$el.Current.ControlType.ProgrammaticName
          automationId=$el.Current.AutomationId; className=$el.Current.ClassName
          processId=[int64]$el.Current.ProcessId
          enabled=[bool]$el.Current.IsEnabled; offscreen=[bool]$el.Current.IsOffscreen
          handle=[int64]$el.Current.NativeWindowHandle
          bounds=[string]$el.Current.BoundingRectangle
        })
      }catch{
        $sample.Add([ordered]@{error='Inaccessible UIA child'})
      }
    }
    $evidence=[ordered]@{
      phase=$phase; inspectedCount=$items.Count; capturedCount=$sample.Count
      expectedProcessId=[int64]$script:process.Id
      ownerHandle=[int64]$owner.Current.NativeWindowHandle
      ownerTitle=$owner.Current.Name; elements=$sample.ToArray()
    }
    $evidence|ConvertTo-Json -Depth 6 |
      Set-Content -LiteralPath (Join-Path $EvidenceDir ("uia-"+$safe+".json")) -Encoding UTF8
    $report.errors=@($report.errors)+@("UIA tree captured for "+$phase)
  }catch{
    $report.errors=@($report.errors)+@('UIA diagnostic failed: '+$_.Exception.Message)
    # Preserve the authenticated owner when enumeration fails; this is only
    # a diagnostic, not a substitute for a missing accessible child.
    try {
      [ordered]@{
        phase=$phase; diagnostic='SUBTREE_UNAVAILABLE'
        expectedProcessId=[int64]$script:process.Id
        ownerTitle=$owner.Current.Name
        ownerHandle=[int64]$owner.Current.NativeWindowHandle
        error=$_.Exception.Message
      }|ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $EvidenceDir ("uia-"+$safe+".json")) -Encoding UTF8
    }catch{}

  }
  try {
    $report.errors=@($report.errors)+@(
      'Failure-time desktop pixels: '+(Save-Visible-Desktop ("failed-"+$safe+".png"))
    )
  }catch{
    $report.errors=@($report.errors)+@('Failure screenshot unavailable: '+$_.Exception.Message)
  }
}

function Wait-Element([System.Windows.Automation.AutomationElement]$window,[string]$label,[bool]$button=$false){
  $deadline=[DateTime]::UtcNow.AddSeconds(20)
  do{
    $items=$window.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    foreach($item in $items){
      try{
        if($item.Current.Name -ne $label){continue}
        if($button -and $item.Current.ControlType -ne [System.Windows.Automation.ControlType]::Button){continue}
        return $item
      }catch{}
    }
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  Save-UIA-Failure $window ("missing-"+$label)
  throw "Installed WebView2 did not expose UI Automation element: $label"
}
# WebView2 Chromium controls can be accessible Button elements while not
# advertising InvokePattern. Try the supported UIA contracts first, then
# verified on-screen physical input. Never synthesize a successful dialog.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuP0Mouse {
  [DllImport("user32.dll", SetLastError=true)]
  public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll", SetLastError=true)]
  public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
'@
# Only an independently created top-level native Tauri window, owned by
# the same installed EXE process but with a DISTINCT Win32 handle, counts.
# A same-WebView overlay/dialog in the main Lu window cannot pass this gate.
function Wait-NativeWindow([string]$title){
  $deadline=[DateTime]::UtcNow.AddSeconds(30)
  do{
    $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
      [System.Windows.Automation.TreeScope]::Children,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    foreach($candidate in $top){
      try{
        if($candidate.Current.Name -ne $title){continue}
        if([int64]$candidate.Current.ProcessId -ne [int64]$script:process.Id){continue}
        if($candidate.Current.ControlType -ne [System.Windows.Automation.ControlType]::Window){continue}
        if($candidate.Current.NativeWindowHandle -eq 0){continue}
        if($candidate.Current.NativeWindowHandle -eq $script:window.Current.NativeWindowHandle){continue}
        if($candidate.Current.IsOffscreen){continue}
        return $candidate
      }catch{}
    }
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  throw "No distinct native top-level Lu window with installed process identity: $title"
}
function Invoke-UIA([System.Windows.Automation.AutomationElement]$element,[System.Windows.Automation.AutomationElement]$owner){
  if($null -eq $element -or $null -eq $owner){throw 'No trusted WebView UIA element or native owner'}
  if([int64]$owner.Current.ProcessId -ne [int64]$script:process.Id){
    throw 'Selected native window does not belong to installed Lu process'
  }
  $pattern=$null
  if($element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){
    ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
    return 'UIA_INVOKE'
  }
  # UIAutomationClient on modern PowerShell/.NET need not expose the
  # optional LegacyIAccessiblePattern type. Discover it reflectively, rather
  # than failing before the strictly verified on-screen fallback is tried.
  $legacyType='System.Windows.Automation.LegacyIAccessiblePattern' -as [type]
  if($null -ne $legacyType){
    $legacyField=$legacyType.GetField('Pattern')
    $legacyAction=$legacyType.GetMethod('DoDefaultAction')
    if($null -ne $legacyField -and $null -ne $legacyAction){
      $pattern=$null
      if($element.TryGetCurrentPattern($legacyField.GetValue($null),[ref]$pattern)){
        $null=$legacyAction.Invoke($pattern,[object[]]@())
        return 'UIA_LEGACY_DEFAULT'
      }
    }
  }
  # Last resort on a disposable, interactive runner only: click the point
  # that the CURRENT genuine UIA element says is reachable. The parent
  # native process identity was verified before calling this function.
  if($element.Current.IsOffscreen){throw 'Installed WebView control is offscreen'}
  $rect=$element.Current.BoundingRectangle
  $windowRect=$owner.Current.BoundingRectangle
  $point=$element.GetClickablePoint()
  if($rect.Width -lt 3 -or $rect.Height -lt 3 -or
     $point.X -lt $rect.Left -or $point.X -ge $rect.Right -or
     $point.Y -lt $rect.Top -or $point.Y -ge $rect.Bottom -or
     $point.X -lt $windowRect.Left -or $point.X -ge $windowRect.Right -or
     $point.Y -lt $windowRect.Top -or $point.Y -ge $windowRect.Bottom){
    throw 'WebView clickable point not within reviewed installed native window'
  }
  $x=[int][Math]::Round($point.X)
  $y=[int][Math]::Round($point.Y)
  if(-not([LuP0Mouse]::SetCursorPos($x,$y))){
    throw 'Could not position desktop pointer at genuine WebView UIA element'
  }
  [LuP0Mouse]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero)
  [LuP0Mouse]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
  return 'UIA_VERIFIED_POINT_OS_CLICK'
}
function Save-Visible-Desktop([string]$name){
  Add-Type -AssemblyName System.Windows.Forms
  Add-Type -AssemblyName System.Drawing
  $display=[System.Windows.Forms.Screen]::PrimaryScreen
  if($null -eq $display -or $display.Bounds.Width -lt 640 -or $display.Bounds.Height -lt 480){
    throw 'No adequately sized interactive Windows screen for screenshot'
  }
  $bmp=[System.Drawing.Bitmap]::new($display.Bounds.Width,$display.Bounds.Height)
  $g=[System.Drawing.Graphics]::FromImage($bmp)
  try{
    $g.CopyFromScreen($display.Bounds.Location,[System.Drawing.Point]::Empty,$display.Bounds.Size)
    $path=Join-Path $EvidenceDir $name
    $bmp.Save($path,[System.Drawing.Imaging.ImageFormat]::Png)
    if((Get-Item -LiteralPath $path).Length -lt 2000){throw 'Empty installed UI screenshot'}
    return $path
  }finally{$g.Dispose();$bmp.Dispose()}
}


Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuCp05Window {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsChild(IntPtr parent, IntPtr child);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [StructLayout(LayoutKind.Sequential)] private struct POINT { public int X; public int Y; }
  [DllImport("user32.dll")] private static extern IntPtr WindowFromPoint(POINT point);
  public static uint OwnerPid(IntPtr hwnd) {
    uint pid; return GetWindowThreadProcessId(hwnd, out pid) == 0 ? 0 : pid;
  }
  public static IntPtr HitWindow(int x, int y) {
    return WindowFromPoint(new POINT { X = x, Y = y });
  }
}
'@
function Verify-File([string]$path,[string]$sha){
  $f=Get-Item -LiteralPath $path -Force
  if($f.PSIsContainer -or ($f.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
     (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash -ine $sha){
    throw 'Exact installed/model file identity failed'
  }
}
function Wait-Observation([scriptblock]$probe,[int]$seconds,[string]$label){
  $until=[DateTime]::UtcNow.AddSeconds($seconds)
  do{
    $value=& $probe
    if($value){return $value}
    Start-Sleep -Milliseconds 100
  }while([DateTime]::UtcNow -lt $until)
  throw "Deadline: $label"
}
function Wait-Enabled([string]$label,[int]$seconds=40){
  return Wait-Observation {
    $items=$script:window.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition)
    foreach($item in $items){
      try{
        if($item.Current.Name -ceq $label -and $item.Current.IsEnabled -and
           -not $item.Current.IsOffscreen){return $item}
      }catch{}
    }
  } $seconds $label
}
function Start-Lu([string]$phase){
  $script:process=Start-Process -FilePath $InstalledExe -WorkingDirectory (Split-Path -Parent $InstalledExe) -PassThru
  $null=Wait-Observation {
    $script:process.Refresh()
    if($script:process.HasExited){throw 'Installed Lu exited before GUI'}
    $script:process.MainWindowHandle -ne [IntPtr]::Zero
  } 60 'main window'
  $script:window=[System.Windows.Automation.AutomationElement]::FromHandle($script:process.MainWindowHandle)
  if($null -eq $script:window -or $script:window.Current.ProcessId -ne $script:process.Id){throw 'Wrong installed HWND identity'}
  # A Win32 HWND and WRY pane can appear before the WebView2 document.
  # The container window is not required to support keyboard focus. Use
  # the same bounded descendant readiness condition as the passing UI smoke.
  $report.currentPhase=$phase+'-wait-webview'
  $ready=Wait-Enabled 'Choose a model' 60
  if($script:window.Current.ProcessId -ne $script:process.Id -or
     $script:window.Current.NativeWindowHandle -ne $script:process.MainWindowHandle.ToInt64() -or
     $ready.Current.IsOffscreen -or -not $ready.Current.IsEnabled){
    throw 'WebView readiness evidence lost original installed owner'
  }
  # Foreground activation is best-effort for UIA invocation, not a test
  # success condition or bypass. Keyboard input is separately fenced.
  try{$null=[LuCp05Window]::SetForegroundWindow($script:process.MainWindowHandle)}catch{}
  $report.phases[$phase+'-webview']=[ordered]@{
    appPid=$script:process.Id;windowHandle=$script:process.MainWindowHandle.ToInt64()
    readyControl=$ready.Current.Name;webViewDescendantReady=$true
  }
  $null=Save-Visible-Desktop ($phase+'-window.png')
}
function Engine-Probe {
  # Inspect only this exact owned process and its private loopback health.
  # Never emit the command line or bearer token to logs/evidence.
  $rows=@(Get-CimInstance Win32_Process -Filter ("ParentProcessId="+$script:process.Id+" AND Name='llama-server.exe'"))
  if($rows.Count -eq 0){return}
  if($rows.Count -ne 1){throw 'Unexpected multiple owned inference engines'}
  $row=$rows[0]
  if($row.ExecutablePath -ine $enginePath){throw 'Inference engine not installed reviewed executable'}
  if(-not $row.CommandLine -or $row.CommandLine.IndexOf($script:registeredModel,[StringComparison]::OrdinalIgnoreCase) -lt 0){throw 'Wrong loaded model path'}
  $port=[regex]::Match($row.CommandLine,'--port\s+"?([0-9]{1,5})"?')
  $key=[regex]::Match($row.CommandLine,'--api-key\s+"?([a-f0-9]{64})"?')
  if(-not $port.Success -or -not $key.Success -or [int]$port.Groups[1].Value -lt 1024){throw 'Unrecognized private loopback endpoint'}
  $p=Get-Process -Id $row.ProcessId -ErrorAction Stop
  # Retain the exact kernel process handle to defeat PID reuse.
  $null=$p.Handle
  return [pscustomobject]@{Proc=$p;Port=[int]$port.Groups[1].Value;Key=$key.Groups[1].Value;Pid=$p.Id;Start=$p.StartTime.ToUniversalTime().ToString('o')}
}
function Slot-State([bool]$active){
  if($null -eq $script:engine -or $script:engine.Proc.HasExited){return $false}
  try{
    $s=Invoke-RestMethod -Uri ("http://127.0.0.1:"+$script:engine.Port+"/slots") -Headers @{Authorization=('Bearer '+$script:engine.Key)} -TimeoutSec 2 -NoProxy
    return (@($s).Count -eq 1 -and $s[0].is_processing -is [bool] -and $s[0].is_processing -eq $active)
  }catch{return $false}
}
function Load-Lu([string]$phase){
  $report.currentPhase=$phase+'-load-model'
  $report.activationMethods+=@(Invoke-UIA (Wait-Enabled 'Choose a model') $script:window)
  $report.activationMethods+=@(Invoke-UIA (Wait-Enabled 'Load selected' 60) $script:window)
  $script:engine=Wait-Observation { Engine-Probe } 180 'actual installed inference process'
  Verify-File $enginePath $EngineSha256
  $null=Wait-Observation { Slot-State $false } 180 'real GGUF idle decoder'
  $null=Wait-Element $script:window 'Ready to chat'
  $null=Save-Visible-Desktop ($phase+'-loaded.png')
  $report.phases[$phase]=[ordered]@{appPid=$script:process.Id;enginePid=$script:engine.Pid;engineStart=$script:engine.Start;actualIdleSlot=$true}
}
function Send-Fixture([string]$prompt){
  $input=Wait-Enabled 'Message Lu'
  if($script:window.Current.ProcessId -ne $script:process.Id){throw 'Composer owner mismatch'}
  $null=[LuCp05Window]::SetForegroundWindow($script:process.MainWindowHandle)
  # WebView edit controls may not implement SetFocus even when reachable.
  # Do not type into an unverified target or accept a guessed coordinate.
  $runtimeId=($input.GetRuntimeId() -join ',')
  try{$input.SetFocus()}catch{Write-Warning 'Composer UIA focus unavailable; checking genuine clickable control'}
  function Test-ComposerFocus {
    try{
      $focused=[System.Windows.Automation.AutomationElement]::FocusedElement
      return ($null -ne $focused -and
        ($focused.GetRuntimeId() -join ',') -ceq $runtimeId -and
        $focused.Current.Name -ceq 'Message Lu' -and
        [LuCp05Window]::GetForegroundWindow() -eq $script:process.MainWindowHandle)
    }catch{return $false}
  }
  if(-not (Test-ComposerFocus)){
    if([LuCp05Window]::GetForegroundWindow() -ne $script:process.MainWindowHandle){throw 'Composer is not foreground'}
    $rect=$input.Current.BoundingRectangle;$owner=$script:window.Current.BoundingRectangle
    $point=$input.GetClickablePoint()
    if($input.Current.IsOffscreen -or $rect.Width -lt 3 -or $rect.Height -lt 3 -or
       $point.X -lt $rect.Left -or $point.X -ge $rect.Right -or
       $point.Y -lt $rect.Top -or $point.Y -ge $rect.Bottom -or
       $point.X -lt $owner.Left -or $point.X -ge $owner.Right -or
       $point.Y -lt $owner.Top -or $point.Y -ge $owner.Bottom){throw 'Composer point outside verified installed window'}
    if(-not [LuP0Mouse]::SetCursorPos([int][Math]::Round($point.X),[int][Math]::Round($point.Y))){
      throw 'Could not activate genuine composer'
    }
    [LuP0Mouse]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero)
    [LuP0Mouse]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
    $null=Wait-Observation { Test-ComposerFocus } 5 'verified actual composer focus'
  }
  Add-Type -AssemblyName System.Windows.Forms
  if(-not (Test-ComposerFocus)){throw 'Composer focus lost before keyboard input'}
  [System.Windows.Forms.Clipboard]::SetText($prompt)
  [System.Windows.Forms.SendKeys]::SendWait('^a')
  if(-not (Test-ComposerFocus)){throw 'Composer focus lost before paste'}
  [System.Windows.Forms.SendKeys]::SendWait('^v')
  $report.activationMethods+=@('UIA_EXACT_FOCUSED_COMPOSER_KEYBOARD_PASTE')
  $report.activationMethods+=@(Invoke-UIA (Wait-Enabled 'Send' 20) $script:window)
}
function Check-Journal([string]$phase,[int]$seconds=90){
  $raw=Wait-Observation {
    $result=& python $oracle $profile $phase 2>&1
    if($LASTEXITCODE -eq 0){return ($result -join [Environment]::NewLine)}
    # Preserve only the synthetic oracle error, not a complete journal dump.
    ($result -join [Environment]::NewLine)|Set-Content -LiteralPath (Join-Path $EvidenceDir ($phase+'-oracle-last-error.txt')) -Encoding utf8
  } $seconds ('native read-only journal '+$phase)
  $raw|Set-Content -LiteralPath (Join-Path $EvidenceDir ($phase+'-journal.json')) -Encoding utf8
}
function Test-ConsentControlIdentity($current,[string]$action,[int]$ownerPid){
  # Windows native TaskDialog exposes CCPushButton as Pane in some UIA
  # providers. This narrow exception is NEVER used for WebView controls,
  # arbitrary panes, or a label that belongs to another process.
  $id=switch -CaseSensitive ($action){
    'Allow once'{'CommandButton_1000'}
    'Cancel'{'CommandButton_1001'}
    default{return $false}
  }
  return ($current.Name -ceq $action -and
    $current.AutomationId -ceq $id -and
    $current.ClassName -ceq 'CCPushButton' -and
    $current.ControlType.ProgrammaticName -cin @('ControlType.Pane','ControlType.Button') -and
    [int64]$current.ProcessId -eq $ownerPid -and
    [int64]$current.NativeWindowHandle -ne 0 -and
    $current.IsEnabled -and -not $current.IsOffscreen)
}
# Separate GUI provider identity from the Win32 hit proof. A UIA ID such as
# CommandButton_1000 is NOT a documented assertion about GetDlgCtrlID.
# Native IDs and provider HWND ancestry are retained as diagnostics.
function Test-ConsentHitIdentity($proof){
  return ($proof.ownerLive -and $proof.controlLive -and
    $proof.sameModalForeground -and $proof.hitLive -and
    $proof.hitInModal -and ($proof.nativeTargetMatch -or $proof.uiaTargetMatch))
}
function Consent([string]$action,[string]$phase){
  # Bind this native human-fixture interaction to the actual displayed
  # request text, not merely the first button with a matching display label.
  $report.currentPhase=$phase+'-consent'
  if($phase -ceq 'approve' -and $action -ceq 'Allow once'){
    $request='Could you validate this JSON schema {"schema":{"type":"integer"},"data":17}'
    $controlId=1000
  }elseif($phase -ceq 'deny' -and $action -ceq 'Cancel'){
    $request='Could you validate this JSON schema {"schema":{"type":"integer"},"data":18}'
    $controlId=1001
  }else{throw 'Unexpected synthetic consent phase/action'}
  $dialog=Wait-NativeWindow 'Allow this inspection?'
  $script:consentDialog=$dialog
  $owner=[IntPtr][int64]$dialog.Current.NativeWindowHandle
  if($owner -eq [IntPtr]::Zero -or $owner -eq [IntPtr][int64]$script:window.Current.NativeWindowHandle -or
     [LuCp05Window]::OwnerPid($owner) -ne $script:process.Id -or
     -not [LuCp05Window]::IsWindowVisible($owner)){
    throw 'Consent dialog is not an exact visible native modal of installed Lu'
  }
  $null=Save-Visible-Desktop ($phase+'-native-dialog.png')
  $expected="Lu suggests using the exact schema and data from your message (offline validation only) for this request:`n`n$request`n`nNo files will be changed."
  $elements=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition)
  $content=@($elements|Where-Object{
    try{$_.Current.AutomationId -ceq 'ContentText' -and
       $_.Current.ProcessId -eq $script:process.Id -and
       $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and
       -not $_.Current.IsOffscreen -and
       ($_.Current.Name.Replace("`r`n","`n")) -ceq $expected}catch{$false}
  })
  if($content.Count -ne 1){throw 'Native consent text does not match exact synthetic request and scope'}
  $matches=@($elements|Where-Object{
    try{Test-ConsentControlIdentity $_.Current $action $script:process.Id}catch{$false}
  })
  if($matches.Count -ne 1){throw 'Expected exactly one owned native consent command control'}
  $button=$matches[0]
  $child=[IntPtr][int64]$button.Current.NativeWindowHandle
  $controlRuntimeId=($button.GetRuntimeId() -join ',')
  $dialogRuntimeId=($dialog.GetRuntimeId() -join ',')
  function Get-ConsentTargetSnapshot {
    # The exact source content and unique provider command were checked
    # above. Revalidate the same live identities before input, including
    # a fresh enumeration from the original native dialog.
    $currentDialog=[System.Windows.Automation.AutomationElement]::FromHandle($owner)
    $sameDialog=($null -ne $currentDialog -and
      $currentDialog.Current.Name -ceq 'Allow this inspection?' -and
      $currentDialog.Current.ProcessId -eq $script:process.Id -and
      [int64]$currentDialog.Current.NativeWindowHandle -eq $owner.ToInt64())
    $allFresh=$currentDialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition)
    # Recheck the original request and scope on every pre-click snapshot.
    $freshContent=@($allFresh|Where-Object{
      try{$_.Current.AutomationId -ceq 'ContentText' -and
         $_.Current.ProcessId -eq $script:process.Id -and
         $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and
         -not $_.Current.IsOffscreen -and
         ($_.Current.Name.Replace("`r`n","`n")) -ceq $expected}catch{$false}
    })
    $fresh=@($allFresh|Where-Object{
      try{Test-ConsentControlIdentity $_.Current $action $script:process.Id}catch{$false}
    })
    $controlLive=($fresh.Count -eq 1 -and
      ($fresh[0].GetRuntimeId() -join ',') -ceq $controlRuntimeId -and
      [int64]$fresh[0].Current.NativeWindowHandle -eq $child.ToInt64())
    $ownerPidNow=[LuCp05Window]::OwnerPid($owner)
    $childPidNow=[LuCp05Window]::OwnerPid($child)
    $ownerValid=[LuCp05Window]::IsWindow($owner)
    $childValid=[LuCp05Window]::IsWindow($child)
    $ownerVisible=[LuCp05Window]::IsWindowVisible($owner)
    $childVisible=[LuCp05Window]::IsWindowVisible($child)
    $childEnabled=[LuCp05Window]::IsWindowEnabled($child)
    $childInDialog=[LuCp05Window]::IsChild($owner,$child)
    $childRoot=[LuCp05Window]::GetAncestor($child,2)
    return [ordered]@{
      ownerLive=($sameDialog -and $freshContent.Count -eq 1 -and $ownerValid -and $ownerVisible -and $ownerPidNow -eq $script:process.Id)
      controlLive=($controlLive -and $childValid -and $childVisible -and $childEnabled -and $childPidNow -eq $script:process.Id)
      dialogHandle=$owner.ToInt64();commandHandle=$child.ToInt64()
      ownerPid=$ownerPidNow;childPid=$childPidNow;ownerValid=$ownerValid;childValid=$childValid
      ownerVisible=$ownerVisible;childVisible=$childVisible;childEnabled=$childEnabled
      sameDialog=$sameDialog;exactRequestText=($freshContent.Count -eq 1)
      uniqueLiveCommand=$controlLive;nativeChildOfDialog=$childInDialog
      nativeChildRoot=$childRoot.ToInt64()
      providerAutomationId=$button.Current.AutomationId
      observedNativeControlId=[LuCp05Window]::GetDlgCtrlID($child)
    }
  }
  function Save-ConsentSnapshot($snapshot,[string]$stage){
    $snapshot|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $EvidenceDir ($phase+'-consent-'+$stage+'.json')) -Encoding utf8
  }
  $before=Get-ConsentTargetSnapshot
  Save-ConsentSnapshot $before 'identity'
  if(-not $before.ownerLive -or -not $before.controlLive){
    throw 'Exact dialog or live consent control identity rejected; see individual snapshot predicates'
  }
  $null=[LuCp05Window]::SetForegroundWindow($owner)
  $null=Wait-Observation {[LuCp05Window]::GetForegroundWindow() -eq $owner} 5 'original native consent modal foreground'
  $rect=$button.Current.BoundingRectangle
  $bounds=$dialog.Current.BoundingRectangle
  try{$point=$button.GetClickablePoint()}catch{
    $point=[System.Windows.Point]::new($rect.Left+$rect.Width/2,$rect.Top+$rect.Height/2)
  }
  $x=[int][Math]::Round($point.X);$y=[int][Math]::Round($point.Y)
  if($rect.Width -lt 3 -or $rect.Height -lt 3 -or
     $x -lt $rect.Left -or $x -ge $rect.Right -or $y -lt $rect.Top -or $y -ge $rect.Bottom -or
     $x -lt $bounds.Left -or $x -ge $bounds.Right -or $y -lt $bounds.Top -or $y -ge $bounds.Bottom){
    throw 'Native consent command point outside exact control or modal'
  }
  function Get-ConsentHitProof {
    $proof=Get-ConsentTargetSnapshot
    $hit=[LuCp05Window]::HitWindow($x,$y)
    $hitRoot=[LuCp05Window]::GetAncestor($hit,2)
    $proof.sameModalForeground=([LuCp05Window]::GetForegroundWindow() -eq $owner)
    $proof.hitLive=([LuCp05Window]::IsWindow($hit) -and
      [LuCp05Window]::OwnerPid($hit) -eq $script:process.Id -and
      [LuCp05Window]::IsWindowVisible($hit) -and [LuCp05Window]::IsWindowEnabled($hit))
    $proof.hitInModal=($hit -eq $owner -or $hitRoot -eq $owner -or [LuCp05Window]::IsChild($owner,$hit))
    $proof.nativeTargetMatch=($hit -eq $child -or [LuCp05Window]::IsChild($child,$hit))
    # TaskDialog may hit its painted parent rather than the provider child.
    # Only accept that case if a fresh UIA point hit resolves to the SAME
    # command runtime ID, exact label and provider identity in this modal.
    $proof.uiaTargetMatch=$false
    try{
      $at=[System.Windows.Automation.AutomationElement]::FromPoint([System.Windows.Point]::new($x,$y))
      for($i=0;$i -lt 8 -and $null -ne $at;$i++){
        $atId=($at.GetRuntimeId() -join ',')
        if($atId -ceq $dialogRuntimeId){break}
        if($atId -ceq $controlRuntimeId){
          $proof.uiaTargetMatch=(Test-ConsentControlIdentity $at.Current $action $script:process.Id)
          break
        }
        # Do not climb through another command to manufacture a match.
        if($at.Current.ClassName -ceq 'CCPushButton'){break}
        $at=[System.Windows.Automation.TreeWalker]::RawViewWalker.GetParent($at)
      }
    }catch{}
    $proof.hitHandle=$hit.ToInt64();$proof.hitRoot=$hitRoot.ToInt64()
    $proof.x=$x;$proof.y=$y
    $proof.accepted=Test-ConsentHitIdentity $proof
    return $proof
  }
  $hitProof=Get-ConsentHitProof
  Save-ConsentSnapshot $hitProof 'hit-before'
  if(-not $hitProof.accepted){throw 'Exact modal/control native hit proof rejected; see hit-before snapshot'}
  if(-not [LuP0Mouse]::SetCursorPos($x,$y)){throw 'Cannot position pointer at verified consent control'}
  # Revalidate original modal, exact provider command and both native / UIA
  # point identities immediately before one OS click. No default key, RPC,
  # generic Pane click, or second click on timeout.
  $hitProof=Get-ConsentHitProof
  Save-ConsentSnapshot $hitProof 'hit-final'
  if(-not $hitProof.accepted){throw 'Consent target changed before physical click; see hit-final snapshot'}
  $report.phases[$phase+'-dialog']=[ordered]@{
    appPid=$script:process.Id;dialogHandle=$owner.ToInt64()
    commandHandle=$child.ToInt64();providerCommandId=$controlId
    observedNativeControlId=$hitProof.observedNativeControlId
    nativeChildOfDialog=$hitProof.nativeChildOfDialog;nativeTargetMatch=$hitProof.nativeTargetMatch
    uiaTargetMatch=$hitProof.uiaTargetMatch
    className=$button.Current.ClassName;controlType=$button.Current.ControlType.ProgrammaticName
    action=$action;exactRequestText=$true;nativeOwnedHit=$true
  }
  [LuP0Mouse]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero)
  [LuP0Mouse]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
  $null=Wait-Observation {(-not [LuCp05Window]::IsWindow($owner)) -or
    (-not [LuCp05Window]::IsWindowVisible($owner))} 15 'original consent dialog dismissed'
  $report.phases[$phase+'-dialog'].dismissed=$true
  $script:consentDialog=$null
  $report.activationMethods+=@('EXACT_NATIVE_CONSENT_COMMAND_HIT_TEST_OS_CLICK')
  $report.currentPhase=$phase+'-journal'
}
function Start-Long([string]$marker,[string]$phase){
  $report.currentPhase=$phase+'-active-generation'
  Send-Fixture ('No tools. Write an extremely long original fictional story with 100 detailed paragraphs about '+$marker+'.')
  $null=Wait-Observation { Slot-State $true } 60 'actual active inference slot'
  $null=Save-Visible-Desktop ($phase+'-active.png')
}
function Close-Exact([bool]$force,[string]$phase){
  $p=$script:process;$worker=$script:engine.Proc
  if($p.HasExited -or $worker.HasExited){throw 'Active close lost original process'}
  if(-not (Slot-State $true)){throw 'No independently active slot at close'}
  if($force){$p.Kill()}elseif(-not $p.CloseMainWindow()){throw 'Normal native WM_CLOSE failed'}
  if(-not $p.WaitForExit(30000)){throw 'Original Lu did not exit'}
  if(-not $worker.WaitForExit(15000)){throw 'Original inference engine survived Lu exit'}
  $report.phases[$phase]=[ordered]@{appPid=$p.Id;enginePid=$worker.Id;appExit=$p.ExitCode;activeSlot=$true;workerExited=$true;force=$force}
  if(-not $force -and $p.ExitCode -ne 0){throw 'Unexpected graceful exit code'}
  $script:engine=$null
}
try{
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  Verify-File $InstalledExe $ExpectedSha256
  Verify-File $enginePath $EngineSha256
  Verify-File $ModelPath $ModelSha256
  if(-not (Test-Path -LiteralPath (Join-Path $profile 'lifecycle-retain-this-data.txt')) -or
     (Test-Path -LiteralPath $models) -or (Test-Path -LiteralPath (Join-Path $profile 'ansf'))){
    throw 'Expected fresh isolated native NSIS test profile'
  }
  $null=New-Item -ItemType Directory -Path $models
  $script:registeredModel=Join-Path $models 'Qwen3-0.6B.Q4_K_M.gguf'
  Copy-Item -LiteralPath $ModelPath -Destination $script:registeredModel
  Verify-File $script:registeredModel $ModelSha256
  # Seed actual pinned bytes in Lu's normal model directory. Discovery and
  # selected Load are through the installed application; no Tauri test hook.
  Start-Lu 'initial'
  Load-Lu 'initial'
  $report.currentPhase='approve'
  Send-Fixture 'Could you validate this JSON schema {"schema":{"type":"integer"},"data":17}'
  Consent 'Allow once' 'approve'
  Check-Journal 'approve' 180
  $report.genuineGgufInference='PASS_GENUINE_TOOL_AND_FINAL'
  $null=Save-Visible-Desktop 'approved-final.png'
  $report.currentPhase='deny'
  Send-Fixture 'Could you validate this JSON schema {"schema":{"type":"integer"},"data":18}'
  Consent 'Cancel' 'deny'
  Check-Journal 'deny' 90
  $null=Save-Visible-Desktop 'refused.png'
  Start-Long 'CP05-WINDOWS-STOP-ORCHID' 'stop'
  $report.activationMethods+=@(Invoke-UIA (Wait-Enabled 'Cancel') $script:window)
  Check-Journal 'stop' 120
  # Cancellation may legitimately unload this model. Do not auto-reload or
  # reset the restart circuit. A missing resident idle slot fails this witness.
  $null=Wait-Observation { Slot-State $false } 120 'original decoder idle after Stop'
  $report.phases.stop=[ordered]@{actualActiveSlot=$true;cancelReceipt=$true;idle=$true}
  Start-Long 'CP05-WINDOWS-CLOSE-AZURE' 'graceful'
  Close-Exact $false 'graceful'
  # Both distinct original workers must be observed; no more than two starts
  # within the production circuit window, and no counter manipulation.
  Start-Lu 'second'
  Check-Journal 'graceful' 30
  Load-Lu 'second'
  Start-Long 'CP05-WINDOWS-DEATH-CEDAR' 'forced'
  Close-Exact $true 'forced'
  Start-Lu 'reopened'
  Check-Journal 'forced' 30
  $null=Wait-Element $script:window 'New Chat' $true
  Start-Sleep -Seconds 2
  if(@(Get-CimInstance Win32_Process -Filter ("ParentProcessId="+$script:process.Id+" AND Name='llama-server.exe'")).Count -ne 0){
    throw 'Recovery automatically spawned a model engine'
  }
  & python $oracle $profile all --output (Join-Path $EvidenceDir 'reopened-all-journals.json')
  if($LASTEXITCODE -ne 0){throw 'Exact reopened journal verification failed'}
  $null=Save-Visible-Desktop 'reopened-no-replay.png'
  if(-not $script:process.CloseMainWindow() -or -not $script:process.WaitForExit(30000)){throw 'Reopened app failed closing'}
  $report.phases.reopened=[ordered]@{noEngineReplay=$true;allFiveOriginalTasksVerified=$true}
  $report.currentPhase='complete'
  $report.result='PASS_INSTALLED_GGUF_NATIVE_CONSENT_AND_LIFECYCLE'
}catch{
  # Never print process command lines or the ephemeral bearer.
  $report.errors+=@($_.Exception.Message)
  try{if($null -ne $script:consentDialog){Save-UIA-Failure $script:consentDialog 'cp05-consent'}}catch{}
  try{if($null -ne $script:window){Save-UIA-Failure $script:window 'cp05-installed'}}catch{}
  Write-Warning ('Installed CP-05E5 witness NOT ACCEPTED: '+$_.Exception.Message)
}finally{
  # Cleanup is deliberately NOT counted as tested normal/forced termination.
  if($null -ne $script:process -and -not $script:process.HasExited){
    try{$script:process.Kill($true);$null=$script:process.WaitForExit(10000)}catch{}
  }
  if($null -ne $script:engine -and -not $script:engine.Proc.HasExited){
    try{$script:engine.Proc.Kill();$null=$script:engine.Proc.WaitForExit(10000)}catch{}
  }
  $script:engine=$null
  $report|ConvertTo-Json -Depth 9|Set-Content -LiteralPath (Join-Path $EvidenceDir 'cp05-installed-gguf.json') -Encoding utf8
}
if($report.result -ne 'PASS_INSTALLED_GGUF_NATIVE_CONSENT_AND_LIFECYCLE'){exit 2}
exit 0
