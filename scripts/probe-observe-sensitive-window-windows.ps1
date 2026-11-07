# P06 native OS/UIA sensitive-control diagnostic. Read-only and opt-in.
# Requires an explicitly known window and exact process lifetime. Never
# reads screen pixels, text values, window titles, keystrokes or passwords.
# Its limited coverage CANNOT authorize WGC image/model pixel release.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][long]$WindowHandle,
  [Parameter(Mandatory=$true)][uint32]$ExpectedPid,
  [Parameter(Mandatory=$true)][long]$ExpectedBirthFileTime,
  [ValidateRange(1,128)][int]$MaxRegions=128
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or $WindowHandle -le 0 -or
   $ExpectedPid -eq 0 -or $ExpectedBirthFileTime -le 0){
  throw 'Explicit valid native Windows HWND, PID and process birth are required'
}
if(-not ('LuP06WinIdentity' -as [type])){
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuP06WinIdentity {
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd,uint mode);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd,int attribute,out uint result,uint size);
}
'@
}
$hwnd=[IntPtr]::new($WindowHandle)
if(-not [LuP06WinIdentity]::IsWindow($hwnd) -or
   -not [LuP06WinIdentity]::IsWindowVisible($hwnd) -or
   [LuP06WinIdentity]::GetAncestor($hwnd,2) -ne $hwnd){
  throw 'Unattested non-root or hidden native window'
}
$osPid=[uint32]0
$thread=[LuP06WinIdentity]::GetWindowThreadProcessId($hwnd,[ref]$osPid)
if($thread -eq 0 -or $osPid -ne $ExpectedPid){throw 'Window owning PID mismatch'}
$cloaked=[uint32]0
$hr=[LuP06WinIdentity]::DwmGetWindowAttribute($hwnd,14,[ref]$cloaked,4)
if($hr -ne 0 -or $cloaked -ne 0){throw 'Compositor identity unavailable or window cloaked'}
$proc=Get-Process -Id ([int]$ExpectedPid) -ErrorAction Stop
$auditor=Get-Process -Id $PID -ErrorAction Stop
if($proc.SessionId -ne $auditor.SessionId){
  throw 'Native accessibility source is outside reviewer Windows session'
}
if($proc.StartTime.ToUniversalTime().ToFileTimeUtc() -ne $ExpectedBirthFileTime){
  throw 'OS process instance changed'
}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$root=[System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
if($null -eq $root -or [uint32]$root.Current.ProcessId -ne $ExpectedPid){
  throw 'Independent UI Automation root identity mismatch'
}
# OS IsPassword flag is useful but INCOMPLETE. Do not inspect values/names.
$condition=[System.Windows.Automation.PropertyCondition]::new(
  [System.Windows.Automation.AutomationElement]::IsPasswordProperty,$true)
$matches=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition)
if($matches.Count -gt $MaxRegions){throw 'OS password region limit exceeded'}
$rects=@()
for($i=0;$i -lt $matches.Count;$i++){
  $item=$matches.Item($i)
  if([uint32]$item.Current.ProcessId -ne $ExpectedPid){
    throw 'Cross-process accessibility tree cannot be trusted'
  }
  $b=$item.Current.BoundingRectangle
  if($b.IsEmpty -or $b.Width -le 0 -or $b.Height -le 0 -or
     [double]::IsNaN($b.Left) -or [double]::IsNaN($b.Top) -or
     [double]::IsInfinity($b.Left) -or [double]::IsInfinity($b.Top)){
    throw 'Unusable OS sensitive-control rectangle'
  }
  $rects+=@{left=$b.Left;top=$b.Top;width=$b.Width;height=$b.Height}
}
$after=Get-Process -Id ([int]$ExpectedPid) -ErrorAction Stop
if($after.SessionId -ne $auditor.SessionId){
  throw 'Native accessibility session changed during review'
}
if($after.StartTime.ToUniversalTime().ToFileTimeUtc() -ne $ExpectedBirthFileTime){
  throw 'Source process changed while querying OS accessibility'
}
[pscustomobject]@{
  schema='Lu-P06-sensitive-uia-v1'
  sourceIdentity='ROOT_VISIBLE_PID_CREATION_OS_VERIFIED'
  count=$rects.Count
  passwordControlDesktopBounds=$rects
  detectorCoverage='INCOMPLETE_UIA_ONLY'
  wgcFrameCoordinatesVerified=$false
  pixelReleaseAllowed=$false
  guiVerificationPassed=$false
}|ConvertTo-Json -Depth 5 -Compress
