# Installed Windows acceptance — actual NSIS install, UI Automation, clipboard.
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
}
'@
$report = [ordered]@{ tested='installed-nsis-windows'; installed=$false; launched=$false;
  uia=$false; projectRegistered=$false; editorOpened=$false; edited=$false;
  clipboardVerified=$false; noImplicitSave=$false; completed=$false; failure=$null }
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
  $root=[System.Windows.Automation.AutomationElement]::RootElement
  $cond=[System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,$app.Id)
  return $root.FindFirst([System.Windows.Automation.TreeScope]::Children,$cond)
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
  ClickItem $open.Current.Name
  Start-Sleep -Seconds 2
  # Navigate the actual native folder picker, not a privileged Tauri IPC call.
  [System.Windows.Forms.Clipboard]::SetText($projectRoot)
  Keys '%d'
  Keys '^v'
  Keys '{ENTER}'
  Start-Sleep -Seconds 2
  Keys '{ENTER}'
  Start-Sleep -Seconds 3
  FocusApp
  if($null -eq (FindItem 'Edit' 18)) {throw 'Native picker failed to register project'}
  $report.projectRegistered=$true
  ClickItem 'Edit'
  Start-Sleep -Seconds 2
  ClickItem 'acceptance.txt'
  Start-Sleep -Seconds 2
  if($null -eq (FindItem 'Copy draft reference' 15)) {
    throw 'Installed editor does not expose Copy draft reference'
  }
  $report.editorOpened=$true
  $window=AppWindow
  $controls=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition)
  $editable=$null
  foreach($control in $controls) {
    if($control.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) {
      $editable=$control
      break
    }
  }
  if($null -eq $editable) {throw 'CodeMirror editable surface unavailable to UI Automation'}
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
  $report.noImplicitSave=(Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash -eq $original
  if(!$report.noImplicitSave) {throw 'Draft operation changed saved file without Save'}
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
