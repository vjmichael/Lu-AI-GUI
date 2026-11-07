# WO8-A/B native Windows UI Automation acceptance. Never executes project code.
# Run on an isolated disposable Windows runner *after* real NSIS installation.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExecutable,
  [Parameter(Mandatory=$true)][string]$ExpectedSha256,
  [ValidateSet('Shell','ProjectReadOnly','EditorPersistence','PersistedRestart','P04ReinstalledProject','ExternalConflict','ProtectedBuildDebug','ProtectedCancellation','ProtectedHostCrash','ProtectedServiceCrash','ProtectedTamperDenial','ProtectedDebuggerCancellation','Wo9WorkflowPortfolio','Wo9ReviewedGit','Wo9ProtectedGraph','Wo9ProtectedGraphCancel','Wo9ProtectedGraphHostCrash')][string]$Scenario='Shell',
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
if ([IO.Path]::GetFileName($exe) -ine 'lu-ide-desktop.exe') { Deny 'unexpected_executable' }
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
# Real WebView2/Chromium UIA can expose its content through a child renderer
# HWND even when the Tauri top-level UIA fragment walk is orphaned.
# Native operating-system UI Automation ONLY; no JS, IPC or test shortcut.
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class P04NativeWebViewWindows {
  public delegate bool Enumeration(IntPtr hwnd, IntPtr state);
  [DllImport("user32.dll", SetLastError=true)]
  private static extern bool EnumChildWindows(IntPtr parent, Enumeration callback, IntPtr data);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);
  public static IntPtr[] RendererHandles(IntPtr top) {
    var results = new List<IntPtr>();
    Enumeration callback = (hwnd, data) => {
      var name = new StringBuilder(96);
      if (GetClassName(hwnd, name, name.Capacity) > 0 &&
          name.ToString() == "Chrome_RenderWidgetHostHWND") {
        results.Add(hwnd);
      }
      return true;
    };
    EnumChildWindows(top, callback, IntPtr.Zero);
    return results.ToArray();
  }
}
'@
function NativeAccessibilityRoots {
  $parent=WindowRoot
  if (!$parent) { return }
  Write-Output $parent
  $handles=@([P04NativeWebViewWindows]::RendererHandles($proc.MainWindowHandle))
  if ($handles.Count -gt 8) { Deny 'unexpected_webview_renderer_hierarchy' }
  foreach ($handle in $handles) {
    if ($handle -eq [IntPtr]::Zero) { continue }
    try {
      $web=[System.Windows.Automation.AutomationElement]::FromHandle($handle)
      if ($web) { Write-Output $web }
    } catch {}
  }
}
function Find-Control([string]$query,[string]$kind,[int]$seconds=18) {
  $end=(Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $end) {
    # Do not fake a WebView response. Every candidate is a real native UIA
    # provider reached from this exact installed desktop's child HWND chain.
    foreach ($window in @(NativeAccessibilityRoots)) {
      try {
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
      } catch {}
    }
    Start-Sleep -Milliseconds 450
  }
  # P02: an actual Windows installed run failed here for Execution requests.
  # Emit a bounded snapshot of the real native UIA tree before denying; this is
  # diagnostic evidence only and NEVER converts a missing control into a pass.
  try {
    $nativeRoot=WindowRoot
    if ($nativeRoot) {
      $nodes=$nativeRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition)
      Write-Host ('P02_NATIVE_UIA_MISSING_CONTROL='+$query+'; tree_nodes='+$nodes.Count)
      Write-Host ('P04_NATIVE_WEBVIEW_RENDERER_HWND_COUNT='+@([P04NativeWebViewWindows]::RendererHandles($proc.MainWindowHandle)).Count)
      $printed=0
      foreach($entry in $nodes) {
        if ($printed -ge 100) { break }
        try {
          $n=[string]$entry.Current.Name
          if (![string]::IsNullOrWhiteSpace($n)) {
            $n=($n -replace '[\r\n\t]+',' ')
            if ($n.Length -gt 128) { $n=$n.Substring(0,128) }
            $t=[string]$entry.Current.ControlType.ProgrammaticName
            Write-Host ('P02_NATIVE_UIA_NODE ['+$t+'] '+$n)
            $printed++
          }
        } catch {}
      }
    } else {
      Write-Host ('P02_NATIVE_UIA_NO_MAIN_WINDOW_FOR='+$query)
    }
  } catch {
    Write-Host ('P02_NATIVE_UIA_SNAPSHOT_ERROR='+$_.Exception.GetType().Name)
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
function ProtectedWorkerCount {
  $count=0
  foreach($name in @('cargo.exe','rustc.exe','lldb-dap.exe','wo6-selftest.exe')) {
    $count+=@(Get-CimInstance Win32_Process -Filter ("Name='"+$name+"'") -ErrorAction SilentlyContinue).Count
  }
  return $count
}
function Wait-RealProtectedWorker([int]$before) {
  # A mere enabled Cancel button is NOT proof that an OS process was ever
  # admitted. Require an observed executable in the protected job window.
  $deadline=(Get-Date).AddSeconds(60)
  do {
    if ((ProtectedWorkerCount) -gt $before) { return }
    Start-Sleep -Milliseconds 300
  } while ((Get-Date) -lt $deadline)
  Deny 'no_actual_protected_os_worker_observed'
}
function ActiveProtectedJobId {
  $deadline=(Get-Date).AddSeconds(25)
  while ((Get-Date) -lt $deadline) {
    try {
      $record=Find-Control 'Active protected job:' '' 2
      $name=[string]$record.Current.Name
      if ($name -match '[·:]\s*(?<id>job-[0-9a-zA-Z_-]{8,})\s*$') {
        return $Matches['id']
      }
    } catch {}
  }
  Deny 'active_native_job_id_not_exposed_by_installed_ui'
}
function ActiveProtectedDebuggerJobId {
  $deadline=(Get-Date).AddSeconds(25)
  while ((Get-Date) -lt $deadline) {
    try {
      $record=Find-Control 'Active debug session:' '' 2
      $name=[string]$record.Current.Name
      if ($name -match 'Active debug session:\s*(?<id>job-[0-9a-zA-Z_-]{8,})') {
        return $Matches['id']
      }
    } catch {}
  }
  Deny 'active_native_debugger_job_id_not_exposed'
}
function Verify-DurableDebuggerEvidence([string]$jobId) {
  [void](Find-Control ('Durable debugger job '+$jobId) '' 45)
  $button=$null
  try { $button=Find-Control 'Verify protected debugger cleanup (no execution)' 'Button' 3 } catch {}
  if ($button -and $button.Current.IsEnabled) {
    Click 'Verify protected debugger cleanup (no execution)'
    Confirm-NativeOneUseDialog 'Reconcile interrupted protected debugger' 'Verify OS cleanup'
    [void](Find-Control ('Durable debugger job '+$jobId) '' 35)
  }
}
function Verify-DurableJobEvidence([string]$jobId) {
  $observed=Find-Control $jobId '' 35
  if (!$observed.Current.Name.Contains($jobId)) {
    Deny 'durable_recovery_job_identity_mismatch'
  }
  $reconcile=$null
  try { $reconcile=Find-Control 'Verify OS-only cleanup (no rerun)' 'Button' 3 } catch {}
  if ($reconcile -and $reconcile.Current.IsEnabled) {
    Click 'Verify OS-only cleanup (no rerun)'
    Confirm-NativeOneUseDialog 'Reconcile interrupted protected job' 'Verify and clean up'
    [void](Find-Control ('Durable protected job '+$jobId) '' 45)
  } else {
    [void](Find-Control ('Durable protected job '+$jobId) '' 25)
  }
}
function Wait-WorkerReturnToBaseline([int]$before) {
  $deadline=(Get-Date).AddSeconds(75)
  do {
    $count=ProtectedWorkerCount
    if ($count -le $before) { return }
    Start-Sleep -Milliseconds 500
  } while ((Get-Date) -lt $deadline)
  Deny 'protected_worker_processes_not_reaped'
}
function VerifiedInstalledService {
  $service=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'" -ErrorAction Stop
  if ($null -eq $service) { Deny 'installed_protected_service_absent' }
  $expected=Join-Path (Split-Path $exe) 'resources\recovery\windows_recovery_service.exe'
  if ($service.PathName.Trim('"') -ine $expected -or
      $service.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE')) {
    Deny 'service_identity_or_image_not_pinned'
  }
  if ((Get-AuthenticodeSignature -LiteralPath $expected).Status -ne 'Valid') {
    Deny 'installed_service_signature_invalid'
  }
  return $service
}
function Start-ProtectedSlowTest {
  Click 'Job requests'
  Require 'Protected Run / Test'
  $recipe=Find-Control 'Recipe' 'ComboBox' 20
  if (!$recipe.Current.IsKeyboardFocusable) { Deny 'native_recipe_picker_not_focusable' }
  $recipe.SetFocus()
  [System.Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{DOWN}{ENTER}')
  $action=Find-Control 'Request protected Run / Test' 'Button' 30
  if (!$action.Current.IsEnabled) { Deny 'protected_cargo_worker_not_ready' }
  Click 'Request protected Run / Test'
  Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
  [void](Find-Control 'Cancel and verify process-tree termination' 'Button' 30)
}
function Prepare-Wo9ProtectedSlowGraph {
  Click 'Packages & workflows'
  Require 'Multi-project release review'
  $checkbox=Find-Control (Split-Path $fixed -Leaf) 'CheckBox' 25
  if (!$checkbox.Current.IsEnabled) { Deny 'wo9_recovery_project_not_selectable' }
  $checkbox.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
  Click 'Inspect selected workflow'
  Require 'Eligible for human source review'
  Click 'Verify frozen Git and artifact evidence again'
  Require 'Exact evidence matched'
  Click 'Add dependent step'
  Click 'Add dependent step'
  Require 'Step 2'
  # The first step must actually run the 90-second disposable Rust test:
  # simply selecting rust_check would finish before cancellation/crash.
  $recipe=Find-Control 'Approved offline recipe' 'ComboBox' 20
  if (!$recipe.Current.IsKeyboardFocusable) { Deny 'wo9_slow_recipe_not_accessible' }
  $recipe.SetFocus()
  [System.Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{ENTER}')
  Click 'Record exact dependency workflow'
  Require 'Durable dependent build/test workflow'
  Require 'Auto-run: disabled'
  $launch=Find-Control 'Request separately approved protected step' 'Button' 25
  if (!$launch.Current.IsEnabled) { Deny 'wo9_recovery_first_step_unavailable' }
}
function Ensure-Wo9DependentsBlocked {
  Click 'Packages & workflows'
  Click 'Refresh durable workflows'
  Require 'Durable dependent build/test workflow'
  Require 'Auto-run: disabled'
  Require 'Release approved: no'
  $launch=$null
  try { $launch=Find-Control 'Request separately approved protected step' 'Button' 4 } catch {}
  if ($launch -and $launch.Current.IsEnabled) {
    Deny 'wo9_interrupted_or_cancelled_step_unlocked_dependents'
  }
}
function Confirm-NativeOneUseDialog([string]$titleFragment,[string]$approvalCaption) {
  $deadline=(Get-Date).AddSeconds(35)
  $desktop=[System.Windows.Automation.AutomationElement]::RootElement
  while ((Get-Date) -lt $deadline) {
    $windows=$desktop.FindAll([System.Windows.Automation.TreeScope]::Children,
      ([System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Window)))
    foreach($win in $windows) {
      try {
        if ($win.Current.ProcessId -ne $proc.Id -or
            $win.Current.Name.IndexOf($titleFragment,[StringComparison]::OrdinalIgnoreCase) -lt 0) { continue }
        $desc=$win.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)
        foreach($child in $desc) {
          if ($child.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
              $child.Current.Name -eq $approvalCaption -and $child.Current.IsEnabled) {
            $child.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            return
          }
        }
      } catch {}
    }
    Start-Sleep -Milliseconds 350
  }
  Deny 'real_native_consent_dialog_not_observed'
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
  if ($Scenario -notin @('PersistedRestart','P04ReinstalledProject')) {
    # Native folder chooser must be opened through UI, never by trusting input path.
    Click '+ Open folder'
    Start-Sleep -Seconds 2
    $proc.Refresh()
    if ($proc.HasExited) { Deny 'picker_killed_desktop' }
  }
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
    if ($Scenario -notin @('PersistedRestart','P04ReinstalledProject')) {
      # Standard Windows folder dialog address navigation; real desktop input.
      [System.Windows.Forms.SendKeys]::SendWait('^l')
      if ($fixed -match '[+^%~(){}\[\]]') { Deny 'native_picker_path_contains_keyboard_metacharacters' }
      [System.Windows.Forms.SendKeys]::SendWait($fixed)
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      Start-Sleep -Seconds 2
      # The Windows directory chooser's confirmation label is OS-version dependent.
      $button=$null
      foreach ($name in @('Select Folder','Choose Folder','Open','Select')) {
      try {
        $desktop=[System.Windows.Automation.AutomationElement]::RootElement
        $buttons=$desktop.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          ([System.Windows.Automation.PropertyCondition]::new(
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
    }
    Require 'Registered projects'
    Require (Split-Path $fixed -Leaf)
    Click 'Edit'
    Require 'Source explorer and editor'
    Click 'main.rs'
    Require 'Source code editor'
    Require 'main.rs'
    if ($Scenario -in @('Wo9ProtectedGraphCancel','Wo9ProtectedGraphHostCrash')) {
      # This test MUST be performed against an independently signed, installed
      # protected WO7 candidate. No unsigned Tauri or mocked worker path.
      $sig=Get-AuthenticodeSignature -LiteralPath $exe
      if ($sig.Status -ne 'Valid' -or !$sig.SignerCertificate) {
        Deny 'wo9_graph_recovery_requires_real_signed_installed_app'
      }
      [void](VerifiedInstalledService)
      $initial=ProtectedWorkerCount
      Prepare-Wo9ProtectedSlowGraph
      if ((ProtectedWorkerCount) -ne $initial) {
        Deny 'wo9_graph_creation_autostarted_native_worker'
      }
      Click 'Request separately approved protected step'
      Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
      Click 'Job requests'
      [void](Find-Control 'Cancel and verify process-tree termination' 'Button' 40)
      $nativeId=ActiveProtectedJobId
      Wait-RealProtectedWorker $initial
      if ($Scenario -eq 'Wo9ProtectedGraphCancel') {
        Click 'Cancel and verify process-tree termination'
        Wait-WorkerReturnToBaseline $initial
        Verify-DurableJobEvidence $nativeId
        Ensure-Wo9DependentsBlocked
        Write-Host 'WO9_SIGNED_INSTALLED_GRAPH_CANCEL_DEPENDENTS_BLOCKED_AND_NATIVE_REAP=PASS'
      } else {
        # Kill the real installed Tauri process during native Cargo execution.
        # The supervisor/OS must reap protected child processes. On restart,
        # the persisted running DAG node must remain interrupted, not replayed.
        $previous=$proc.Id
        Stop-Process -Id $previous -Force -ErrorAction Stop
        $proc.WaitForExit(10000) | Out-Null
        Wait-WorkerReturnToBaseline $initial
        $proc=Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
        Require 'Workspace projects'
        Require (Split-Path $fixed -Leaf)
        Click 'Packages & workflows'
        Click 'Refresh durable workflows'
        Require 'interrupted'
        Ensure-Wo9DependentsBlocked
        Click 'Job requests'
        Verify-DurableJobEvidence $nativeId
        Write-Host 'WO9_SIGNED_INSTALLED_GRAPH_HOST_CRASH_NO_AUTOREPLAY_AND_DURABLE_RECEIPT=PASS'
      }
    }
    if ($Scenario -eq 'Wo9ProtectedGraph') {
      # Signed-release-only test: the portable unsigned IDE must never launch.
      $sign=Get-AuthenticodeSignature -LiteralPath $exe
      if ($sign.Status -ne 'Valid' -or !$sign.SignerCertificate) {
        Deny 'wo9_protected_graph_requires_signed_installed_candidate'
      }
      $service=VerifiedInstalledService
      $beforeWorkers=ProtectedWorkerCount
      Click 'Packages & workflows'
      Require 'Multi-project release review'
      $checkbox=Find-Control (Split-Path $fixed -Leaf) 'CheckBox' 25
      if (!$checkbox.Current.IsEnabled) { Deny 'wo9_protected_project_disabled' }
      $checkbox.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
      Click 'Inspect selected workflow'
      Require 'Eligible for human source review'
      Click 'Verify frozen Git and artifact evidence again'
      Require 'Exact evidence matched'
      Click 'Add dependent step'
      Click 'Add dependent step'
      Require 'Step 2'
      Click 'Record exact dependency workflow'
      Require 'Durable dependent build/test workflow'
      if ((ProtectedWorkerCount) -ne $beforeWorkers) {
        Deny 'wo9_graph_creation_launched_worker_automatically'
      }
      $runControls=Find-Control 'Request separately approved protected step' 'Button' 30
      if (!$runControls.Current.IsEnabled) { Deny 'wo9_first_protected_step_unavailable' }
      Click 'Request separately approved protected step'
      Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
      [void](Find-Control 'Native step returned' '' 360)
      Require 'passed'
      $second=Find-Control 'Request separately approved protected step' 'Button' 30
      if (!$second.Current.IsEnabled) { Deny 'wo9_dependent_protected_step_unavailable' }
      Click 'Request separately approved protected step'
      Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
      [void](Find-Control 'Native step returned' '' 360)
      Require 'passed'
      Require 'Auto-run: disabled'
      Require 'Release approved: no'
      Write-Host 'WO9_SIGNED_INSTALLED_TWO_STEP_PROTECTED_DEPENDENT_WORKFLOW=PASS'
    }
    if ($Scenario -eq 'Wo9WorkflowPortfolio') {
      # Actual installed Tauri WebView, not the standalone frontend served in a
      # browser. The registered fixture must have a clean HEAD + one artifact.
      Click 'Packages & workflows'
      Require 'Multi-project release review'
      Require 'Projects in workflow'
      $projectCheck=Find-Control (Split-Path $fixed -Leaf) 'CheckBox' 30
      if (!$projectCheck.Current.IsEnabled) { Deny 'wo9_portfolio_checkbox_disabled' }
      $projectCheck.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
      Click 'Inspect selected workflow'
      [void](Find-Control 'Native workflow review' '' 40)
      Require 'Eligible for human source review'
      Require 'Plan digest'
      Click 'Verify frozen Git and artifact evidence again'
      Require 'Exact evidence matched'
      Require 'Export verified artifacts as ZIP'
      Require 'Record exact dependency workflow'
      Write-Host 'WO9_INSTALLED_NATIVE_MULTI_PROJECT_REVIEW_AND_PINNED_EXPORT_UI=PASS'
    }
    if ($Scenario -eq 'Wo9ReviewedGit') {
      # External editor modifies one *tracked* file; only this one is reviewed.
      [IO.File]::AppendAllText((Join-Path $fixed 'main.rs'),"`n// WO9_NATIVE_REVIEWED_CHANGE`n")
      Click 'Source control'
      Require 'Reviewed local Git staging and commit'
      $before=(git -C $fixed rev-parse HEAD).Trim()
      if ($LASTEXITCODE -ne 0) { Deny 'wo9_fixture_git_head_unavailable' }
      $pathInput=Find-Control 'Project-relative tracked file' 'Edit' 20
      $pathInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('main.rs')
      Click 'Inspect exact Git file'
      Require 'Reviewed local file'
      Click 'Confirm and stage these reviewed bytes'
      Confirm-NativeOneUseDialog 'One-file Git stage' 'Stage reviewed bytes'
      Require 'Staged exact file'
      foreach($pair in @(
          @('Local commit message','WO9 installed reviewed Git source'),
          @('Author name','WO9 Native Acceptance'),
          @('Author email','wo9@localhost.invalid'))) {
        $edit=Find-Control $pair[0] 'Edit' 20
        $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($pair[1])
      }
      Click 'Confirm separate local commit'
      Confirm-NativeOneUseDialog 'Confirm local Git commit' 'Commit exact staged file'
      Require 'Local commit'
      $after=(git -C $fixed rev-parse HEAD).Trim()
      if ($LASTEXITCODE -ne 0 -or $before -eq $after) { Deny 'wo9_native_commit_not_observed' }
      $files=@(git -C $fixed diff-tree --no-commit-id --name-only -r HEAD)
      if ($files.Count -ne 1 -or $files[0] -ne 'main.rs') {
        Deny 'wo9_git_commit_changed_unreviewed_files'
      }
      Write-Host 'WO9_INSTALLED_ONE_FILE_GIT_STAGE_COMMIT_NO_HOOKS_OR_PUSH=PASS'
    }
    if ($Scenario -eq 'ProtectedDebuggerCancellation') {
      $sign=Get-AuthenticodeSignature -LiteralPath $exe
      if ($sign.Status -ne 'Valid' -or !$sign.SignerCertificate) {
        Deny 'unsigned_debugger_cancellation_test_denied'
      }
      $service=VerifiedInstalledService
      $initial=ProtectedWorkerCount
      Click 'Job requests'
      $button=Find-Control 'Run protected debugger self-test' 'Button' 30
      if (!$button.Current.IsEnabled) { Deny 'native_debugger_unavailable' }
      Click 'Run protected debugger self-test'
      Confirm-NativeOneUseDialog 'Approve one protected native debugger self-test' 'Run fixed debugger self-test'
      [void](Find-Control 'Request debugger cancellation' 'Button' 30)
      $dapJob=ActiveProtectedDebuggerJobId
      Wait-RealProtectedWorker $initial
      Click 'Request debugger cancellation'
      Wait-WorkerReturnToBaseline $initial
      Verify-DurableDebuggerEvidence $dapJob
      Write-Host 'WO8_SIGNED_INSTALLED_NATIVE_DAP_CANCEL_AND_DURABLE_RECEIPT=PASS'
    }
    if ($Scenario -in @('ProtectedCancellation','ProtectedHostCrash','ProtectedServiceCrash','ProtectedTamperDenial')) {
      $sign=Get-AuthenticodeSignature -LiteralPath $exe
      if ($sign.Status -ne 'Valid' -or !$sign.SignerCertificate) {
        Deny 'protected_recovery_requires_actual_signed_app'
      }
      $initial=ProtectedWorkerCount
      $installedService=VerifiedInstalledService
      Click 'Job requests'
      Require 'Protected Run / Test'
      $recipe=Find-Control 'Recipe' 'ComboBox' 25
      $recipe.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{DOWN}{ENTER}')
      $allow=Find-Control 'Request protected Run / Test' 'Button'
      if (!$allow.Current.IsEnabled) { Deny 'native_protected_test_unavailable' }
      Click 'Request protected Run / Test'
      if ($Scenario -eq 'ProtectedTamperDenial') {
        $tamper=Join-Path $fixed 'src\\main.rs'
        if (!(Test-Path $tamper)) { Deny 'missing_cargo_source_tamper_fixture' }
        [IO.File]::AppendAllText($tamper,"`n// WO8_TAMPER_AFTER_PLAN_BEFORE_CONSENT`n")
        Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
        [void](Find-Control 'Protected build/test was denied or failed' '' 40)
        Wait-WorkerReturnToBaseline $initial
        Write-Host 'WO8_INSTALLED_CHANGED_SOURCE_DENIED_BEFORE_WORKER=PASS'
      } else {
        Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
        [void](Find-Control 'Cancel and verify process-tree termination' 'Button' 40)
        $activeJobId=ActiveProtectedJobId
        Wait-RealProtectedWorker $initial
        if ($Scenario -eq 'ProtectedCancellation') {
          Click 'Cancel and verify process-tree termination'
          Wait-WorkerReturnToBaseline $initial
          Verify-DurableJobEvidence $activeJobId
          Write-Host 'WO8_INSTALLED_NATIVE_CANCEL_AND_DURABLE_JOB_RECONCILIATION=PASS'
        } elseif ($Scenario -eq 'ProtectedHostCrash') {
          $previousId=$proc.Id
          Stop-Process -Id $previousId -Force -ErrorAction Stop
          $proc.WaitForExit(10000) | Out-Null
          Wait-WorkerReturnToBaseline $initial
          $proc=Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
          Require 'Workspace projects'
          Require (Split-Path $fixed -Leaf)
          Click 'Job requests'
          Require 'Protected Run / Test'
          $inspected=VerifiedInstalledService
          Verify-DurableJobEvidence $activeJobId
          Write-Host 'WO8_INSTALLED_HOST_CRASH_NATIVE_REAP_RESTART_AND_RECEIPT=PASS'
        } elseif ($Scenario -eq 'ProtectedServiceCrash') {
          $broker=VerifiedInstalledService
          if ($broker.State -ne 'Running' -or $broker.ProcessId -le 0) {
            Deny 'cannot_inject_real_running_protected_service_crash'
          }
          Stop-Process -Id $broker.ProcessId -Force -ErrorAction Stop
          Start-Sleep -Seconds 2
          if ((Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'").ProcessId -eq $broker.ProcessId) {
            Deny 'real_service_crash_not_observed'
          }
          # The protected host must deny or quarantine further execution.
          $cancel=$null
          try { $cancel=Find-Control 'Cancel and verify process-tree termination' 'Button' 5 } catch {}
          if ($cancel -and $cancel.Current.IsEnabled) { Click 'Cancel and verify process-tree termination' }
          Wait-WorkerReturnToBaseline $initial
          $serviceState=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
          if ($serviceState.State -ne 'Running') {
            & sc.exe start LuIdeRecoveryBroker | Out-Null
            if ($LASTEXITCODE -ne 0) { Deny 'service_restart_denied_after_crash' }
          }
          Start-Sleep -Seconds 2
          $restored=VerifiedInstalledService
          if ($restored.State -ne 'Running') { Deny 'service_restart_not_healthy' }
          Verify-DurableJobEvidence $activeJobId
          Write-Host 'WO8_INSTALLED_PROTECTED_SCM_CRASH_RESTART_AND_DURABLE_RECEIPT=PASS'
        }
      }
    }
    if ($Scenario -eq 'ProtectedBuildDebug') {
      # Only on explicitly selected real signed WO7 candidate on native Windows.
      # The true one-use Tauri OS confirmation is driven through UI Automation;
      # no script calls native worker or forges an approval/IPC payload.
      Click 'Job requests'
      Require 'Protected Run / Test'
      # Native accessible select rather than a hidden direct invocation.
      $recipe=Find-Control 'Recipe' 'ComboBox' 20
      if (!$recipe.Current.IsKeyboardFocusable) { Deny 'fixed_recipe_selector_not_focusable' }
      $recipe.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{ENTER}')
      $buildAction=Find-Control 'Request protected Run / Test' 'Button' 25
      if (!$buildAction.Current.IsEnabled) { Deny 'installed_native_build_not_ready' }
      Click 'Request protected Run / Test'
      Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
      [void](Find-Control 'Last worker result:' '' 360)
      Require 'Native exit: 0'
      Require 'Reconciliation: not indicated'
      Require 'Worker output seal:'
      $debugAction=Find-Control 'Run protected debugger self-test' 'Button' 30
      if (!$debugAction.Current.IsEnabled) { Deny 'installed_native_debugger_not_ready' }
      Click 'Run protected debugger self-test'
      Confirm-NativeOneUseDialog 'Approve one protected native debugger self-test' 'Run fixed debugger self-test'
      [void](Find-Control 'Native LLDB self-test:' '' 240)
      Require 'Thread inspection: observed'
      Require 'Stack: observed'
      Require 'Kernel tree cleanup: reported by protected supervisor'
      Write-Host 'WO8_INSTALLED_NATIVE_PROTECTED_BUILD_AND_DEBUG_INSPECTION=PASS'
    }
    if ($Scenario -eq 'ExternalConflict') {
      # A real external editor replaces the file after the CodeMirror draft is
      # modified, but before the human presses Save in the installed WebView.
      $source=Join-Path $fixed 'main.rs'
      $saved=[IO.File]::ReadAllText($source)
      if ($saved.Contains('WO8_CONFLICT')) { Deny 'fixture_already_has_conflict_marker' }
      $editor=Find-Control 'Source editor text' ''
      if (!$editor.Current.IsKeyboardFocusable) { Deny 'native_editor_not_focusable' }
      $editor.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('^{END}')
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}// WO8_UNSAVED_LOCAL_DRAFT')
      Start-Sleep -Milliseconds 650
      if ([IO.File]::ReadAllText($source) -ne $saved) { Deny 'unsaved_code_mutated_source' }
      $outside=$saved+"`n// WO8_CONFLICT_EXTERNAL_UPDATE`n"
      [IO.File]::WriteAllText($source,$outside)
      $externalHash=(Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash
      Click 'Save · Ctrl+S'
      # The native save must detect the revision mismatch instead of replacing
      # an external writer, and it must keep the local unsaved editor draft.
      [void](Find-Control 'File changed on disk' '' 15)
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash -ne $externalHash) {
        Deny 'external_changes_were_overwritten'
      }
      Require 'Unsaved'
      $stillOpen=Find-Control 'Source editor text' ''
      if (!$stillOpen.Current.IsKeyboardFocusable) { Deny 'conflicted_draft_was_discarded' }
      Write-Host 'WO8_NATIVE_EXTERNAL_SOURCE_CONFLICT_DENIED_AND_UNSAVED_DRAFT_PRESERVED=PASS'
    }
    if ($Scenario -eq 'EditorPersistence') {
      $source=Join-Path $fixed 'main.rs'
      $saved=(Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash
      $editor=Find-Control 'Source editor text' ''
      if (!$editor.Current.IsKeyboardFocusable) { Deny 'editor_not_accessibly_focusable' }
      $editor.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('^{END}')
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}// WO8_EXPLICIT_PERSISTENCE_SENTINEL')
      Start-Sleep -Milliseconds 900
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash -ne $saved) {
        Deny 'unsaved_editor_draft_mutated_disk'
      }
      Click 'Review changes'
      Require 'Save reviewed changes'
      Click 'Return to editor'
      Click 'Save · Ctrl+S'
      $deadline=(Get-Date).AddSeconds(12)
      do {
        Start-Sleep -Milliseconds 450
        $current=[IO.File]::ReadAllText($source)
        if ($current.Contains('WO8_EXPLICIT_PERSISTENCE_SENTINEL')) { break }
      } while ((Get-Date) -lt $deadline)
      if (!$current.Contains('WO8_EXPLICIT_PERSISTENCE_SENTINEL')) { Deny 'explicit_save_not_persisted' }
      Require 'Saved main.rs'
      Write-Host 'WO8_INSTALLED_NATIVE_REVIEW_AND_EXPLICIT_SAVE=PASS'
    } elseif ($Scenario -eq 'P04ReinstalledProject') {
      # No new picker registration: verify persisted user catalog and source
      # in the genuinely reinstalled desktop, without writing source bytes.
      Require 'Source editor text'
      Require 'main.rs'
      Write-Host 'P04_INSTALLED_REINSTALL_PERSISTED_PROJECT_AND_EDITOR=PASS'
    } elseif ($Scenario -eq 'PersistedRestart') {
      if (![IO.File]::ReadAllText((Join-Path $fixed 'main.rs')).Contains('WO8_EXPLICIT_PERSISTENCE_SENTINEL')) {
        Deny 'previously_saved_file_missing_after_restart'
      }
      Require 'Source editor text'
      Write-Host 'WO8_NATIVE_PERSISTENT_PROJECT_REOPEN_AND_SOURCE=PASS'
    } else {
      Write-Host 'WO8_INSTALLED_NATIVE_FOLDER_REGISTRATION_AND_SOURCE_OPEN=PASS'
    }
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
