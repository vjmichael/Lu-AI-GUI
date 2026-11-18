# B2 actual NSIS installed Windows editor, session and durable clipboard, restart recovery.
# Fails explicitly when GitHub runner has no interactive UI desktop.
param([Parameter(Mandatory)][string]$InstallerPath,
      [Parameter(Mandatory)][string]$EvidenceDirectory)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuMouse {
 [DllImport("user32.dll")]
 public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")]
 public static extern IntPtr GetWindow(IntPtr hwnd,uint command);
 [DllImport("user32.dll")]
 public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")]
 public static extern bool SetForegroundWindow(IntPtr hwnd);
}
'@
$report = [ordered]@{ tested='installed-nsis-windows'; installed=$false; launched=$false;
  uia=$false; projectRegistered=$false; editorOpened=$false; edited=$false;
  clipboardVerified=$false; durableClipboardVerified=$false; recoveredAfterRestart=$false; explicitSaveVerified=$false; noImplicitSave=$false; completed=$false; failure=$null }
$app = $null
function Screenshot([string]$name) {
  try {
    $rect=[System.Windows.Forms.SystemInformation]::VirtualScreen
    $image=[System.Drawing.Bitmap]::new($rect.Width,$rect.Height)
    $graphics=[System.Drawing.Graphics]::FromImage($image)
    $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,$image.Size)
    $image.Save((Join-Path $EvidenceDirectory "$name.png"))
    $graphics.Dispose();$image.Dispose()
  } catch {
    "$_" | Set-Content (Join-Path $EvidenceDirectory "$name-error.txt")
  }
}
function AppWindow {
  $app.Refresh()
  if($app.HasExited -or $app.MainWindowHandle -eq [IntPtr]::Zero) {return $null}
  try {return [System.Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)}
  catch {return $null}
}
function FindItem([string]$name,[int]$seconds=15) {
  $until=[DateTime]::UtcNow.AddSeconds($seconds)
  while([DateTime]::UtcNow -lt $until) {
    $window=AppWindow
    if($null -ne $window) {
      $condition=[System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::NameProperty,$name)
      $item=$window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
      if($null -ne $item) {return $item}
    }
    Start-Sleep -Milliseconds 400
  }
  return $null
}
function ClickItem([string]$name) {
  $item=FindItem $name
  if($null -eq $item) {throw "UIA control not visible: $name"}
  $pattern=$null
  if($item.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) {
    $pattern.Invoke()
  } else {
    $bounds=$item.Current.BoundingRectangle
    if($bounds.IsEmpty) {throw "UIA control has no interactive bounds: $name"}
    [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new(
      [int]($bounds.X+$bounds.Width/2),[int]($bounds.Y+$bounds.Height/2))
    [LuMouse]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    [LuMouse]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
  }
}
function Keys([string]$value) {
  [System.Windows.Forms.SendKeys]::SendWait($value)
  Start-Sleep -Milliseconds 500
}
function FocusApp {
  $shell=New-Object -ComObject WScript.Shell
  if(!$shell.AppActivate($app.Id)) {throw 'Installed app could not be focused'}
  Start-Sleep -Milliseconds 500
}
try {
  if(!(Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw 'Installer file was not produced'
  }
  $testRoot=Join-Path $env:RUNNER_TEMP 'LuIdeInstalledAcceptance'
  $installRoot=Join-Path $testRoot 'Installed'
  $projectRoot=Join-Path $testRoot 'Fixture'
  New-Item -ItemType Directory -Path $installRoot,$projectRoot -Force | Out-Null
  $crlf=[string][char]13+[string][char]10
  $fixture=Join-Path $projectRoot 'acceptance.txt'
  [System.IO.File]::WriteAllText($fixture,'alpha line'+$crlf+'second text'+$crlf,
    [System.Text.UTF8Encoding]::new($false))
  $original=(Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash

  # This is a real silent NSIS installation into a fresh user-owned directory.
  $install=Start-Process -FilePath $InstallerPath -ArgumentList @('/S',"/D=$installRoot") -PassThru -Wait
  if($install.ExitCode -ne 0) {throw "NSIS installation failed: $($install.ExitCode)"}
  $apps=@(Get-ChildItem -LiteralPath $installRoot -Filter '*.exe' -Recurse -File |
    Where-Object {$_.Name -notmatch '(unins|setup|install)'})
  if($apps.Count -ne 1) {throw "Installed application missing or ambiguous: $($apps.Count)"}
  $report.installed=$true
  $app=Start-Process -FilePath $apps[0].FullName -WorkingDirectory $installRoot -PassThru
  Start-Sleep -Seconds 7
  $app.Refresh()
  if($app.HasExited) {throw 'Installed application exited before interaction'}
  $report.launched=$true
  FocusApp
  Screenshot 'startup'
  $open=FindItem 'Choose a folder' 20
  if($null -eq $open) {$open=FindItem '+ Open folder' 8}
  if($null -eq $open) {throw 'Installed WebView controls unavailable to UI Automation'}
  $report.uia=$true
  # Pin the actual main HWND before opening the separate native picker.
  $mainHwnd=$app.MainWindowHandle
  if($mainHwnd -eq [IntPtr]::Zero) {throw 'Installed main window unavailable'}
  ClickItem $open.Current.Name
  $picker=$null
  $deadline=[DateTime]::UtcNow.AddSeconds(20)
  do {
    $desktop=[System.Windows.Automation.AutomationElement]::RootElement
    $windows=$desktop.FindAll([System.Windows.Automation.TreeScope]::Children,
      [System.Windows.Automation.Condition]::TrueCondition)
    $matching=@($windows | Where-Object {
      try {
        $h=[IntPtr]::new([int64]$_.Current.NativeWindowHandle)
        $h -ne [IntPtr]::Zero -and $h -ne $mainHwnd -and
          ($_.Current.ProcessId -eq $app.Id -or [LuMouse]::GetWindow($h,4) -eq $mainHwnd) -and
          $_.Current.Name -ieq 'Select folder' -and !$_.Current.IsOffscreen
      } catch { $false }
    })
    if($matching.Count -gt 1) {throw 'Ambiguous installed native folder picker'}
    if($matching.Count -eq 1) {$picker=$matching[0]; break}
    Start-Sleep -Milliseconds 300
  } while([DateTime]::UtcNow -lt $deadline)
  if($null -eq $picker) {throw 'Owned native folder picker unavailable'}
  $pickerHwnd=[IntPtr]::new([int64]$picker.Current.NativeWindowHandle)
  [void][LuMouse]::SetForegroundWindow($pickerHwnd)
  if([LuMouse]::GetForegroundWindow() -ne $pickerHwnd) {throw 'Native picker not foreground'}
  # Navigate the actual OS modal; never send keys if another window took focus.
  [System.Windows.Forms.Clipboard]::SetText($projectRoot)
  Keys '^l'
  if([LuMouse]::GetForegroundWindow() -ne $pickerHwnd) {throw 'Native picker focus changed'}
  Keys '^v'
  Keys '{ENTER}'
  Start-Sleep -Milliseconds 800
  $deadline=[DateTime]::UtcNow.AddSeconds(15)
  do {
    if([LuMouse]::GetForegroundWindow() -ne $pickerHwnd) {break}
    try {
      $condition=[System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::NameProperty,'Select Folder')
      $buttons=@($picker.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition) |
        Where-Object {$_.Current.IsEnabled -and !$_.Current.IsOffscreen -and
          $_.Current.ControlType.ProgrammaticName -in @('ControlType.Button','ControlType.Pane')})
      if($buttons.Count -eq 1) {
        $button=$buttons[0]
        try { $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
        catch {
          $rect=$button.Current.BoundingRectangle
          $owner=$picker.Current.BoundingRectangle
          if($rect.Width -lt 8 -or $rect.Height -lt 8) {throw 'Native confirmation bounds missing'}
          $x=[int]($rect.Left+$rect.Width/2); $y=[int]($rect.Top+$rect.Height/2)
          if($x -le $owner.Left -or $x -ge $owner.Right -or $y -le $owner.Top -or $y -ge $owner.Bottom) {throw 'Native confirmation outside picker'}
          [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new($x,$y)
          [LuMouse]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
          [LuMouse]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
        }
        break
      }
    } catch {throw}
    Start-Sleep -Milliseconds 300
  } while([DateTime]::UtcNow -lt $deadline)
  Start-Sleep -Seconds 1
  if([LuMouse]::GetForegroundWindow() -eq $pickerHwnd) {throw 'Native folder picker remained open'}
  FocusApp
  if($null -eq (FindItem 'Edit project Fixture' 18)) {throw 'Native picker failed to register project'}
  $report.projectRegistered=$true
  ClickItem 'Edit project Fixture'
  Start-Sleep -Seconds 2
  ClickItem 'acceptance.txt'
  Start-Sleep -Seconds 2
  if($null -eq (FindItem 'Copy draft reference' 15)) {
    throw 'Installed editor does not expose Copy draft reference'
  }
  $report.editorOpened=$true
  # Exact current CodeMirror accessible identity, not an arbitrary input or
  # the WebView document. Preserve the real native editing witness.
  $editable=FindItem 'Source editor text · First editor: acceptance.txt' 15
  if($null -eq $editable -or !$editable.Current.IsKeyboardFocusable) {
    throw 'CodeMirror editable surface unavailable to UI Automation'
  }
  $bounds=$editable.Current.BoundingRectangle
  if($bounds.IsEmpty) {throw 'CodeMirror interactive bounds empty'}
  [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new(
    [int]($bounds.X+$bounds.Width/2),[int]($bounds.Y+$bounds.Height/2))
  [LuMouse]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [LuMouse]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
  Keys '^{END}'
  Keys 'x'
  $report.edited=$true
  [System.Windows.Forms.Clipboard]::Clear()
  ClickItem 'Copy draft reference'
  $clipboard=''
  for($i=0;$i -lt 30;$i++) {
    Start-Sleep -Milliseconds 400
    if([System.Windows.Forms.Clipboard]::ContainsText()) {
      $clipboard=[System.Windows.Forms.Clipboard]::GetText()
      if($clipboard.StartsWith('ide-buffer:v1')) {break}
    }
  }
  if($clipboard -notmatch '^ide-buffer:v1' -or
     $clipboard -notmatch 'revision=[1-9][0-9]*' -or
     $clipboard -notmatch 'buffer=[a-zA-Z0-9-]+' -or
     $clipboard -notmatch 'baseDisk=[a-f0-9]{64}' -or
     $clipboard -notmatch 'draftSha256=[a-f0-9]{64}' -or
     $clipboard -notmatch 'lifetime=session-only') {
    throw 'Installed GUI did not produce a complete changed-draft reference'
  }
  if($clipboard.Contains('alpha line') -or $clipboard.Contains('second text')) {
    throw 'Copy Draft Reference disclosed source text instead of identity metadata'
  }
  $report.clipboardVerified=$true
  # The installed desktop must persist the exact unsaved bytes in its
  # private Tauri app-data vault, not merely create a volatile UUID.
  [System.Windows.Forms.Clipboard]::Clear()
  ClickItem 'Preserve draft + copy durable reference'
  $durable=''
  for($i=0;$i -lt 40;$i++) {
    Start-Sleep -Milliseconds 400
    if([System.Windows.Forms.Clipboard]::ContainsText()) {
      $durable=[System.Windows.Forms.Clipboard]::GetText()
      if($durable.StartsWith('ide-draft:v1')) {break}
    }
  }
  if($durable -notmatch '^ide-draft:v1' -or
     $durable -notmatch 'checkpoint=[a-f0-9]{64}' -or
     $durable -notmatch 'draftSha256=[a-f0-9]{64}' -or
     $durable -notmatch 'lifetime=persistent-private-app-data' -or
     $durable.Contains('alpha line') -or $durable.Contains('second text')) {
    throw 'Installed UI did not copy an immutable non-disclosing durable private draft'
  }
  $report.durableClipboardVerified=$true
  $report.noImplicitSave=(Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash -eq $original
  if(!$report.noImplicitSave) {throw 'Draft operation changed saved file without Save'}
  # Restart the ACTUAL installed process. Private draft must remain intact
  # with the new editor session ID; none of the source bytes may change.
  if(!$app.HasExited) {Stop-Process -Id $app.Id -Force}
  $app=Start-Process -FilePath $apps[0].FullName -WorkingDirectory $installRoot -PassThru
  Start-Sleep -Seconds 7
  $app.Refresh()
  if($app.HasExited) {throw 'Installed app failed to reopen after durable draft checkpoint'}
  FocusApp
  $reopen=FindItem 'Reopen project Fixture' 12
  if($null -ne $reopen) {ClickItem 'Reopen project Fixture'; Start-Sleep -Seconds 2}
  if($null -eq (FindItem 'Edit project Fixture' 15)) {throw 'Persisted project not available for reopening'}
  ClickItem 'Edit project Fixture'
  Start-Sleep -Seconds 2
  ClickItem 'acceptance.txt'
  if($null -eq (FindItem 'Recover preserved draft' 18)) {
    throw 'No native private recovery surfaced after installed app restart'
  }
  ClickItem 'Recover preserved draft'
  if($null -eq (FindItem 'Recovered private checkpoint to editor only' 15)) {
    throw 'Private draft was not explicitly recovered into installed editor'
  }
  $report.recoveredAfterRestart=$true
  if((Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash -ne $original) {
    throw 'Recovery changed project source without Save'
  }
  # Explicit human Save after recovery is the ONLY allowed project file change.
  ClickItem 'Save · Ctrl+S'
  $changed=$false
  for($i=0;$i -lt 40;$i++) {
    Start-Sleep -Milliseconds 350
    if((Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash -ne $original) {
      $changed=$true
      break
    }
  }
  if(!$changed) {throw 'Native editor did not save recovered draft after explicit Save'}
  $report.explicitSaveVerified=$true
  $report.completed=$true
  Screenshot 'draft-reference-success'
} catch {
  $report.failure=($_ | Out-String).Trim()
  Screenshot 'failure'
  throw
} finally {
  $report | ConvertTo-Json -Depth 5 |
    Set-Content -Encoding UTF8 (Join-Path $EvidenceDirectory 'installed-acceptance.json')
  if($null -ne $app) {
    try {if(!$app.HasExited) {Stop-Process -Id $app.Id -Force}} catch {}
  }
}
