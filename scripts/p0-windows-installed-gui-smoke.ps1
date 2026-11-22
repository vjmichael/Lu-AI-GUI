# Genuine installed Windows Tauri/WebView2 smoke; NO GGUF or tool effects.
# Intended only for a disposable, explicitly confirmed Windows test host.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
  throw 'Installed UI smoke requires explicitly confirmed disposable Windows'
}
if(-not(Test-Path -LiteralPath $InstalledExe -PathType Leaf) -or
   (Get-FileHash -LiteralPath $InstalledExe -Algorithm SHA256).Hash -ine $ExpectedSha256){
  throw 'Native installed executable SHA-256 differs from the reviewed build'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'UI evidence path must not already exist'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$report=[ordered]@{
  schema='lu-c64-real-windows-unified-tabs-smoke-v1'
  installedExeSha256=$ExpectedSha256.ToLowerInvariant()
  nativeWindow='NOT_TESTED'
  uiAutomationProcessIdentity='NOT_TESTED'
  singleSettingsEntry='NOT_TESTED'
  settingsTab='NOT_TESTED'
  modelsTab='NOT_TESTED'
  historyTab='NOT_TESTED'
  tasksTab='NOT_TESTED'
  chatTabReturned='NOT_TESTED'
  noAuxiliaryWindows='NOT_TESTED'
  settingsScreenshot='NOT_TESTED'
  modelLibraryScreenshot='NOT_TESTED'
  genuineGgufInference='NOT_TESTED'
  activeGpuOffload='NOT_TESTED'
  result='NOT_ACCEPTED'
  activationMethods=@()
  errors=@()
}
$process=$null
# Bind the accessibility witness to an actual descendant HWND of the
# exact installed Lu main window. WebView2 may host its provider in a child
# HWND that the top-level Tauri UIA provider does not forward on hosted Windows.
# This is not a screenshot/coordinate pass and cannot invent a missing button.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuP0UIAChildren {
  public delegate bool ChildCallback(IntPtr handle, IntPtr unused);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, ChildCallback callback, IntPtr data);
  [DllImport("user32.dll")] public static extern bool IsChild(IntPtr parent, IntPtr candidate);
}
'@
function Get-InstalledUIAElements([System.Windows.Automation.AutomationElement]$owner) {
  $rootHandle=[IntPtr]$owner.Current.NativeWindowHandle
  if($rootHandle -eq [IntPtr]::Zero){throw 'Installed UIA owner has no native HWND'}
  $handles=[System.Collections.Generic.List[IntPtr]]::new()
  $callback=[LuP0UIAChildren+ChildCallback]{
    param([IntPtr]$handle,[IntPtr]$unused)
    if($handles.Count -lt 128){$handles.Add($handle)}
    return $true
  }
  $null=[LuP0UIAChildren]::EnumChildWindows($rootHandle,$callback,[IntPtr]::Zero)
  $roots=[System.Collections.Generic.List[System.Windows.Automation.AutomationElement]]::new()
  $roots.Add($owner)
  foreach($handle in $handles){
    if(-not([LuP0UIAChildren]::IsChild($rootHandle,$handle))){continue}
    try{
      $child=[System.Windows.Automation.AutomationElement]::FromHandle($handle)
      if($null -ne $child){$roots.Add($child)}
    }catch{}
  }
  $results=[System.Collections.Generic.List[System.Windows.Automation.AutomationElement]]::new()
  $seen=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
  foreach($root in $roots){
    try{
      $items=$root.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
      )
      foreach($item in $items){
        $identity=($item.GetRuntimeId() -join '.')
        if($identity -and $seen.Add($identity)){$results.Add($item)}
        if($results.Count -ge 4096){return @($results.ToArray())}
      }
    }catch{} # Inaccessible WebView provider never establishes acceptance.
  }
  return @($results.ToArray())
}
# Genuine UIAutomation tree and pixel evidence when an installed WebView fails.
# Never use this snapshot to manufacture an element or pass a gate.
function Save-UIA-Failure([System.Windows.Automation.AutomationElement]$owner,[string]$phase){
  if($null -eq $owner){return}
  $safe=($phase -replace '[^a-zA-Z0-9._-]','-')
  try {
    $items=@(Get-InstalledUIAElements $owner)
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
    $items=@(Get-InstalledUIAElements $window)
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
try{
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  # Force the real WebView2 renderer to publish accessibility to this one
  # disposable test process. Do not mutate production Lu defaults or permit
  # arbitrary browser flags/remote debugging on the acceptance host.
  $priorArgs=[Environment]::GetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','Process')
  if(-not [string]::IsNullOrEmpty($priorArgs)){
    throw 'Unexpected existing WebView2 browser argument override in installed acceptance'
  }
  [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','--force-renderer-accessibility','Process')
  try{
    $process=Start-Process -FilePath $InstalledExe -WorkingDirectory (Split-Path -Parent $InstalledExe) -PassThru
  }finally{
    [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS',$priorArgs,'Process')
  }
  $deadline=[DateTime]::UtcNow.AddSeconds(45)
  do{
    $process.Refresh()
    if($process.HasExited){throw 'Installed Lu exited before native window appeared'}
    if($process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  if($process.MainWindowHandle -eq [IntPtr]::Zero){throw 'Installed Lu never opened native GUI window'}
  $report.nativeWindow='PASS_WIN32_WINDOW_HANDLE'
  $window=[System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
  if($null -eq $window -or [int64]$window.Current.ProcessId -ne [int64]$process.Id){
    throw 'Genuine UIA window does not belong to the installed Lu process'
  }
  $report.uiAutomationProcessIdentity='PASS_EXACT_OS_PROCESS'
  try{$window.SetFocus()}catch{Write-Warning ('Installed main-window focus unavailable: '+$_.Exception.Message)}
  $settings=Wait-Element $window 'Settings' $true
  $all=@(Get-InstalledUIAElements $window)
  $settingsButtons=0
  foreach($control in $all){
    try{
      if($control.Current.Name -eq 'Settings' -and
         $control.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button){
        $settingsButtons++
      }
    }catch{}
  }
  if($settingsButtons -ne 1){throw "Expected exactly one Settings button; observed $settingsButtons"}
  $report.singleSettingsEntry='PASS_ONLY_TOP_RIGHT_GEAR'
  $report.activationMethods+=@(Invoke-UIA $settings $window)
  # Genuine accessibility controls in the SAME verified installed Win32 window.
  $null=Wait-Element $window 'General & appearance' $true
  $null=Wait-Element $window 'Tools & permissions' $true
  $report.settingsTab='PASS_SAME_NATIVE_WINDOW'
  $report.settingsScreenshot=Save-Visible-Desktop 'installed-settings.png'
  $report.activationMethods+=@(Invoke-UIA (Wait-Element $window 'Models' $true) $window)
  # Require controls that exist in the genuine current Model Library.
  # A source-only rename must not be treated as an installed accessibility PASS.
  $null=Wait-Element $window 'My models' $true
  $null=Wait-Element $window 'Search local models'
  $null=Wait-Element $window 'Discover · Hugging Face' $true
  $report.modelsTab='PASS_SAME_NATIVE_WINDOW'
  $report.modelLibraryScreenshot=Save-Visible-Desktop 'installed-model-library.png'
  $report.activationMethods+=@(Invoke-UIA (Wait-Element $window 'History' $true) $window)
  $null=Wait-Element $window 'Search all conversations'
  $report.historyTab='PASS_SAME_NATIVE_WINDOW'
  $report.activationMethods+=@(Invoke-UIA (Wait-Element $window 'Tasks' $true) $window)
  $null=Wait-Element $window 'Background work' $true
  $null=Wait-Element $window 'Project plans' $true
  $report.tasksTab='PASS_SAME_NATIVE_WINDOW'
  $report.activationMethods+=@(Invoke-UIA (Wait-Element $window 'Chat' $true) $window)
  $null=Wait-Element $window 'Choose a model' $true
  $null=Wait-Element $window 'New Chat' $true
  $report.chatTabReturned='PASS_SAME_NATIVE_WINDOW'
  $process.Refresh()
  if($process.MainWindowHandle -ne [IntPtr]$window.Current.NativeWindowHandle){
    throw 'Main installed window was replaced during tab navigation'
  }
  $top=[System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    [System.Windows.Automation.Condition]::TrueCondition
  )
  foreach($candidate in $top){
    if($candidate.Current.ProcessId -eq $process.Id -and
       $candidate.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
       $candidate.Current.NativeWindowHandle -ne 0 -and
       $candidate.Current.NativeWindowHandle -ne $window.Current.NativeWindowHandle -and
       -not $candidate.Current.IsOffscreen){
      throw 'Unexpected independent Lu window during tab navigation'
    }
  }
  $report.noAuxiliaryWindows='PASS_ONE_INSTALLED_WINDOW'
  $report.result='PASS_INSTALLED_UNIFIED_TABS'
}catch{
  $report.errors=@($report.errors)+@($_.Exception.Message)
  Write-Warning ('Genuine installed UI smoke is NOT ACCEPTED: '+$_.Exception.Message)
}finally{
  if($null -ne $process){
    try{
      $process.Refresh()
      if(-not $process.HasExited){
        if($process.CloseMainWindow()){
          if(-not $process.WaitForExit(15000)){$process.Kill($true)}
        }else{$process.Kill($true)}
        $null=$process.WaitForExit(10000)
      }
    }catch{
      $report.errors=@($report.errors)+@('Failed to reap owned GUI process: '+$_.Exception.Message)
      $report.result='NOT_ACCEPTED'
    }
  }
  $report|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $EvidenceDir 'p0-installed-ui.json') -Encoding UTF8
}
if($report.result -ne 'PASS_INSTALLED_UNIFIED_TABS'){exit 2}
exit 0
