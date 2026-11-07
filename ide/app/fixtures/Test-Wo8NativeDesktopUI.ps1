# WO8-A/B native Windows UI Automation acceptance. Never executes project code.
# Run on an isolated disposable Windows runner *after* real NSIS installation.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExecutable,
  [Parameter(Mandatory=$true)][string]$ExpectedSha256,
  [ValidateSet('Shell','ProjectReadOnly')][string]$Scenario='Shell',
  [string]$FixtureRoot=''
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

function Deny([string]$code) { throw "WO8_NATIVE_UI_DENIED:$code" }
function BoundedProcess([string]$name) {
  return @(Get-Process -Name $name -ErrorAction SilentlyContinue).Count
}
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
  Deny 'disposable_native_runner_only'
}
$exe=(Resolve-Path -LiteralPath $InstalledExecutable).Path
if ($exe -notlike '*\\lu-ide-desktop.exe') { Deny 'unexpected_executable' }
if ($ExpectedSha256 -notmatch '^[a-fA-F0-9]{64}$' -or
    (Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash -ine $ExpectedSha256) {
  Deny 'installed_binary_digest_mismatch'
}
$before=@{}
foreach($name in @('lldb-dap','clangd','rust-analyzer','ruff','cargo','windows_recovery_service')) {
  $before[$name]=BoundedProcess $name
}
$root=$null
$proc=$null
$stage='startup'
function WindowRoot {
  if ($null -eq $proc) { return $null }
  $proc.Refresh()
  if ($proc.HasExited) { Deny 'desktop_exited_prematurely' }
  if ($proc.MainWindowHandle -eq [IntPtr]::Zero) { return $null }
  try { return [System.Windows.Automation.AutomationElement]::FromHandle($proc.MainWindowHandle) }
  catch { return $null }
}
function Find-Control([string]$query,[string]$kind,[int]$seconds=18) {
  $end=(Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $end) {
    $window=WindowRoot
    if ($window) {
      $all=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition)
      foreach($item in $all) {
        try {
          $name=$item.Current.Name
          $type=$item.Current.ControlType.ProgrammaticName
          if ($name -and $name.IndexOf($query,[StringComparison]::OrdinalIgnoreCase) -ge 0 -and
              (!$kind -or $type -eq ('ControlType.'+$kind))) {
            return $item
          }
        } catch {}
      }
    }
    Start-Sleep -Milliseconds 450
  }
  Deny ('ui_element_not_exposed_'+$query.Replace(' ','_'))
}
function Click([string]$label) {
  $element=Find-Control $label 'Button'
  if (!$element.Current.IsEnabled) { Deny ('button_disabled_'+$label.Replace(' ','_')) }
  try {
    $p=$element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $p.Invoke()
  } catch { Deny ('ui_invoke_failure_'+$label.Replace(' ','_')) }
}
function Require([string]$text) {
  [void](Find-Control $text '' 15)
}
try {
  $proc=Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
  $root=Find-Control 'Lu IDE' '' 30
  $stage='shell'
  Require 'Workspace projects'
  Require 'Not connected'
  Click 'Capabilities'
  Require 'IDE capabilities'
  Require 'Available host operations'
  Click 'Language services'
  Require 'Language server providers'
  Click 'Job requests'
  Require 'Execution requests'
  Click 'Projects'
  Require 'Workspace projects'
  $stage='picker'
  # Native folder chooser must be opened through UI, never by trusting input path.
  Click '+ Open folder'
  Start-Sleep -Seconds 2
  $proc.Refresh()
  if ($proc.HasExited) { Deny 'picker_killed_desktop' }
  if ($Scenario -eq 'Shell') {
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 800
    Require 'Workspace projects'
    Write-Host 'WO8_INSTALLED_NATIVE_UI_NAVIGATION_AND_PICKER=PASS'
  } else {
    if (!$FixtureRoot) { Deny 'missing_fixture_root' }
    $fixed=[IO.Path]::GetFullPath($FixtureRoot)
    $temporary=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\\')+'\\'
    if (!$fixed.StartsWith($temporary,[StringComparison]::OrdinalIgnoreCase) -or
        !(Test-Path -LiteralPath (Join-Path $fixed 'main.rs'))) {
      Deny 'non_disposable_or_missing_fixture'
    }
    # Standard Windows folder dialog address navigation; real desktop input.
    [System.Windows.Forms.SendKeys]::SendWait('^l')
    [System.Windows.Forms.Clipboard]::SetText($fixed)
    [System.Windows.Forms.SendKeys]::SendWait('^v')
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Seconds 2
    # The Windows directory chooser's confirmation label is OS-version dependent.
    $button=$null
    foreach ($name in @('Select Folder','Choose Folder','Open','Select')) {
      try {
        $desktop=[System.Windows.Automation.AutomationElement]::RootElement
        $buttons=$desktop.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button)))
        foreach($entry in $buttons) {
          if ($entry.Current.Name -eq $name -and $entry.Current.IsEnabled) { $button=$entry; break }
        }
        if ($button) { break }
      } catch {}
    }
    if (!$button) { Deny 'native_picker_confirmation_not_found' }
    $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Require 'Registered projects'
    Require (Split-Path $fixed -Leaf)
    Click 'Edit'
    Require 'Source explorer and editor'
    Click 'main.rs'
    Require 'Source code editor'
    Require 'main.rs'
    Write-Host 'WO8_INSTALLED_NATIVE_FOLDER_REGISTRATION_AND_SOURCE_OPEN=PASS'
  }
  foreach($name in $before.Keys) {
    if ((BoundedProcess $name) -gt $before[$name]) {
      Deny ('unauthorized_worker_autostart_'+$name)
    }
  }
  Write-Host ('WO8_INSTALLED_NATIVE_UI_SCENARIO='+$Scenario+':PASS')
  Write-Host 'WO8_INSTALLED_PROTECTED_RUN_TEST_DEBUG=NOT_ACCEPTED'
} finally {
  if ($proc) {
    try {
      $proc.Refresh()
      if (!$proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -ErrorAction Stop
        $proc.WaitForExit(10000) | Out-Null
      }
    } catch { Deny 'desktop_process_reap_unverified' }
  }
}
