# WO8-A/B native Windows UI Automation acceptance. Never executes project code.
# Run on an isolated disposable Windows runner *after* real NSIS installation.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExecutable,
  [Parameter(Mandatory=$true)][string]$ExpectedSha256,
  [ValidateSet('Shell','ProjectReadOnly','EditorPersistence','PersistedRestart','P04ReinstalledProject','ExternalConflict','ProtectedBuildDebug','ProtectedCancellation','ProtectedHostCrash','ProtectedServiceCrash','ProtectedTamperDenial','ProtectedDebuggerCancellation','Wo9WorkflowPortfolio','Wo9ReviewedGit','Wo9ProtectedGraph','Wo9ProtectedGraphCancel','Wo9ProtectedGraphHostCrash','Dev02ProtectedLsp','Dev035JobsReadOnly','Dev045ProjectDebuggerDenied','Dev045CInstalledProjectDebugger','Dev045CInstalledProjectRestartDenied','B4SplitEditor','B5LargeProject','Dev01Modern','Dev01ModernRecovery')][string]$Scenario='Shell',
  [string]$FixtureRoot='',
  [string]$MetricsFile=''
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
$workerBaseline=@{}
foreach($name in @('lldb-dap','clangd','rust-analyzer','ruff','cargo','windows_recovery_service')) {
  $workerBaseline[$name]=BoundedProcess $name
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
  private static extern bool EnumWindows(Enumeration callback, IntPtr data);
  [DllImport("user32.dll")]
  private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
  [DllImport("user32.dll")]
  private static extern bool IsWindowVisible(IntPtr hwnd);
  public static bool IsOwnedConsentWindow(IntPtr hwnd, IntPtr owner, uint expectedPid) {
    if (hwnd == IntPtr.Zero || owner == IntPtr.Zero || hwnd == owner ||
        !IsWindowVisible(hwnd) || GetWindow(hwnd, 4) != owner) return false;
    uint pid, ownerPid;
    return GetWindowThreadProcessId(hwnd, out pid) != 0 &&
      GetWindowThreadProcessId(owner, out ownerPid) != 0 &&
      pid == expectedPid && ownerPid == expectedPid;
  }
  public static IntPtr[] OwnedConsentHandles(IntPtr owner, uint expectedPid) {
    var results = new List<IntPtr>();
    Enumeration callback = (hwnd, data) => {
      if (IsOwnedConsentWindow(hwnd, owner, expectedPid)) results.Add(hwnd);
      return results.Count <= 16;
    };
    EnumWindows(callback, IntPtr.Zero);
    return results.ToArray();
  }
  // Observational only. Counts OS HWNDs; never creates an approval path.
  // Avoid logging HWNDs, titles, clipboard bytes or file content.
  public static string ConsentWindowCensus(IntPtr owner, uint expectedPid) {
    if (owner == IntPtr.Zero) return "invalid_owner";
    uint ownerPid;
    if (GetWindowThreadProcessId(owner, out ownerPid) == 0 ||
        ownerPid != expectedPid) return "owner_identity_unverified";
    int topLevel = 0, directOwner = 0, otherOwner = 0, noOwner = 0;
    Enumeration callback = (hwnd, data) => {
      if (hwnd == owner || !IsWindowVisible(hwnd)) return true;
      uint pid;
      if (GetWindowThreadProcessId(hwnd, out pid) == 0 ||
          pid != expectedPid) return true;
      topLevel++;
      var actualOwner = GetWindow(hwnd, 4);
      if (actualOwner == owner) directOwner++;
      else if (actualOwner == IntPtr.Zero) noOwner++;
      else otherOwner++;
      return topLevel <= 64;
    };
    EnumWindows(callback, IntPtr.Zero);
    int childCount = 0;
    Enumeration children = (hwnd, data) => {
      uint pid;
      if (GetWindowThreadProcessId(hwnd, out pid) != 0 &&
          pid == expectedPid) childCount++;
      return childCount <= 128;
    };
    EnumChildWindows(owner, children, IntPtr.Zero);
    return "same_pid_top_" + topLevel + ";direct_owner_" + directOwner +
      ";other_owner_" + otherOwner + ";no_owner_" + noOwner +
      ";owned_hwnd_children_" + childCount;
  }

  [DllImport("user32.dll", SetLastError=true)]
  private static extern bool EnumChildWindows(IntPtr parent, Enumeration callback, IntPtr data);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")]
  public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")]
  public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
  [DllImport("user32.dll")]
  public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
  [DllImport("user32.dll")]
  public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  // Read only visible text from native child HWNDs of the already-verified
  // consent popup. WM_GETTEXT is bounded and cannot activate any control.
  [DllImport("user32.dll")]
  private static extern bool IsChild(IntPtr parent, IntPtr child);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  private static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint msg,
    UIntPtr wParam, [Out] StringBuilder text, uint flags, uint timeout, out UIntPtr result);
  public static string[] OwnedConsentChildText(IntPtr hwnd, IntPtr owner, uint expectedPid) {
    if (!IsOwnedConsentWindow(hwnd, owner, expectedPid)) return new string[0];
    var children = new List<IntPtr>();
    Enumeration callback = (child, data) => {
      children.Add(child); return children.Count <= 64;
    };
    EnumChildWindows(hwnd, callback, IntPtr.Zero);
    if (children.Count > 64) return new string[0];
    var result = new List<string>();
    foreach (var child in children) {
      uint pid;
      if (!IsOwnedConsentWindow(hwnd, owner, expectedPid)) return new string[0];
      if (!IsChild(hwnd, child) || !IsWindowVisible(child) ||
          GetWindowThreadProcessId(child, out pid) == 0 || pid != expectedPid) continue;
      var buf = new StringBuilder(4096);
      UIntPtr copied;
      // SMTO_BLOCK | SMTO_ABORTIFHUNG; read-only native message.
      if (SendMessageTimeout(child, 0x000D, new UIntPtr(4096), buf, 3, 80, out copied) != IntPtr.Zero &&
          copied.ToUInt64() > 0 && copied.ToUInt64() < 4095) result.Add(buf.ToString());
    }
    return IsOwnedConsentWindow(hwnd, owner, expectedPid) ? result.ToArray() : new string[0];
  }
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
  # Chromium/wry may orphan the top-down UIA fragment on a background app.
  # Inspect the real OS-focused element only when the foreground root is our
  # exact installed Tauri desktop. No foreign windows, JS or backend bypass.
  $top=$proc.MainWindowHandle
  if ([P04NativeWebViewWindows]::GetForegroundWindow() -ne $top) {
    [void][P04NativeWebViewWindows]::SetForegroundWindow($top)
    Start-Sleep -Milliseconds 120
  }
  if ([P04NativeWebViewWindows]::GetForegroundWindow() -eq $top) {
    try {
      $node=[System.Windows.Automation.AutomationElement]::FocusedElement
      $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
      $trace=@()
      $owned=$false
      for($count=0; $node -and $count -lt 14; $count++) {
        $trace+=,$node
        try {
          $raw=$node.Current.NativeWindowHandle
          if ($raw -ne 0) {
            $ancestor=[P04NativeWebViewWindows]::GetAncestor([IntPtr]::new($raw),2)
            if ($ancestor -eq $top) { $owned=$true }
          }
        } catch {}
        $node=$walker.GetParent($node)
      }
      if ($owned) {
        foreach ($candidate in $trace) { Write-Output $candidate }
      }
    } catch {}
  }
}
function Warm-InstalledWebViewAccessibility {
  # A real Windows NSIS run on SHA 05141c9 found a genuine installed shell
  # and renderer HWND but only ONE UIA descendant before the WebView content
  # became accessible. Reassert exact app foreground and use harmless native
  # OS Tab focus changes. Never invoke backend APIs or fake a React result.
  for($attempt=0; $attempt -lt 3; $attempt++) {
    try {
      [void](Find-Control 'Workspace projects' '' 6)
      if($attempt -gt 0) { Write-Host ('P04_NATIVE_WEBVIEW_FOCUS_RECOVERY=real_uia_after_os_tab_'+$attempt) }
      return
    } catch {
      $main=WindowRoot
      if(!$main) { Deny 'installed_native_window_unavailable_during_focus_recovery' }
      $hwnd=$proc.MainWindowHandle
      [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
      Start-Sleep -Milliseconds 180
      if([P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {
        Deny 'installed_native_window_could_not_be_focused'
      }
      [System.Windows.Forms.SendKeys]::SendWait('{TAB}')
      Start-Sleep -Milliseconds 450
    }
  }
  # A frontend render-failure alert is diagnostic only, never a successful
  # UIA workspace witness. The original exact heading is still mandatory.
  try {
    [void](Find-Control 'Lu IDE frontend unavailable' '' 2)
    Write-Host 'WO8_WINDOWS_FRONTEND_DIAGNOSTIC=visible_render_failure'
  } catch {
    Write-Host 'WO8_WINDOWS_FRONTEND_DIAGNOSTIC=workspace_heading_missing_cause_unverified'
  }
  Deny 'installed_webview_accessibility_not_ready_after_native_focus'
}

function Test-EditableNativeEditor($item) {
  try {
    $type=$item.Current.ControlType.ProgrammaticName
    if ($type -ceq 'ControlType.Edit') {
      $value=$null
      if ($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$value)) {
        return !$value.Current.IsReadOnly
      }
      return $true
    }
    if ($type -ceq 'ControlType.Document') {
      $value=$null
      if ($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$value)) {
        return !$value.Current.IsReadOnly
      }
      $tp=$null
      if ($item.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern,[ref]$tp)) {
        $ro=$tp.DocumentRange.GetAttributeValue([System.Windows.Automation.TextPattern]::IsReadOnlyAttribute)
        return ($ro -is [bool] -and !$ro)
      }
    }
  } catch {}
  return $false
}
function Test-ExactNativeEditorIdentity($item,[string]$expected) {
  try {
    if ($expected -cmatch '^Source editor text · (First|Second) editor: (.+)$') {
      $paneName="$($Matches[1]) editor, $($Matches[2])"
    } else {return $false}
    if (!$item.Current.IsEnabled -or $item.Current.IsOffscreen -or
        !$item.Current.IsKeyboardFocusable -or !(Test-EditableNativeEditor $item)) {return $false}
    if ($item.Current.Name -ceq $expected) {return $true}
    if ($item.Current.Name -cne 'Source editor text') {return $false}
    # Only the independently named native file-bearing pane can qualify an
    # otherwise generic contenteditable name; do not match another editor.
    $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
    $parent=$item
    for($i=0;$i -lt 24;$i++) {
      $parent=$walker.GetParent($parent)
      if (!$parent) {return $false}
      $type=$parent.Current.ControlType.ProgrammaticName
      if ($type -ceq 'ControlType.Window') {return $false}
      if ($parent.Current.Name -ceq $paneName -and !$parent.Current.IsOffscreen -and
          $type -cin @('ControlType.Group','ControlType.Pane','ControlType.Custom')) {return $true}
    }
  } catch {}
  return $false
}
function Find-Control([string]$query,[string]$kind,[int]$seconds=18,[switch]$Exact) {
  $start=Get-Date
  $end=$start.AddSeconds($seconds)
  $focusRecoveries=0
  # Actual NSIS/WebView2 acceptance at 5788f9a: immediately after invoking
  # Job requests, UIA exposed only 3 root nodes although its HWND remained
  # visible and foreground. The initial shell focus-recovery proved native
  # Tab can restore Chromium's UIA provider; repeat that bounded method
  # only for known IDE navigation headings, never for a permission dialog.
  $navigationHeading=$query -in @(
    'Workspace projects', 'Execution requests', 'IDE capabilities',
    'Language server providers', 'Packages & workflows',
    'Source control', 'Coding proposals', 'Project continuity'
  )
  while ((Get-Date) -lt $end) {
    # Do not fake a WebView response. Every candidate is a real native UIA
    # provider reached from this exact installed desktop's child HWND chain.
    $exactEditor=$query.StartsWith('Source editor text · ',[StringComparison]::Ordinal)
    $editorMatches=[Collections.Generic.List[object]]::new()
    foreach ($window in @(NativeAccessibilityRoots)) {
      try {
        $all=$window.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)
        if ($exactEditor -and (Test-ExactNativeEditorIdentity $window $query)) {
          $duplicate=$false
          foreach($seen in $editorMatches) {if ($seen.Equals($window)) {$duplicate=$true;break}}
          if (!$duplicate) {$editorMatches.Add($window)}
        }
        foreach($item in $all) {
          try {
            $name=$item.Current.Name
            $type=$item.Current.ControlType.ProgrammaticName
            if ($exactEditor) {
              if (Test-ExactNativeEditorIdentity $item $query) {
                $duplicate=$false
                foreach($seen in $editorMatches) {if ($seen.Equals($item)) {$duplicate=$true;break}}
                if (!$duplicate) {$editorMatches.Add($item)}
              }
              continue
            }
            if ($query -eq 'Source editor text') {
              # Only the real visible editable content surface, never the
              # enclosing CodeMirror region or a stale offscreen renderer.
              $matches=($name -ceq $query -or ($query -eq 'Source editor text' -and
                $name -and $name.StartsWith('Source editor text · ',[StringComparison]::Ordinal)))
              if ($matches -and $item.Current.IsEnabled -and !$item.Current.IsOffscreen -and
                  $item.Current.IsKeyboardFocusable -and (Test-EditableNativeEditor $item)) {return $item}
              continue
            }
            if ($name -and (($Exact -and $name -ceq $query) -or
                (!$Exact -and $name.IndexOf($query,[StringComparison]::OrdinalIgnoreCase) -ge 0)) -and
                (!$kind -or $type -eq ('ControlType.'+$kind))) {
              return $item
            }
          } catch {}
        }
      } catch {}
    }
    if ($exactEditor -and $editorMatches.Count -eq 1) {return $editorMatches[0]}
    # Do not convert an unavailable WebView into a fabricated UI pass.
    # Only re-focus the exact native installed process (max 2 harmless Tabs),
    # and still require the requested real UIA element to materialize.
    if ($navigationHeading -and $focusRecoveries -lt 2 -and
        ((Get-Date)-$start).TotalSeconds -ge (4 + 4*$focusRecoveries)) {
      $main=WindowRoot
      if ($main -and $proc.MainWindowHandle -ne [IntPtr]::Zero) {
        $hwnd=$proc.MainWindowHandle
        [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
        Start-Sleep -Milliseconds 150
        if ([P04NativeWebViewWindows]::GetForegroundWindow() -eq $hwnd) {
          [System.Windows.Forms.SendKeys]::SendWait('{TAB}')
          Write-Host ('WO8_NATIVE_UIA_POST_NAV_FOCUS_RECOVERY='+$query+
            ';attempt='+($focusRecoveries+1))
          Start-Sleep -Milliseconds 250
        }
      }
      $focusRecoveries++
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
      $front=[P04NativeWebViewWindows]::GetForegroundWindow()
      Write-Host ('P04_NATIVE_INSTALLED_WINDOW_FOREGROUND='+[string]($front -eq $proc.MainWindowHandle))
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
function Find-Wo9CommitField([string]$label,[int]$seconds=10) {
  $until=(Get-Date).AddSeconds($seconds)
  do {
    $form=Find-Control 'Reviewed local commit details' 'Group' 2 -Exact
    $fields=@()
    try {
      foreach($candidate in $form.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)) {
        if ($candidate.Current.Name -ceq $label -and
            $candidate.Current.ControlType.ProgrammaticName -ceq 'ControlType.Edit' -and
            $candidate.Current.IsEnabled -and $candidate.Current.IsKeyboardFocusable) {
          $fields+=,$candidate
        }
      }
    } catch {$fields=@()}
    if ($fields.Count -eq 1) {return $fields[0]}
    Start-Sleep -Milliseconds 150
  } while((Get-Date) -lt $until)
  Deny ('wo9_exact_commit_form_field_not_verified_'+$label.Replace(' ','_'))
}
function Reveal-NativeControl($element) {
  if (!$element.Current.IsOffscreen) {return}
  # The editor now protects its viewport with independently scrollable
  # chrome. Reveal actual native controls, never alter application state.
  $scroll=$null
  if ($element.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern,[ref]$scroll)) {
    $scroll.ScrollIntoView()
  } elseif ($element.Current.IsKeyboardFocusable) {
    $element.SetFocus()
  }
  Start-Sleep -Milliseconds 250
}
function Focus-ExactNativeControl([string]$label,[string]$kind='') {
  $hwnd=$proc.MainWindowHandle
  if ($hwnd -eq [IntPtr]::Zero) { Deny 'exact_native_focus_missing_window' }
  for ($attempt=0; $attempt -lt 3; $attempt++) {
    [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
    $item=Find-Control $label $kind 4 -Exact
    if (!$item.Current.IsEnabled -or !$item.Current.IsKeyboardFocusable) { continue }
    try {
      Reveal-NativeControl $item
      $item=Find-Control $label $kind 4 -Exact
      $item.SetFocus()
    } catch {}
    $until=(Get-Date).AddSeconds(3)
    do {
      $item=Find-Control $label $kind 2 -Exact
      $actual=[System.Windows.Automation.AutomationElement]::FocusedElement
      if ($item.Current.IsEnabled -and !$item.Current.IsOffscreen -and
          (($label.StartsWith('Source editor text · ',[StringComparison]::Ordinal) -and
            (Test-ExactNativeEditorIdentity $item $label)) -or
            (!$label.StartsWith('Source editor text · ',[StringComparison]::Ordinal) -and $item.Current.Name -ceq $label)) -and
          [P04NativeWebViewWindows]::GetForegroundWindow() -eq $hwnd -and
          ($item.Current.HasKeyboardFocus -or ($actual -and $actual.Equals($item)))) {
        return $item
      }
      Start-Sleep -Milliseconds 160
    } while ((Get-Date) -lt $until)
  }
  Deny ('exact_native_focus_unverified_'+$label.Replace(' ','_'))
}
function Click([string]$label) {
  $reviewToolbar=$label -cin @('New file','New folder','Move file','Trash file','Restore file','Toggle split editor','Confirm separate local commit (no push)')
  if ($reviewToolbar) {
    # UIA Invoke is not guaranteed for Chromium disclosure/toggle buttons.
    # Use exactly one real OS pointer action, with a verified hit on the
    # named control in the foreground installed process. No retry after
    # dispatch; the caller must still observe the actual review form,
    # separate native consent and independent on-disk outcome.
    $denial='ui_invoke_failure_'+$label.Replace(' ','_')
    $hwnd=$proc.MainWindowHandle
    if ($hwnd -eq [IntPtr]::Zero) {Deny $denial}
    [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
    $fresh=Find-Control $label 'Button' 12 -Exact
    if (!$fresh.Current.IsEnabled) {Deny $denial}
    try {Reveal-NativeControl $fresh} catch {Deny $denial}
    $fresh=Find-Control $label 'Button' 6 -Exact
    if (!$fresh.Current.IsEnabled -or $fresh.Current.IsOffscreen -or
        $fresh.Current.Name -cne $label -or
        [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {Deny $denial}
    try {
      $bounds=(WindowRoot).Current.BoundingRectangle
      $rect=$fresh.Current.BoundingRectangle
      if ($rect.Width -lt 5 -or $rect.Height -lt 5) {Deny $denial}
      # Some WebView2 providers expose a bounding rectangle but not
      # GetClickablePoint. The fallback must still pass the SAME exact
      # native hit-test at the rounded coordinates actually dispatched.
      try {$point=$fresh.GetClickablePoint()} catch {
        $point=[System.Windows.Point]::new($rect.Left+$rect.Width/2,$rect.Top+$rect.Height/2)
      }
      $point=[System.Windows.Point]::new([Math]::Round($point.X),[Math]::Round($point.Y))
      if (!$bounds.Contains($point) -or !$rect.Contains($point)) {Deny $denial}
      $hit=[System.Windows.Automation.AutomationElement]::FromPoint($point)
      $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
      $exactHit=$false
      for ($i=0;$hit -and $i -lt 8;$i++) {
        if ($hit.Equals($fresh)) {$exactHit=$true; break}
        $hit=$walker.GetParent($hit)
      }
      if (!$exactHit -or [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {Deny $denial}
      [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new([int]$point.X,[int]$point.Y)
      [P04NativeWebViewWindows]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
      [P04NativeWebViewWindows]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
      Write-Host 'WO8_SOURCE_TOOLBAR_EXACT_NATIVE_POINTER=attempted_not_acceptance'
      return
    } catch {Deny $denial}
  }
  $element=Find-Control $label 'Button'
  if (!$element.Current.IsEnabled) { Deny ('button_disabled_'+$label.Replace(' ','_')) }
  try {
    Reveal-NativeControl $element
    $element=Find-Control $label 'Button'
    if (!$element.Current.IsEnabled -or $element.Current.IsOffscreen) {Deny ('button_not_visible_'+$label.Replace(' ','_'))}
    $p=$element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $p.Invoke()
  } catch {Deny ('ui_invoke_failure_'+$label.Replace(' ','_'))}
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
# Validate the actual ordered-dictionary samples, not projected properties.
function Get-B5MemoryPeaks($samples) {
  if ($null -eq $samples -or @($samples).Count -lt 6) {Deny 'b5_missing_memory_samples'}
  $rss=[long]0; $private=[long]0
  $seen=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
  $stages=@('opened_large_main','opened_12_files','opened_24_files','opened_36_files','opened_48_files','after_explicit_save')
  foreach ($sample in $samples) {
    if ($sample -isnot [System.Collections.IDictionary] -or
        !$sample.Contains('stage') -or !$sample.Contains('rssBytes') -or
        !$sample.Contains('privateBytes') -or !$sample.Contains('observedProcessCount') -or
        !$sample.Contains('scope') -or $sample['scope'] -cne 'installed-pid-and-live-Win32_Process-descendants' -or
        $sample['stage'] -cnotin $stages -or !$seen.Add([string]$sample['stage'])) {Deny 'b5_invalid_memory_sample'}
    foreach ($field in @('rssBytes','privateBytes','observedProcessCount')) {
      $value=$sample[$field]
      if (($value -isnot [int] -and $value -isnot [long]) -or $value -le 0) {Deny 'b5_invalid_memory_sample_value'}
    }
    if ($sample['observedProcessCount'] -gt 128) {Deny 'b5_unbounded_descendant_process_count'}
    $rss=[Math]::Max($rss,[long]$sample['rssBytes'])
    $private=[Math]::Max($private,[long]$sample['privateBytes'])
  }
  if ($seen.Count -ne $stages.Count) {Deny 'b5_incomplete_memory_stage_set'}
  return [ordered]@{rssPeakObservedBytes=$rss;privatePeakObservedBytes=$private}
}
# A SHA witness must be a complete token; never reconstruct fragments.
function Test-NativeConsentMessage([string[]]$texts,[string]$expected) {
  if ([string]::IsNullOrWhiteSpace($expected)) {return $false}
  $normalized=($expected -replace '\s+',' ').Trim()
  foreach ($text in $texts) {
    if ([string]::IsNullOrWhiteSpace($text) -or $text.Length -gt 16384) {continue}
    $observed=($text -replace '\s+',' ').Trim()
    if ($normalized -cmatch '^[0-9a-f]{64}$') {
      if ($observed -cmatch ('(?<![0-9A-Fa-f])'+[regex]::Escape($normalized)+'(?![0-9A-Fa-f])')) {return $true}
    } elseif ($observed.IndexOf($normalized,[StringComparison]::Ordinal) -ge 0) {return $true}
  }
  return $false
}
function Get-NativeConsentText($win,$desc) {
  $texts=[System.Collections.Generic.List[string]]::new()
  foreach ($item in @($win)+@($desc)) {
    try {
      if ($item.Current.IsOffscreen) {continue}
      $name=[string]$item.Current.Name
      if ($name.Length -le 16384) {$texts.Add($name)}
      $pattern=$null
      if ($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)) {
        $value=[string]$pattern.Current.Value
        if ($value.Length -le 16384) {$texts.Add($value)}
      }
      $pattern=$null
      if ($item.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern,[ref]$pattern)) {
        $value=[string]$pattern.DocumentRange.GetText(16385)
        if ($value.Length -le 16384) {$texts.Add($value)}
      }
    } catch {}
  }
  return $texts.ToArray()
}
# Exact native OS title witnesses: Windows TaskDialog body text is not
# consistently exposed through UIA or WM_GETTEXT. This checks a complete
# independently calculated digest in the genuine owned popup, never WebView text.
function Native-Wo9TitleWitness([string]$titleFragment,[string]$expectedMessage) {
  if ($titleFragment -ceq 'One-file Git stage') {
    if ($expectedMessage -cnotmatch '^[0-9a-f]{64}$') { Deny 'wo9_stage_witness_invalid' }
    return ('Lu IDE · One-file Git stage · SHA-256 '+$expectedMessage)
  }
  if ($titleFragment -ceq 'Confirm local Git commit') {
    if ([string]::IsNullOrWhiteSpace($expectedMessage) -or $expectedMessage.Length -gt 1200) {
      Deny 'wo9_commit_witness_invalid'
    }
    $messageSha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
      [Text.Encoding]::UTF8.GetBytes($expectedMessage))).ToLowerInvariant()
    return ('Lu IDE · Confirm local Git commit · Message SHA-256 '+$messageSha256)
  }
  Deny 'wo9_unexpected_native_witness_contract'
}
function Confirm-NativeOneUseDialog([string]$titleFragment,[string]$approvalCaption,[string]$expectedMessage='', [IntPtr]$expectedOwnerHwnd=[IntPtr]::Zero) {
  # Real WO9 Windows TaskDialog: exact caption exposes UIA Text/Pane, not Button.
  # Only interact with a separate OS popup owned by this installed process.
  $deadline=(Get-Date).AddSeconds(35)
  $desktop=[System.Windows.Automation.AutomationElement]::RootElement
  $ownedRoots=0; $titleMatches=0; $buttonMatches=0
  $matchedTextPane=0; $unusableNativePopup=0; $unusableBounds=0
  $unavailableNativeAction=0; $messageMatches=0; $messageMissing=0; $nativeMessageMatches=0; $ambiguousControls=0
  $nativeTitleWitnessMatches=0
  $exactOwnedTitle=''
  if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
    $exactOwnedTitle=Native-Wo9TitleWitness $titleFragment $expectedMessage
  }
  while ((Get-Date) -lt $deadline) {
    if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
      # WO9: the installed popup was visible as a nested UIA Window, but
      # absent from the desktop-child enumeration. Observe the real owned
      # native HWND rather than re-focusing the underlying main WebView.
      $handles=@([P04NativeWebViewWindows]::OwnedConsentHandles($expectedOwnerHwnd,[uint32]$proc.Id))
      if ($handles.Count -gt 16) {Deny 'native_consent_window_scope_overflow'}
      $roots=@()
      foreach($nativeHwnd in $handles) {
        try {
          $nativeRoot=[System.Windows.Automation.AutomationElement]::FromHandle($nativeHwnd)
          if ($nativeRoot) {$roots+=,$nativeRoot}
        } catch {}
      }
      $roots=@($roots | Where-Object {
        try {$_.Current.Name -ceq $exactOwnedTitle} catch {$false}
      })
      if ($roots.Count -ne 1) {Start-Sleep -Milliseconds 350; continue}
    } else {
      $roots=$desktop.FindAll([System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition)
    }
    foreach($win in $roots) {
      try {
        if ($win.Current.ProcessId -ne $proc.Id -or !$win.Current.IsEnabled) { continue }
        $type=$win.Current.ControlType
        if ($type -ne [System.Windows.Automation.ControlType]::Window -and
            $type -ne [System.Windows.Automation.ControlType]::Pane) { continue }
        $ownedRoots++
        $name=[string]$win.Current.Name
        if ($name.IndexOf($titleFragment,[StringComparison]::OrdinalIgnoreCase) -lt 0) { continue }
        $titleMatches++
        $hwnd=[IntPtr]::new([int64]$win.Current.NativeWindowHandle)
        if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
          if (![P04NativeWebViewWindows]::IsOwnedConsentWindow($hwnd,$expectedOwnerHwnd,[uint32]$proc.Id) -or
              $name -cne $exactOwnedTitle) {
            $unusableNativePopup++; continue
          }
        } elseif ($hwnd -eq [IntPtr]::Zero -or $hwnd -eq $proc.MainWindowHandle) {
          $unusableNativePopup++; continue
        }
        $windowBounds=$win.Current.BoundingRectangle
        if ($windowBounds.Width -lt 80 -or $windowBounds.Height -lt 45) {
          $unusableBounds++; continue
        }
        $desc=$win.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)
        if ($desc.Count -gt 512) {continue}
        if ($expectedMessage) {
          $texts=@(Get-NativeConsentText $win $desc)
          $matched=Test-NativeConsentMessage $texts $expectedMessage
          if (!$matched -and $expectedOwnerHwnd -ne [IntPtr]::Zero) {
            $nativeTexts=@([P04NativeWebViewWindows]::OwnedConsentChildText($hwnd,$expectedOwnerHwnd,[uint32]$proc.Id))
            $matched=Test-NativeConsentMessage $nativeTexts $expectedMessage
            if ($matched) {$nativeMessageMatches++}
          }
          # A native title bearing the full exact digest can witness this
          # approval even if TaskDialog's body cannot be read by UIA.
          # The owner, process, exact title and visible approval control
          # are still independently verified; no partial hash is accepted.
          if (!$matched -and $expectedOwnerHwnd -ne [IntPtr]::Zero -and
              $name -ceq $exactOwnedTitle) {
            $matched=$true
            $nativeTitleWitnessMatches++
          }
          # Non-hash prose can be split over UIA nodes in legacy dialogs;
          # preserve the original visible-text path without joining a SHA.
          if (!$matched -and $expectedOwnerHwnd -eq [IntPtr]::Zero -and
              $expectedMessage -cnotmatch '^[0-9a-f]{64}$') {
            $visibleNames=@($desc | Where-Object { !$_.Current.IsOffscreen } | ForEach-Object { [string]$_.Current.Name }) -join ' '
            $matched=Test-NativeConsentMessage @($visibleNames) $expectedMessage
          }
          if (!$matched) {$messageMissing++; continue}
          $messageMatches++
        }
        if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
          $matches=@($desc | Where-Object {
            try {
              $_.Current.Name -ceq $approvalCaption -and $_.Current.IsEnabled -and !$_.Current.IsOffscreen -and
                $_.Current.ControlType -in @([System.Windows.Automation.ControlType]::Button,
                  [System.Windows.Automation.ControlType]::Text,[System.Windows.Automation.ControlType]::Pane)
            } catch {$false}
          })
          # No choosing among indistinguishable native approval controls.
          if ($matches.Count -ne 1) {$ambiguousControls++;continue}
        }
        foreach($child in $desc) {
          try {
            if ([string]$child.Current.Name -cne $approvalCaption -or
                !$child.Current.IsEnabled -or $child.Current.IsOffscreen) { continue }
            $kind=$child.Current.ControlType
            $isButton=$kind -eq [System.Windows.Automation.ControlType]::Button
            $isTextPane=$kind -eq [System.Windows.Automation.ControlType]::Text -or
              $kind -eq [System.Windows.Automation.ControlType]::Pane
            if (!$isButton -and !$isTextPane) { continue }
            if ($expectedOwnerHwnd -ne [IntPtr]::Zero -and
                ![P04NativeWebViewWindows]::IsOwnedConsentWindow($hwnd,$expectedOwnerHwnd,[uint32]$proc.Id)) {
              $unusableNativePopup++;continue
            }
            if ($isButton) {
              $buttonMatches++
              try {
                $child.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                Write-Host 'P04_NATIVE_ONE_USE_DIALOG=real_owned_os_button_invoked'
                return
              } catch { $unavailableNativeAction++ }
            }
            if ($isTextPane) {
              $matchedTextPane++
              $rect=$child.Current.BoundingRectangle
              if ($rect.Width -lt 5 -or $rect.Height -lt 5) {
                $unusableBounds++; continue
              }
              $px=[int][Math]::Round($rect.Left+$rect.Width/2)
              $py=[int][Math]::Round($rect.Top+$rect.Height/2)
              if ($px -le $windowBounds.Left+2 -or $px -ge $windowBounds.Right-2 -or
                  $py -le $windowBounds.Top+2 -or $py -ge $windowBounds.Bottom-2) {
                $unusableBounds++; continue
              }
              if (![P04NativeWebViewWindows]::SetForegroundWindow($hwnd)) {
                $unavailableNativeAction++; continue
              }
              if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
                if ([P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {
                  $unavailableNativeAction++;continue
                }
                $hit=[System.Windows.Automation.AutomationElement]::FromPoint([System.Windows.Point]::new($px,$py))
                $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
                $exactHit=$false
                for($hop=0;$hit -and $hop -lt 8;$hop++) {
                  if ($hit.Equals($child)) {$exactHit=$true;break}
                  $hit=$walker.GetParent($hit)
                }
                if (!$exactHit) {$unavailableNativeAction++;continue}
              }
              [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new($px,$py)
              [P04NativeWebViewWindows]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
              [P04NativeWebViewWindows]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
              Write-Host 'P04_NATIVE_ONE_USE_DIALOG=exact_owned_native_text_pane_mouse_click'
              return
            }
          } catch { $unavailableNativeAction++ }
        }
      } catch {}
    }
    Start-Sleep -Milliseconds 350
  }
  if ($expectedOwnerHwnd -ne [IntPtr]::Zero) {
    # Bound same-PID window census. No owned-dialog fallback or approval.
    Write-Host ('P04_NATIVE_CONSENT_HWND_CENSUS='+
      [P04NativeWebViewWindows]::ConsentWindowCensus($expectedOwnerHwnd,[uint32]$proc.Id))
  }
  Write-Host ('P04_NATIVE_CONSENT_OBSERVATION=owned_roots_'+$ownedRoots+
    ';matched_titles_'+$titleMatches+
    ';button_candidates_'+$buttonMatches+';text_pane_candidates_'+$matchedTextPane+
    ';native_popup_unusable_'+$unusableNativePopup+
    ';unusable_bounds_'+$unusableBounds+
    ';native_action_unavailable_'+$unavailableNativeAction+
    ';message_matches_'+$messageMatches+';message_missing_'+$messageMissing+
    ';native_message_matches_'+$nativeMessageMatches+
    ';native_title_witness_matches_'+$nativeTitleWitnessMatches+
    ';ambiguous_controls_'+$ambiguousControls)
  Deny 'real_native_consent_dialog_not_observed'
}
try {
  $proc=Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
  $root=Find-Control 'Lu IDE' '' 30
  $stage='shell'
  Warm-InstalledWebViewAccessibility
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
  if ($Scenario -notin @('PersistedRestart','P04ReinstalledProject','Dev045CInstalledProjectRestartDenied','Dev01ModernRecovery')) {
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
    $temporary=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
    if (!$fixed.StartsWith($temporary,[StringComparison]::OrdinalIgnoreCase) -or
        !(Test-Path -LiteralPath (Join-Path $fixed 'main.rs'))) {
      Deny 'non_disposable_or_missing_fixture'
    }
    if ($Scenario -notin @('PersistedRestart','P04ReinstalledProject','Dev045CInstalledProjectRestartDenied','Dev01ModernRecovery')) {
      # Native chooser name is not an IDE project registration.
      [System.Windows.Forms.SendKeys]::SendWait('^l')
      if ($fixed -match '[+^%~(){}\[\]]') { Deny 'native_picker_path_contains_keyboard_metacharacters' }
      [System.Windows.Forms.SendKeys]::SendWait($fixed)
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      Start-Sleep -Milliseconds 900
      $accepted=$false
      $sawChooser=$false
      $deadline=(Get-Date).AddSeconds(16)
      while (!$accepted -and (Get-Date) -lt $deadline) {
        $desktop=[System.Windows.Automation.AutomationElement]::RootElement
        $windows=$desktop.FindAll([System.Windows.Automation.TreeScope]::Children,
          [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window))
        foreach ($dialog in $windows) {
          try {
            $hwnd=[IntPtr]::new([int64]$dialog.Current.NativeWindowHandle)
            if ($hwnd -eq [IntPtr]::Zero -or $hwnd -eq $proc.MainWindowHandle) { continue }
            $trusted=($dialog.Current.ProcessId -eq $proc.Id -or
              [P04NativeWebViewWindows]::GetWindow($hwnd,4) -eq $proc.MainWindowHandle)
            if (!$trusted) { continue }
            $children=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,
              [System.Windows.Automation.Condition]::TrueCondition)
            foreach ($entry in $children) {
              $name=[string]$entry.Current.Name
              $kind=[string]$entry.Current.ControlType.ProgrammaticName
              if ($name -notin @('Select Folder','Choose Folder','Select','Open') -or
                  $kind -notin @('ControlType.Button','ControlType.Pane')) { continue }
              $sawChooser=$true
              if (!$entry.Current.IsEnabled) { continue }
              # Windows 2022 real IFileDialog exposed Select Folder as Pane.
              # First use InvokePattern; else actual user32 mouse input,
              # bounded to the exact process-owned dialog rectangle.
              try {
                $invoke=$entry.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                $invoke.Invoke()
                $accepted=$true
              } catch {
                $rect=$entry.Current.BoundingRectangle
                $owner=$dialog.Current.BoundingRectangle
                if ($rect.Width -lt 8 -or $rect.Height -lt 8) { continue }
                $px=[int][Math]::Round($rect.Left+$rect.Width/2)
                $py=[int][Math]::Round($rect.Top+$rect.Height/2)
                if ($px -le $owner.Left -or $px -ge $owner.Right -or
                    $py -le $owner.Top -or $py -ge $owner.Bottom) { continue }
                [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
                [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new($px,$py)
                [P04NativeWebViewWindows]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
                [P04NativeWebViewWindows]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
                $accepted=$true
                Write-Host 'WO8_NATIVE_WINDOWS_PICKER=real_owned_pane_mouse_click'
              }
              if ($accepted) { break }
            }
            if ($accepted) { break }
          } catch {}
        }
        if (!$accepted) { Start-Sleep -Milliseconds 350 }
      }
      if (!$accepted) {
        # Real Windows 2022 UIA logs showed the native Select Folder pane
        # INSIDE the installed desktop window's fragment, not as a separate
        # RootElement child window. Search only that exact installed HWND.
        $installedRoot=WindowRoot
        if (!$installedRoot) { Deny 'installed_window_missing_during_picker' }
        $all=$installedRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($entry in $all) {
          try {
            if ([string]$entry.Current.Name -ne 'Select Folder' -or
                [string]$entry.Current.ControlType.ProgrammaticName -notin
                  @('ControlType.Pane','ControlType.Button')) { continue }
            $sawChooser=$true
            $rect=$entry.Current.BoundingRectangle
            $bounds=$installedRoot.Current.BoundingRectangle
            if ($rect.Width -lt 8 -or $rect.Height -lt 8) {
              Write-Host 'WO8_NATIVE_WINDOWS_PICKER=pane_without_clickable_bounds'
              continue
            }
            $px=[int][Math]::Round($rect.Left+$rect.Width/2)
            $py=[int][Math]::Round($rect.Top+$rect.Height/2)
            if ($px -le $bounds.Left -or $px -ge $bounds.Right -or
                $py -le $bounds.Top -or $py -ge $bounds.Bottom) {
              Write-Host 'WO8_NATIVE_WINDOWS_PICKER=pane_outside_installed_window'
              continue
            }
            # Only physical native OS pointer events; zero web scripting.
            [void][P04NativeWebViewWindows]::SetForegroundWindow($proc.MainWindowHandle)
            [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new($px,$py)
            [P04NativeWebViewWindows]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
            [P04NativeWebViewWindows]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
            $accepted=$true
            Write-Host 'WO8_NATIVE_WINDOWS_PICKER=installed_hwnd_native_pane_click'
            break
          } catch {}
        }
      }
      if (!$accepted -and $sawChooser) { Deny 'native_picker_owned_dialog_could_not_be_confirmed' }
      if ($accepted) {
        Write-Host 'WO8_NATIVE_WINDOWS_PICKER=owned_native_dialog_confirmed'
        Start-Sleep -Milliseconds 800
      }
      # Explicitly deny a chooser that remains open after native action.
      # A folder name inside that chooser is not the app catalog.
      $desktop=[System.Windows.Automation.AutomationElement]::RootElement
      $windows=$desktop.FindAll([System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.PropertyCondition]::new(
          [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
          [System.Windows.Automation.ControlType]::Window))
      foreach ($dialog in $windows) {
        try {
          $hwnd=[IntPtr]::new([int64]$dialog.Current.NativeWindowHandle)
          if ($hwnd -eq [IntPtr]::Zero -or $hwnd -eq $proc.MainWindowHandle) { continue }
          $owned=($dialog.Current.ProcessId -eq $proc.Id -or
            [P04NativeWebViewWindows]::GetWindow($hwnd,4) -eq $proc.MainWindowHandle)
          if (!$owned) { continue }
          $controls=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition)
          foreach ($item in $controls) {
            if ([string]$item.Current.Name -eq 'Select Folder') {
              Deny 'native_folder_picker_still_open_after_confirmation'
            }
          }
        } catch {
          if ($_.Exception.Message -like 'WO8_NATIVE_UI_DENIED:*') { throw }
        }
      }
      # Recheck the installed main window provider as well. A still-open
      # Windows IFileDialog may be nested inside this fragment instead of
      # having its own desktop RootElement child HWND.
      $installedRoot=WindowRoot
      if (!$installedRoot) { Deny 'installed_window_disappeared_after_picker' }
      $remaining=$installedRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition)
      foreach ($item in $remaining) {
        try {
          if ([string]$item.Current.Name -eq 'Select Folder' -and
              [string]$item.Current.ControlType.ProgrammaticName -in
                @('ControlType.Pane','ControlType.Button')) {
            Deny 'native_picker_still_open_in_installed_hwnd'
          }
        } catch {
          if ($_.Exception.Message -like 'WO8_NATIVE_UI_DENIED:*') { throw }
        }
      }
      if (!$accepted) { Write-Host 'WO8_NATIVE_WINDOWS_PICKER=dialog_closed_by_os_path_enter' }
    }
    Require 'Registered projects'
    Require (Split-Path $fixed -Leaf)
    # The registry persists the directory across desktop restart but the
    # newly launched desktop intentionally restores it as CLOSED. The real
    # UI exposes Reopen, not Edit, until that explicit project transition.
    # Never bypass via host IPC or assume catalog presence means active.
    $closed=$null
    try { $closed=Find-Control 'Closed · ready to reopen' '' 3 } catch {}
    if ($closed) {
      Click 'Reopen'
      Require 'Open · editor available'
      Write-Host 'WO8_NATIVE_PROJECT_REOPEN=real_installed_ui_reactivated'
    } else {
      Require 'Open · editor available'
    }
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
      $gitHeadBefore=(git -C $fixed rev-parse HEAD).Trim()
      if ($LASTEXITCODE -ne 0) { Deny 'wo9_fixture_git_head_unavailable' }
      $pathInput=Find-Control 'Project-relative tracked file' 'Edit' 20
      $pathInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('main.rs')
      Click 'Inspect exact Git file'
      Require 'Reviewed local file'
      $proc.Refresh()
      $stageOwnerHwnd=$proc.MainWindowHandle
      if ($stageOwnerHwnd -eq [IntPtr]::Zero) {Deny 'wo9_stage_owner_window_not_verified'}
      $expectedStageHash=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed 'main.rs')).Hash.ToLowerInvariant()
      Click 'Confirm and stage these reviewed bytes'
      Confirm-NativeOneUseDialog 'One-file Git stage' 'Stage reviewed bytes' $expectedStageHash $stageOwnerHwnd
      Require 'Staged exact file'
      # Verify bytes independently of the accessibility receipt before commit consent.
      $expectedStageBlob=(git -C $fixed hash-object --no-filters -- main.rs).Trim()
      if ($LASTEXITCODE -ne 0 -or $expectedStageBlob -cnotmatch '^[0-9a-f]{40}$') {Deny 'wo9_native_source_blob_unavailable'}
      $stageDeadline=(Get-Date).AddSeconds(12)
      $stageObserved=$false
      do {
        $stagedFiles=@(git -C $fixed diff --cached --name-only --)
        if ($LASTEXITCODE -ne 0) {Deny 'wo9_native_index_readback_failed'}
        $stageBlob=(git -C $fixed rev-parse :main.rs).Trim()
        if ($LASTEXITCODE -ne 0) {Deny 'wo9_native_index_blob_unavailable'}
        if ($stagedFiles.Count -eq 1 -and $stagedFiles[0] -ceq 'main.rs' -and $stageBlob -ceq $expectedStageBlob) {
          $stageObserved=$true; break
        }
        if ($stagedFiles.Count -gt 1 -or ($stagedFiles.Count -eq 1 -and $stagedFiles[0] -cne 'main.rs')) {Deny 'wo9_native_unreviewed_index_change'}
        Start-Sleep -Milliseconds 200
      } while ((Get-Date) -lt $stageDeadline)
      if (!$stageObserved -or (git -C $fixed rev-parse HEAD).Trim() -cne $gitHeadBefore) {Deny 'wo9_native_stage_not_observed'}
      Write-Host 'WO9_NATIVE_STAGE_EXACT_INDEX_AND_SOURCE_BLOB=PASS'
      foreach($pair in @(
          @('Local commit message','WO9 installed reviewed Git source'),
          @('Author name','WO9 Native Acceptance'),
          @('Author email','wo9@localhost.invalid'))) {
        # WebView2 UIA SetValue may change the provider's displayed value
        # without dispatching the physical input events React needs. Use
        # actual foreground keyboard input and verify the native value.
        $edit=Find-Wo9CommitField $pair[0] 20
        if (!$edit.Current.IsEnabled -or !$edit.Current.IsKeyboardFocusable) {
          Deny 'wo9_commit_input_unavailable'
        }
        $hwnd=$proc.MainWindowHandle
        if ($hwnd -eq [IntPtr]::Zero) { Deny 'wo9_commit_window_unavailable' }
        [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
        if ([P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {
          Deny 'wo9_commit_window_not_foreground'
        }
        Reveal-NativeControl $edit
        $edit=Find-Wo9CommitField $pair[0] 5
        $edit.SetFocus()
        Start-Sleep -Milliseconds 150
        $actual=[System.Windows.Automation.AutomationElement]::FocusedElement
        if ($edit.Current.IsOffscreen -or
            [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd -or
            !($edit.Current.HasKeyboardFocus -or ($actual -and $actual.Equals($edit)))) {
          Deny 'wo9_commit_scoped_input_focus_not_verified'
        }
        [System.Windows.Forms.SendKeys]::SendWait('^a')
        [System.Windows.Forms.Clipboard]::SetText($pair[1])
        [System.Windows.Forms.SendKeys]::SendWait('^v')
        $deadline=(Get-Date).AddSeconds(8)
        $verified=$false
        do {
          $fresh=Find-Wo9CommitField $pair[0] 2
          $value=$fresh.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
          if ($value -ceq $pair[1]) { $verified=$true; break }
          Start-Sleep -Milliseconds 200
        } while ((Get-Date) -lt $deadline)
        if (!$verified) { Deny 'wo9_commit_input_readback_mismatch' }
      }
      foreach($pair in @(
          @('Local commit message','WO9 installed reviewed Git source'),
          @('Author name','WO9 Native Acceptance'),
          @('Author email','wo9@localhost.invalid'))) {
        $field=Find-Wo9CommitField $pair[0] 5
        if ($field.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne $pair[1]) {
          Deny 'wo9_commit_all_fields_readback_mismatch'
        }
      }
      # Do not enable the button programmatically. Wait only for the actual
      # installed application's ordinary input and staging gates to settle.
      $deadline=(Get-Date).AddSeconds(12)
      do {
        $confirm=Find-Control 'Confirm separate local commit' 'Button' 2
        if ($confirm.Current.IsEnabled) { break }
        Start-Sleep -Milliseconds 200
      } while ((Get-Date) -lt $deadline)
      if (!$confirm.Current.IsEnabled) { Deny 'button_disabled_Confirm_separate_local_commit' }
      # Exactly one verified foreground pointer dispatch to the reviewed
      # button; the native modal and independent Git bytes remain mandatory.
      $commitOwnerHwnd=$proc.MainWindowHandle
      if ($commitOwnerHwnd -eq [IntPtr]::Zero -or
          [P04NativeWebViewWindows]::GetForegroundWindow() -ne $commitOwnerHwnd) {
        Deny 'wo9_commit_owner_window_not_verified'
      }
      Click 'Confirm separate local commit (no push)'
      try {
        Confirm-NativeOneUseDialog 'Confirm local Git commit' 'Commit exact staged file' 'WO9 installed reviewed Git source' $commitOwnerHwnd
      } catch {
        # Fail closed, but distinguish UI validation/dispatch from a missing
        # native popup; do not retry the commit or print submitted metadata.
        foreach($category in @(@('Enter valid commit message and author details','validation_rejected'),
          @('Awaiting native confirmation for the exact staged Git file','awaiting_native_confirmation'),
          @('Native Git commit not verified','native_result_unverified'),
          @('Native Git commit cancelled','native_refusal'))) {
          try {
            $observed=Find-Control $category[0] '' 2
            if ($observed) { Write-Host ('WO9_NATIVE_COMMIT_STAGE='+$category[1]) }
          } catch {}
        }
        throw
      }
      # Do not accept the form's 'Local commit message' input as a receipt.
      # Real Win UIA consent invokes asynchronously: the exact native Git
      # operation is the witness, not the OS dialog click or a UI label.
      # Wait a bounded interval for a genuinely changed, resolvable commit.
      $after=$gitHeadBefore
      $gitDeadline=(Get-Date).AddSeconds(24)
      while ((Get-Date) -lt $gitDeadline) {
        $after=(git -C $fixed rev-parse HEAD).Trim()
        if ($LASTEXITCODE -ne 0) { Deny 'wo9_native_git_head_readback_failed' }
        if ($gitHeadBefore -ne $after) { break }
        Start-Sleep -Milliseconds 400
      }
      if ($gitHeadBefore -eq $after) {
        $staged=@(git -C $fixed diff --cached --name-only)
        Write-Host ('WO9_NATIVE_COMMIT_OBSERVATION=unchanged_head;staged_files_'+$staged.Count)
        foreach($category in @(@('Native Git commit not verified','native_result_unverified'),
          @('Native Git commit cancelled','native_refusal'),
          @('Awaiting native confirmation for the exact staged Git file','awaiting_native_confirmation'))) {
          try {if (Find-Control $category[0] '' 2) {Write-Host ('WO9_NATIVE_COMMIT_STAGE='+$category[1])}} catch {}
        }
        Deny 'wo9_native_commit_not_observed'
      }
      $files=@(git -C $fixed diff-tree --no-commit-id --name-only -r HEAD)
      if ($files.Count -ne 1 -or $files[0] -ne 'main.rs') {
        Deny 'wo9_git_commit_changed_unreviewed_files'
      }
      $parent=(git -C $fixed rev-parse 'HEAD^').Trim()
      if ($LASTEXITCODE -ne 0 -or $parent -cne $gitHeadBefore) {Deny 'wo9_native_commit_parent_mismatch'}
      $committedBlob=(git -C $fixed rev-parse 'HEAD:main.rs').Trim()
      if ($LASTEXITCODE -ne 0 -or $committedBlob -cne $expectedStageBlob) {Deny 'wo9_native_commit_blob_mismatch'}
      $remaining=@(git -C $fixed diff --cached --name-only --)
      if ($LASTEXITCODE -ne 0 -or $remaining.Count -ne 0) {Deny 'wo9_native_index_not_clean_after_commit'}
      [void](Find-Control ('Local commit: '+$after) '' 15 -Exact)
      Write-Host 'WO9_INSTALLED_ONE_FILE_GIT_STAGE_COMMIT_NO_HOOKS_OR_PUSH=PASS'
    }
    if ($Scenario -eq 'Dev035JobsReadOnly') {
      # Installed unsigned application: only read-only Jobs UX can pass.
      # Requiring a genuine signed protected worker is a DIFFERENT gate.
      $before=ProtectedWorkerCount
      $sourceBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed 'main.rs')).Hash
      Click 'Job requests'
      Require 'DEV-03.4 · Conventional test files'
      Require 'INT05-C · Protected terminal transcript'
      Require 'No current project-owned native terminal'
      Require 'DEV-03.5 · Protected job lifecycle'
      Require 'No matching protected job journal entries observed'
      Click 'Discover test files'
      [void](Find-Control 'discovery incomplete by design' '' 20)
      $start=Find-Control 'Request protected Build/Test' 'Button' 15
      if ($start.Current.IsEnabled) {
        Deny 'unsigned_installed_job_unexpectedly_executable'
      }
      if ((ProtectedWorkerCount) -ne $before) {
        Deny 'read_only_test_discovery_spawned_a_worker'
      }
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed 'main.rs')).Hash -ne $sourceBefore) {
        Deny 'read_only_jobs_workflow_modified_source'
      }
      Write-Host 'DEV035_WINDOWS_INSTALLED_JOBS_DISCOVERY_NO_RUN=PASS'
      Write-Host 'DEV035_WINDOWS_INSTALLED_PROTECTED_WORKER=NOT_ACCEPTED'
    }
    if ($Scenario -eq 'Dev045CInstalledProjectDebugger') {
      # Separate from the WO8 fixed-debuggee self-test. This installed
      # scenario must run against a reviewed, signed feature-enabled build,
      # with a pinned Cargo bundle, pinned project LLDB bundle and a genuine
      # SCM recovery service. No test IPC or alternate execution entrypoint.
      $signed=Get-AuthenticodeSignature -LiteralPath $exe
      if ($signed.Status -ne 'Valid' -or !$signed.SignerCertificate) {
        Deny 'dev045c_installed_desktop_not_signed'
      }
      [void](VerifiedInstalledService)
      $source=Join-Path $fixed 'main.rs'
      $sourceBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash
      $cargoSource=Join-Path $fixed 'src\main.rs'
      if (!(Test-Path -LiteralPath $cargoSource -PathType Leaf)) {
        Deny 'dev045c_real_cargo_project_source_missing'
      }
      $cargoBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath $cargoSource).Hash
      $workersBefore=ProtectedWorkerCount
      Click 'Job requests'
      Require 'Protected Run / Test'
      $recipe=Find-Control 'Recipe' 'ComboBox' 25
      if (!$recipe.Current.IsKeyboardFocusable) {
        Deny 'dev045c_native_cargo_recipe_selector_unavailable'
      }
      $recipe.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{ENTER}')
      $build=Find-Control 'Request protected Run / Test' 'Button' 25
      if (!$build.Current.IsEnabled) {
        Deny 'dev045c_installed_protected_cargo_unavailable'
      }
      Click 'Request protected Run / Test'
      Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'
      [void](Find-Control 'Last worker result:' '' 360)
      Require 'Native exit: 0'
      Require 'Reconciliation: not indicated'
      Require 'Worker output seal:'
      $target=Find-Control 'Debug last verified project build' 'Button' 35
      if (!$target.Current.IsEnabled) {
        Deny 'dev045c_verified_project_binary_not_ready_for_debug'
      }
      Click 'Debug last verified project build'
      # This is a SECOND, wholly separate actual OS dialog. The preceding
      # Cargo consent is never an LLDB grant and may not be replayed.
      Confirm-NativeOneUseDialog 'Approve one protected project debugger' 'Debug this exact project build'
      [void](Find-Control 'Protected project LLDB session' '' 240)
      Require 'Originating protected build:'
      Require 'Exact build terminal receipt:'
      Require 'Project debugger D3 target binding:'
      Require 'Thread inspection: observed'
      Require 'Stack: observed'
      Require 'Kernel tree cleanup: reported by protected supervisor'
      Require 'Installed/debugger project release acceptance:'
      [void](Find-Control 'Durable debugger jobs' '' 45)
      Wait-WorkerReturnToBaseline $workersBefore
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash -ne $sourceBefore -or
          (Get-FileHash -Algorithm SHA256 -LiteralPath $cargoSource).Hash -ne $cargoBefore) {
        Deny 'dev045c_project_debugger_mutated_original_sources'
      }
      Write-Host 'DEV045C_SIGNED_INSTALLED_REAL_CARGO_TO_PROJECT_LLDB_UI=PASS'
      Write-Host 'DEV045C_TWO_ACTUAL_NATIVE_ONE_USE_CONSENTS=PASS'
      Write-Host 'DEV045C_REAL_PROJECT_STOP_STACK_AND_WORKER_REAP=PASS'
      Write-Host 'DEV045C_CRASH_AND_PROJECT_DEBUG_CANCEL=NOT_TESTED'
    }
    if ($Scenario -eq 'Dev045CInstalledProjectRestartDenied') {
      # Second, separately launched actual installed desktop. The build
      # runtime's most recent sealed executable was process-private; a
      # journal record cannot reauthorize it after a restart.
      [void](VerifiedInstalledService)
      $before=ProtectedWorkerCount
      Click 'Job requests'
      Require 'Durable debugger jobs'
      $replay=Find-Control 'Debug last verified project build' 'Button' 35
      if ($replay.Current.IsEnabled) {
        Deny 'dev045c_stale_project_debug_target_reused_after_restart'
      }
      if ((ProtectedWorkerCount) -ne $before) {
        Deny 'dev045c_reopen_spontaneously_started_debugger'
      }
      Write-Host 'DEV045C_INSTALLED_RESTART_NO_AUTOREPLAY_OF_PROJECT_DEBUGGER=PASS'
    }
    if ($Scenario -eq 'Dev045ProjectDebuggerDenied') {
      # Exact default unsigned installed application: prove the native OS
      # exposes a disabled project-debug action, not a simulated debugger.
      # No worker, SCM service, project source mutation or grant may occur.
      $workerBefore=ProtectedWorkerCount
      $sourceBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed 'main.rs')).Hash
      if (Get-Service LuIdeRecoveryBroker -ErrorAction SilentlyContinue) {
        Deny 'default_installer_unexpectedly_has_debugger_service'
      }
      Click 'Job requests'
      # Current installed UI exposes the release entitlement denial, followed
      # by the actual disabled action. Do not accept a legacy prose label or
      # substitute for the independently inspected disabled native control.
      Require 'protected native release entitlement'
      $debug=Find-Control 'Debug last verified project build' 'Button' 20
      if ($debug.Current.IsEnabled) {
        Deny 'default_unsigned_installer_exposed_project_execution'
      }
      if ((ProtectedWorkerCount) -ne $workerBefore) {
        Deny 'default_installed_project_debug_review_spawned_process'
      }
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed 'main.rs')).Hash -ne $sourceBefore) {
        Deny 'default_installed_project_debug_review_modified_source'
      }
      Write-Host 'DEV045_DEFAULT_INSTALLED_NATIVE_DEBUGGER_DENIED=PASS'
      Write-Host 'DEV045_SIGNED_PINNED_PROJECT_DEBUGGER=NOT_ACCEPTED'
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
    if ($Scenario -eq 'Dev02ProtectedLsp') {
      # This lane runs only against an actual installed protected release
      # with verified provider bundle and the independently installed SCM.
      # Never turn on LSP through JavaScript, mocked IPC, PATH clangd or
      # an uncontained same-user child as a substitute for UI consent.
      [void](VerifiedInstalledService)
      $files=@('main.rs','main.cpp','hello.py','Cargo.toml')
      foreach ($name in $files) {
        if (!(Test-Path -LiteralPath (Join-Path $fixed $name) -PathType Leaf)) {
          Deny ('dev02_real_installed_fixture_missing_'+$name.Replace('.','_'))
        }
      }
      function Native-LspPositionAtCall {
        $editor=Find-Control 'Source editor text' '' 15
        if (!$editor.Current.IsKeyboardFocusable) { Deny 'dev02_editor_not_focusable' }
        $editor.SetFocus()
        [System.Windows.Forms.SendKeys]::SendWait('^{HOME}')
        [System.Windows.Forms.SendKeys]::SendWait('{DOWN}{DOWN}{END}{LEFT 5}')
        Start-Sleep -Milliseconds 500
        return $editor
      }
      function Observe-RealLspWorker([string]$name,[int]$before) {
        $deadline=(Get-Date).AddSeconds(24)
        while ((Get-Date) -lt $deadline) {
          if ((BoundedProcess $name) -gt $before) { return }
          Start-Sleep -Milliseconds 350
        }
        Deny ('dev02_native_protected_lsp_process_not_observed_'+$name)
      }
      function Require-LspReaped([string]$name,[int]$before) {
        $deadline=(Get-Date).AddSeconds(20)
        while ((Get-Date) -lt $deadline) {
          if ((BoundedProcess $name) -le $before) { return }
          Start-Sleep -Milliseconds 350
        }
        Deny ('dev02_protected_lsp_worker_not_reaped_'+$name)
      }
      function Test-RealInstalledSemanticProvider(
        [string]$language,[string]$file,[string]$server
      ) {
        $prior=BoundedProcess $server
        Click $file
        Require ('Start '+$server)
        Click ('Start '+$server)
        Confirm-NativeOneUseDialog 'Allow language analysis' 'Allow this session'
        Observe-RealLspWorker $server $prior
        Require ('Protected '+$server+' session started')
        Click 'Problems'
        [void](Find-Control 'Language server problems' '' 30)
        $editor=Native-LspPositionAtCall
        try {
          $pattern=$editor.GetCurrentPattern(
            [System.Windows.Automation.TextPattern]::Pattern)
          $ranges=$pattern.GetSelection()
          if (!$ranges -or $ranges.Count -ne 1) { Deny 'dev02_real_caret_missing' }
          $before=$ranges[0].Clone()
        } catch { Deny 'dev02_real_editor_textpattern_not_available' }
        Click 'Hover'
        [void](Find-Control 'Symbol information' '' 25)
        Click 'Complete'
        [void](Find-Control 'Suggestions' '' 25)
        Click 'Definition'
        Start-Sleep -Milliseconds 900
        $afterEditor=Find-Control 'Source editor text' '' 12
        try {
          $afterPattern=$afterEditor.GetCurrentPattern(
            [System.Windows.Automation.TextPattern]::Pattern)
          $afterRanges=$afterPattern.GetSelection()
          if (!$afterRanges -or $afterRanges.Count -ne 1 -or
              $before.Compare($afterRanges[0])) {
            Deny 'dev02_same_file_definition_did_not_move_real_caret'
          }
        } catch {
          if ($_.Exception.Message -like 'WO8_NATIVE_UI_DENIED:*') { throw }
          Deny 'dev02_native_definition_caret_unverifiable'
        }
        Write-Host ('DEV02_INSTALLED_WINDOWS_'+$language.ToUpperInvariant()+
          '_DIAGNOSTICS_COMPLETION_HOVER_DEFINITION=PASS')
        Click ('Stop '+$server)
        Require 'Language-server session stopped'
        Require-LspReaped $server $prior
      }
      Test-RealInstalledSemanticProvider 'rust' 'main.rs' 'rust-analyzer'
      Test-RealInstalledSemanticProvider 'cpp' 'main.cpp' 'clangd'

      $priorRuff=BoundedProcess 'ruff'
      Click 'hello.py'
      Require 'Ruff · diagnostics only'
      Click 'Start Ruff'
      Confirm-NativeOneUseDialog 'Allow language analysis' 'Allow this session'
      Observe-RealLspWorker 'ruff' $priorRuff
      Require 'Protected Ruff session started'
      Click 'Problems'
      [void](Find-Control 'Language server problems' '' 30)
      [void](Find-Control 'unused' '' 25)
      foreach ($action in @('Complete','Hover','Definition')) {
        $control=Find-Control $action 'Button'
        if ($control.Current.IsEnabled) {
          Deny ('dev02_unreviewed_python_semantic_command_enabled_'+$action)
        }
      }
      Click 'Stop Ruff'
      Require 'Language-server session stopped'
      Require-LspReaped 'ruff' $priorRuff
      Write-Host 'DEV02_INSTALLED_WINDOWS_PYTHON_RUFF_PROTECTED_DIAGNOSTICS=PASS'
      Write-Host 'DEV02_INSTALLED_WINDOWS_ALL_THREE_PROTECTED_PROVIDERS=PASS'
    }
    if ($Scenario -eq 'B5LargeProject') {
      # Each number below is measured against the genuine installed
      # WebView2 window/OS clipboard and Windows kernel process tree.
      # A successful source-only test is NOT accepted as installed evidence.
      if (!$MetricsFile) { Deny 'b5_missing_installed_evidence_destination' }
      $dest=[IO.Path]::GetFullPath($MetricsFile)
      $tempPrefix=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
      if (!$dest.StartsWith($tempPrefix,[StringComparison]::OrdinalIgnoreCase)) {
        Deny 'b5_evidence_outside_disposable_runner'
      }
      $sources=@(0..46 | ForEach-Object { 'p{0:D2}.rs' -f $_ })
      $main=Join-Path $fixed 'main.rs'
      $extra=Join-Path $fixed 'p47.rs'
      if (!(Test-Path -LiteralPath $extra -PathType Leaf) -or
          (Get-Item -LiteralPath $main).Length -lt (130*1024) -or
          (Get-Item -LiteralPath $main).Length -gt (512*1024)) {
        Deny 'b5_missing_or_unbounded_large_file_fixture'
      }
      $hashes=@{}
      foreach($leaf in @('main.rs')+$sources+@('p47.rs')) {
        $file=Join-Path $fixed $leaf
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) {
          Deny ('b5_missing_native_source_'+$leaf)
        }
        $hashes[$leaf]=(Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash
      }
      if ($env:B5_EXACT_CHECKOUT_SHA -notmatch '^[a-f0-9]{40}$' -or
          (git rev-parse HEAD).Trim() -cne $env:B5_EXACT_CHECKOUT_SHA) {
        Deny 'b5_performance_source_commit_not_exact_checkout'
      }
      $data=[ordered]@{
        schemaVersion=1; os='windows-2022';
        method='installed-WebView2-UIAutomation-STA-clipboard-and-Win32_Process';
        clipboardWitness='bounded-fresh-head-or-tail-v2'
        sourceCommit=$env:B5_EXACT_CHECKOUT_SHA; installedExecutableSha256=$ExpectedSha256
        sourceBytes=(Get-Item -LiteralPath $main).Length
        fileCountToOpen=48; openRoundTripMs=@(); switchRoundTripMs=@()
        memorySamples=@(); keyboardRoundTripMs=$null
        undoRoundTripMs=$null; saveRoundTripMs=$null
        explicitSaveVerified=$false; bounded49thDenial=$false
        noImplicitSaveVerified=$false
      }
      function B5-Memory([string]$label) {
        # Include installed lu-ide-desktop.exe and currently observed child
        # WebView2 renderer processes only; don't mislabel this global OS RSS.
        $native=@(Get-CimInstance Win32_Process -ErrorAction Stop)
        $family=[System.Collections.Generic.HashSet[int]]::new()
        [void]$family.Add([int]$proc.Id)
        for($pass=0;$pass -lt 12;$pass++) {
          $before=$family.Count
          foreach($node in $native) {
            if($family.Contains([int]$node.ParentProcessId)) {
              [void]$family.Add([int]$node.ProcessId)
            }
          }
          if($family.Count -gt 128) {Deny 'b5_unbounded_descendant_process_count'}
          if($family.Count -eq $before) {break}
        }
        [long]$working=0
        [long]$private=0
        $count=0
        foreach($procId in $family) {
          $observed=Get-Process -Id $procId -ErrorAction SilentlyContinue
          if ($null -eq $observed) { continue }
          $working+=[long]$observed.WorkingSet64
          $private+=[long]$observed.PrivateMemorySize64
          $count++
        }
        if($working -le 0 -or $count -lt 1) {Deny 'b5_process_memory_unavailable'}
        return [ordered]@{
          stage=$label; rssBytes=$working; privateBytes=$private
          observedProcessCount=$count
          scope='installed-pid-and-live-Win32_Process-descendants'
        }
      }
      function B5-FreshEditorCopy([string]$leaf,[bool]$tail=$false) {
        $expected='Source editor text · First editor: '+$leaf
        Click 'Focus primary editor (Ctrl+1)'
        # WebView2 can expose a contenteditable textbox as Edit or Document.
        # Bind instead to the precise pane identity and actual keyboard focus.
        $editor=Focus-ExactNativeControl $expected
        $hwnd=$proc.MainWindowHandle
        $actual=[System.Windows.Automation.AutomationElement]::FocusedElement
        if ($hwnd -eq [IntPtr]::Zero -or
            [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd -or
            !$editor.Current.IsKeyboardFocusable -or
            !($editor.Current.HasKeyboardFocus -or ($actual -and $actual.Equals($editor)))) {
          Deny 'b5_native_editor_focus_not_verified'
        }
        # Reset and independently observe a fresh native OS clipboard copy.
        # Navigation snapshots only the first line; append/Undo snapshots the
        # last four lines. Whole-document transfer masks editor latency.
        [System.Windows.Forms.Clipboard]::Clear()
        if ([System.Windows.Forms.Clipboard]::ContainsText()) {Deny 'b5_native_clipboard_reset_failed'}
        if ($tail) {
          [System.Windows.Forms.SendKeys]::SendWait('^{END}+{UP}+{UP}+{UP}+{UP}+{HOME}')
        } else {
          [System.Windows.Forms.SendKeys]::SendWait('^{HOME}+{END}')
        }
        [System.Windows.Forms.SendKeys]::SendWait('^c')
        $until=(Get-Date).AddSeconds(2)
        do {
          Start-Sleep -Milliseconds 100
          if ([System.Windows.Forms.Clipboard]::ContainsText()) {
            $value=[System.Windows.Forms.Clipboard]::GetText()
            if ($value.Length -gt 0 -and $value.Length -le 8192) {return $value}
          }
        } while ((Get-Date) -lt $until)
        return ''
      }
      function B5-VisibleEditor([string]$marker) {
        $leaf=if ($marker -cmatch '^B5_SOURCE_(main|p[0-4][0-9])$') { $Matches[1]+'.rs' } else { 'main.rs' }
        $tail=$marker -ceq 'B5_EXPLICIT_SAVE_PERF_ONLY'
        $end=(Get-Date).AddSeconds(18)
        $attempts=0
        $lastLen=0
        $anySourceMarker=$false
        while((Get-Date) -lt $end) {
          $value=B5-FreshEditorCopy $leaf $tail
          $attempts++
          $lastLen=$value.Length
          $anySourceMarker=$anySourceMarker -or $value.Contains('B5_SOURCE_')
          if ($value.Contains($marker) -and (!$tail -or $value.Contains('B5_SOURCE_main'))) {return}
          Start-Sleep -Milliseconds 190
        }
        # Diagnostic only: never log source text, path or clipboard data.
        Write-Host ('B5_NATIVE_EDITOR_COPY_DIAGNOSTIC=attempts_'+$attempts+
          ';last_clipboard_len_'+$lastLen+
          ';some_b5_marker_seen_'+$anySourceMarker)
        Deny ('b5_native_installed_editor_not_visible_for_'+$marker)
      }
      function B5-OpenRelative([string]$leaf,[string]$marker) {
        $input=Focus-ExactNativeControl 'Open project-relative file' 'Edit'
        $timer=[Diagnostics.Stopwatch]::StartNew()
        [System.Windows.Forms.SendKeys]::SendWait('^a')
        [System.Windows.Forms.SendKeys]::SendWait($leaf)
        $input=Focus-ExactNativeControl 'Open project-relative file' 'Edit'
        if ($input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne $leaf) {
          Deny 'b5_native_relative_file_input_readback_mismatch'
        }
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        B5-VisibleEditor $marker
        $timer.Stop()
        return [Math]::Round($timer.Elapsed.TotalMilliseconds,3)
      }
      function B5-Switch([string]$which,[string]$marker) {
        $picker=Focus-ExactNativeControl 'Jump to open file'
        $timer=[Diagnostics.Stopwatch]::StartNew()
        if ($which -eq 'first') {
          [System.Windows.Forms.SendKeys]::SendWait('{HOME}')
        } else {
          [System.Windows.Forms.SendKeys]::SendWait('{END}')
        }
        B5-VisibleEditor $marker
        $timer.Stop()
        return [Math]::Round($timer.Elapsed.TotalMilliseconds,3)
      }
      function B5-P95($numbers) {
        $sorted=@($numbers | Sort-Object)
        if ($sorted.Count -eq 0) {Deny 'b5_missing_timing_samples'}
        return $sorted[[Math]::Floor(($sorted.Count-1)*.95)]
      }
      B5-VisibleEditor 'B5_SOURCE_main'
      $data.memorySamples+=,(B5-Memory 'opened_large_main')
      for($i=0;$i -lt $sources.Count;$i++) {
        $leaf=$sources[$i]
        $marker='B5_SOURCE_'+[IO.Path]::GetFileNameWithoutExtension($leaf)
        $data.openRoundTripMs+=,(B5-OpenRelative $leaf $marker)
        if(($i+2) -in @(12,24,36,48)) {
          $data.memorySamples+=,(B5-Memory ('opened_'+($i+2)+'_files'))
        }
      }
      # Explicitly attempt a 49th tab. No implicit eviction of unsaved history.
      Require '48 / 48 tabs in this project'
      $input=Focus-ExactNativeControl 'Open project-relative file' 'Edit'
      [System.Windows.Forms.SendKeys]::SendWait('^a')
      [System.Windows.Forms.SendKeys]::SendWait('p47.rs')
      $input=Focus-ExactNativeControl 'Open project-relative file' 'Edit'
      if ($input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne 'p47.rs') {
        Deny 'b5_native_49th_input_readback_mismatch'
      }
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      Require 'Open-file budget'
      Require '48 / 48 tabs in this project'
      $data.bounded49thDenial=$true
      foreach($leaf in $hashes.Keys) {
        if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed $leaf)).Hash -ne
           $hashes[$leaf]) {Deny 'b5_unsaved_tab_open_changed_source'}
      }
      $data.noImplicitSaveVerified=$true
      for($n=0;$n -lt 6;$n++) {
        $data.switchRoundTripMs+=,(B5-Switch 'first' 'B5_SOURCE_main')
        $data.switchRoundTripMs+=,(B5-Switch 'last' 'B5_SOURCE_p46')
      }
      [void](B5-Switch 'first' 'B5_SOURCE_main')
      $editor=Focus-ExactNativeControl 'Source editor text · First editor: main.rs'
      [System.Windows.Forms.SendKeys]::SendWait('^{END}{ENTER}')
      $clock=[Diagnostics.Stopwatch]::StartNew()
      [System.Windows.Forms.SendKeys]::SendWait('// B5_EXPLICIT_SAVE_PERF_ONLY')
      B5-VisibleEditor 'B5_EXPLICIT_SAVE_PERF_ONLY'
      $data.keyboardRoundTripMs=[Math]::Round($clock.Elapsed.TotalMilliseconds,3)
      if((Get-FileHash -Algorithm SHA256 -LiteralPath $main).Hash -ne $hashes['main.rs']) {
        Deny 'b5_typing_implicitly_wrote_native_source'
      }
      $editor=Focus-ExactNativeControl 'Source editor text · First editor: main.rs'
      $clock=[Diagnostics.Stopwatch]::StartNew()
      [System.Windows.Forms.SendKeys]::SendWait('^z')
      $until=(Get-Date).AddSeconds(10)
      $removed=$false
      while((Get-Date) -lt $until) {
        $observed=B5-FreshEditorCopy 'main.rs' $true
        if($observed.Contains('B5_SOURCE_main') -and
           !$observed.Contains('B5_EXPLICIT_SAVE_PERF_ONLY')) {
          $removed=$true
          break
        }
      }
      if(!$removed) {Deny 'b5_native_undo_not_verified'}
      $data.undoRoundTripMs=[Math]::Round($clock.Elapsed.TotalMilliseconds,3)
      $editor=Focus-ExactNativeControl 'Source editor text · First editor: main.rs'
      [System.Windows.Forms.SendKeys]::SendWait('^{END}{ENTER}// B5_EXPLICIT_SAVE_PERF_ONLY')
      $clock=[Diagnostics.Stopwatch]::StartNew()
      Click 'Save · Ctrl+S'
      $until=(Get-Date).AddSeconds(20)
      $saved=$false
      while((Get-Date) -lt $until) {
        if([IO.File]::ReadAllText($main).Contains('B5_EXPLICIT_SAVE_PERF_ONLY')){
          $saved=$true; break
        }
        Start-Sleep -Milliseconds 180
      }
      if(!$saved) {Deny 'b5_native_explicit_save_never_completed'}
      $data.saveRoundTripMs=[Math]::Round($clock.Elapsed.TotalMilliseconds,3)
      foreach($leaf in $sources+@('p47.rs')) {
        if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $fixed $leaf)).Hash -ne
           $hashes[$leaf]) {Deny 'b5_native_primary_save_changed_other_source'}
      }
      $data.explicitSaveVerified=$true
      $data.memorySamples+=,(B5-Memory 'after_explicit_save')
      if($data.openRoundTripMs.Count -ne 47 -or
         $data.switchRoundTripMs.Count -ne 12 -or
         $data.memorySamples.Count -lt 6) {Deny 'b5_incomplete_installed_ui_metrics'}
      $data['openP95Ms']=B5-P95 $data.openRoundTripMs
      $data['switchP95Ms']=B5-P95 $data.switchRoundTripMs
      $peaks=Get-B5MemoryPeaks $data.memorySamples
      $data['rssPeakObservedBytes']=$peaks['rssPeakObservedBytes']
      $data['privatePeakObservedBytes']=$peaks['privatePeakObservedBytes']
      $data | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $dest -Encoding UTF8
      if(!(Test-Path -LiteralPath $dest) -or (Get-Item -LiteralPath $dest).Length -lt 500) {
        Deny 'b5_actual_os_evidence_not_recorded'
      }
      Write-Host ('B5_WINDOWS_INSTALLED_OPEN_P95_ROUNDTRIP_MS='+$data.openP95Ms)
      Write-Host ('B5_WINDOWS_INSTALLED_SWITCH_P95_ROUNDTRIP_MS='+$data.switchP95Ms)
      Write-Host ('B5_WINDOWS_INSTALLED_RSS_PEAK_OBSERVED_BYTES='+$data.rssPeakObservedBytes)
      Write-Host 'B5_WINDOWS_48_REAL_EDITOR_TABS_OS_PROCESS_MEMORY=MEASURED_NOT_APPROVED'
    }
    if ($Scenario -eq 'Dev01Modern') {
      # Exact installed UIA/OS controls on the consolidated source, not the
      # original unsupported DEV-01 page selectors or Tauri/JS shortcuts.
      if (@(Get-ChildItem -LiteralPath $fixed -File -Recurse -Filter '*.rs').Count -ne 867) {
        Deny 'dev01_not_original_867_file_fixture'
      }
      function Dev01-Input([string]$label,[string]$value) {
        # WebView2 focus/scroll changes are asynchronous and can invalidate
        # a previously returned UIA element. Reacquire the exact live field.
        $hwnd=$proc.MainWindowHandle
        if ($hwnd -eq [IntPtr]::Zero) { Deny 'dev01_native_input_window_missing' }
        $focused=$false
        for ($attempt=0; $attempt -lt 3 -and !$focused; $attempt++) {
          [void][P04NativeWebViewWindows]::SetForegroundWindow($hwnd)
          $item=Find-Control $label 'Edit' 8 -Exact
          if (!$item.Current.IsEnabled -or !$item.Current.IsKeyboardFocusable) { Deny 'dev01_native_text_input_not_focusable' }
          try {
            Reveal-NativeControl $item
            $item=Find-Control $label 'Edit' 5 -Exact
            $item.SetFocus()
          } catch {}
          Start-Sleep -Milliseconds 300
          $item=Find-Control $label 'Edit' 5 -Exact
          if ($item.Current.IsOffscreen -or !$item.Current.IsEnabled -or
              $item.Current.Name -cne $label -or
              [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) { continue }
          if (!$item.Current.HasKeyboardFocus -and $attempt -gt 0) {
            # Real mouse input only after the native UIA hit-test proves the
            # exact field (or its descendant), inside the installed window.
            try {
              $rect=$item.Current.BoundingRectangle
              $bounds=(WindowRoot).Current.BoundingRectangle
              if ($rect.Width -lt 5 -or $rect.Height -lt 5) { continue }
              $point=[System.Windows.Point]::new([Math]::Round($rect.Left+$rect.Width/2),[Math]::Round($rect.Top+$rect.Height/2))
              if (!$bounds.Contains($point) -or !$rect.Contains($point)) { continue }
              $hit=[System.Windows.Automation.AutomationElement]::FromPoint($point)
              $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
              $exactHit=$false
              for ($depth=0; $hit -and $depth -lt 8; $depth++) {
                if ($hit.Equals($item)) { $exactHit=$true; break }
                $hit=$walker.GetParent($hit)
              }
              if (!$exactHit -or [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) { continue }
              [System.Windows.Forms.Cursor]::Position=[System.Drawing.Point]::new([int]$point.X,[int]$point.Y)
              [P04NativeWebViewWindows]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
              [P04NativeWebViewWindows]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
            } catch {}
          }
          $deadline=(Get-Date).AddSeconds(4)
          do {
            $item=Find-Control $label 'Edit' 2 -Exact
            $actual=[System.Windows.Automation.AutomationElement]::FocusedElement
            if ($item.Current.IsEnabled -and !$item.Current.IsOffscreen -and
                $item.Current.Name -ceq $label -and
                [P04NativeWebViewWindows]::GetForegroundWindow() -eq $hwnd -and
                ($item.Current.HasKeyboardFocus -or ($actual -and $actual.Equals($item)))) {
              $focused=$true; break
            }
            Start-Sleep -Milliseconds 150
          } while ((Get-Date) -lt $deadline)
        }
        if (!$focused -or [P04NativeWebViewWindows]::GetForegroundWindow() -ne $hwnd) {
          Deny 'dev01_native_text_input_focus_not_verified'
        }
        [System.Windows.Forms.SendKeys]::SendWait('^a')
        [System.Windows.Forms.SendKeys]::SendWait($value)
        $deadline=(Get-Date).AddSeconds(8)
        do {
          $fresh=Find-Control $label 'Edit' 2 -Exact
          if ($fresh.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ceq $value) {
            Write-Host ('DEV01_NATIVE_TEXT_INPUT_READBACK=verified;field='+$label)
            return
          }
          Start-Sleep -Milliseconds 200
        } while ((Get-Date) -lt $deadline)
        Deny 'dev01_native_text_input_readback_mismatch'
      }
      function Dev01-Confirm([string]$message) {
        # Real owned OS dialog, never an inaccessible WebView script alert.
        Confirm-NativeOneUseDialog 'Confirm source file action' 'Apply file action' $message
      }
      Dev01-Input 'Quick Open filename search' 'src-23/file-035.rs'
      Click 'Find files'
      Require 'Quick Open filename results'
      Require 'src-23/file-035.rs'
      Write-Host 'DEV01_MODERN_WINDOWS_QUICK_OPEN_OS_UIA=PASS'
      # Real native keyboard focus and activation, not a synthetic result open.
      function Dev01-WaitKeyboardFocus([string]$label, [string]$kind) {
        $limit=(Get-Date).AddSeconds(12)
        do {
          try {
            $item=Find-Control $label $kind 2 -Exact
            $actual=[System.Windows.Automation.AutomationElement]::FocusedElement
            if ($item.Current.Name -ceq $label -and $item.Current.IsEnabled -and
                !$item.Current.IsOffscreen -and
                [P04NativeWebViewWindows]::GetForegroundWindow() -eq $proc.MainWindowHandle -and
                ($item.Current.HasKeyboardFocus -or ($actual -and $actual.Equals($item)))) { return }
          } catch {}
          Start-Sleep -Milliseconds 150
        } while ((Get-Date) -lt $limit)
        Deny 'dev01_quick_open_native_keyboard_focus_missing'
      }
      [System.Windows.Forms.SendKeys]::SendWait('^p')
      Dev01-WaitKeyboardFocus 'Quick Open filename search' 'Edit'
      Dev01-Input 'Quick Open filename search' 'src-23/file-03'
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      $firstResult='src-23/file-030.rs'
      $secondResult='src-23/file-031.rs'
      Require $firstResult
      Require $secondResult
      $quickOpenPath=Join-Path $fixed $firstResult
      $quickOpenSha=(Get-FileHash -LiteralPath $quickOpenPath -Algorithm SHA256).Hash
      [System.Windows.Forms.SendKeys]::SendWait('{DOWN}')
      Dev01-WaitKeyboardFocus $firstResult 'Button'
      [System.Windows.Forms.SendKeys]::SendWait('{DOWN}')
      Dev01-WaitKeyboardFocus $secondResult 'Button'
      [System.Windows.Forms.SendKeys]::SendWait('{UP}')
      Dev01-WaitKeyboardFocus $firstResult 'Button'
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      Require ('Saved source tab '+$firstResult)
      if ((Get-FileHash -LiteralPath $quickOpenPath -Algorithm SHA256).Hash -ne $quickOpenSha) {
        Deny 'dev01_quick_open_keyboard_changed_source'
      }
      Click ('Close '+$firstResult)
      Click 'main.rs'
      Write-Host 'DEV01_MODERN_WINDOWS_QUICK_OPEN_KEYBOARD=PASS'
      $searchFile=Join-Path $fixed 'r4r4-search.rs'
      $createdDir=Join-Path $fixed 'r4r4-created'
      if ((Test-Path -LiteralPath $searchFile) -or (Test-Path -LiteralPath $createdDir)) { Deny 'non_pristine_residual_editor_fixture' }
      Click 'New folder'
      Dev01-Input 'Source action path or new name' 'r4r4-created'
      Click 'Review and apply'
      Require 'Source action completed: new folder'
      if (!(Test-Path -LiteralPath $createdDir -PathType Container) -or (Get-Item -LiteralPath $createdDir).LinkType) { Deny 'native_folder_creation_not_observed' }
      Click 'New file'
      Dev01-Input 'Source action path or new name' 'r4r4-search.rs'
      Click 'Review and apply'
      Require 'Source action completed: new file'
      if (!(Test-Path -LiteralPath $searchFile -PathType Leaf) -or (Get-Item -LiteralPath $searchFile).LinkType -or (Get-Item -LiteralPath $searchFile).Length -ne 0) { Deny 'native_created_file_not_empty' }
      [IO.File]::WriteAllText($searchFile,"// IDE_R4R4_SEARCH_ORIGINAL`n")
      $searchBefore=(Get-FileHash -LiteralPath $searchFile -Algorithm SHA256).Hash
      Click 'New file'
      Dev01-Input 'Source action path or new name' 'R4R4-SEARCH.rs'
      Click 'Review and apply'
      Require 'SOURCE_TARGET_ALREADY_EXISTS'
      if ((Get-FileHash -LiteralPath $searchFile -Algorithm SHA256).Hash -cne $searchBefore) { Deny 'case_alias_creation_changed_source' }
      Click 'Cancel'
      Write-Host 'DEV01_MODERN_WINDOWS_CREATE_FILE_FOLDER_NO_CLOBBER=PASS'
      Click 'Find in project'
      Dev01-Input 'Search project text' 'IDE_R4R4_SEARCH_ORIGINAL'
      Click 'Search project'
      $hit='r4r4-search.rs:1:4'
      [void](Find-Control $hit 'Button' 30)
      [IO.File]::WriteAllText($searchFile,"// IDE_R4R4_SEARCH_ORIGINAL_CHANGED`n")
      $searchChanged=(Get-FileHash -LiteralPath $searchFile -Algorithm SHA256).Hash
      if ($searchChanged -ceq $searchBefore) { Deny 'external_search_fixture_revision_not_changed' }
      Click $hit
      Require 'FILE_CHANGED_ON_DISK: search again'
      if ((Get-FileHash -LiteralPath $searchFile -Algorithm SHA256).Hash -cne $searchChanged) { Deny 'stale_search_changed_external_bytes' }
      Click 'Search project'
      [void](Find-Control $hit 'Button' 30)
      Click $hit
      Require 'Saved source tab r4r4-search.rs'
      if ((Get-FileHash -LiteralPath $searchFile -Algorithm SHA256).Hash -cne $searchChanged) { Deny 'fresh_search_changed_external_bytes' }
      Click 'Close r4r4-search.rs'
      Click 'main.rs'
      Write-Host 'DEV01_MODERN_WINDOWS_STALE_SEARCH_DENIED_FRESH_NAVIGATION=PASS'
      $original=Join-Path $fixed 'move-me.rs'
      $renamed=Join-Path $fixed 'renamed.rs'
      if (!(Test-Path $original) -or (Test-Path $renamed)) { Deny 'dev01_bad_source_fixture' }
      $originalSha=(Get-FileHash $original -Algorithm SHA256).Hash
      Click 'Move file'
      Dev01-Input 'Source action path or new name' 'move-me.rs'
      Dev01-Input 'Project-relative move destination' 'renamed.rs'
      Click 'Review and apply'
      Dev01-Confirm 'Move move-me.rs to renamed.rs? No existing file will be overwritten.'
      $limit=(Get-Date).AddSeconds(25)
      while (!(Test-Path $renamed) -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 400 }
      if (!(Test-Path $renamed) -or (Test-Path $original) -or
          (Get-FileHash $renamed -Algorithm SHA256).Hash -ne $originalSha) {
        Deny 'dev01_native_move_bytes_changed'
      }
      Click 'Trash file'
      Dev01-Input 'Source action path or new name' 'renamed.rs'
      Click 'Review and apply'
      Dev01-Confirm 'Move renamed.rs to recoverable project trash? This will not erase private draft checkpoints.'
      $limit=(Get-Date).AddSeconds(25)
      while ((Test-Path $renamed) -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 400 }
      if ((Test-Path $renamed) -or !(Test-Path (Join-Path $fixed '.lu-ide-trash'))) {
        Deny 'dev01_native_trash_missing'
      }
      Click 'Restore file'
      $choice=Find-Control 'Recoverable project source file' '' 20
      if (!$choice.Current.IsKeyboardFocusable) { Deny 'dev01_restore_select_not_focusable' }
      $choice.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('{END}{ENTER}')
      Click 'Review and apply'
      Dev01-Confirm 'Restore renamed.rs from project trash? No existing file will be overwritten.'
      $limit=(Get-Date).AddSeconds(25)
      while (!(Test-Path $renamed) -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 400 }
      if (!(Test-Path $renamed) -or
          (Get-FileHash $renamed -Algorithm SHA256).Hash -ne $originalSha) {
        Deny 'dev01_restore_not_original_bytes'
      }
      Write-Host 'DEV01_MODERN_WINDOWS_MOVE_TRASH_RESTORE_NO_CLOBBER=PASS'
    }
    if ($Scenario -in @('B4SplitEditor','Dev01Modern')) {
      # Actual installed Windows WebView2 UIA, no JavaScript or Tauri API bypass.
      $mainSource=Join-Path $fixed 'main.rs'
      $secondSource=Join-Path $fixed 'second.rs'
      if (!(Test-Path -LiteralPath $secondSource -PathType Leaf)) {
        Deny 'b4_second_source_missing'
      }
      $mainBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath $mainSource).Hash
      $secondBefore=(Get-FileHash -Algorithm SHA256 -LiteralPath $secondSource).Hash
      # Opening the first file asynchronously disables the file tree. Wait
      # for the exact second file to become enabled; never override the gate.
      $b4Deadline=(Get-Date).AddSeconds(20)
      $secondFile=$null
      do {
        try {$secondFile=Find-Control 'second.rs' 'Button' 2 -Exact} catch {$secondFile=$null}
        if ($secondFile -and $secondFile.Current.IsEnabled) {break}
        Start-Sleep -Milliseconds 200
      } while ((Get-Date) -lt $b4Deadline)
      if (!$secondFile -or !$secondFile.Current.IsEnabled) {Deny 'b4_second_file_not_ready'}
      Reveal-NativeControl $secondFile
      $secondFile=Find-Control 'second.rs' 'Button' 5 -Exact
      if (!$secondFile.Current.IsEnabled -or $secondFile.Current.IsOffscreen) {Deny 'b4_second_file_not_ready'}
      $secondFile.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
      [void](Find-Control 'Source editor text · First editor: second.rs' '' 25 -Exact)
      Click 'Toggle split editor'
      Require 'Second editor file'
      Require 'Save second file'
      function Get-B4NativeEditors {
        $seen=@{}
        $editors=@()
        foreach($accessRoot in @(NativeAccessibilityRoots)) {
          try {
            $nodes=$accessRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,
              [System.Windows.Automation.Condition]::TrueCondition)
            foreach($node in $nodes) {
              try {
                if ([string]$node.Current.Name -cnotin @(
                    'Source editor text · First editor: second.rs',
                    'Source editor text · Second editor: main.rs')) { continue }
                $rect=$node.Current.BoundingRectangle
                if ($rect.IsEmpty -or $rect.Width -lt 60 -or $rect.Height -lt 40) { continue }
                $loc=('{0:F0}-{1:F0}-{2:F0}-{3:F0}' -f $rect.X,$rect.Y,$rect.Width,$rect.Height)
                if ($seen.ContainsKey($loc)) { continue }
                $seen[$loc]=$true
                if (!$node.Current.IsKeyboardFocusable) {
                  Deny 'b4_installed_codemirror_pane_not_focusable'
                }
                $editors+=,[pscustomobject]@{ Node=$node; X=$rect.X; Y=$rect.Y; Name=[string]$node.Current.Name }
              } catch {
                if ($_.Exception.Message -like 'WO8_NATIVE_UI_DENIED:*') { throw }
              }
            }
          } catch {
            if ($_.Exception.Message -like 'WO8_NATIVE_UI_DENIED:*') { throw }
          }
        }
        if ($editors.Count -ne 2) {
          Deny ('b4_installed_requires_two_distinct_accessible_editors_count_'+$editors.Count)
        }
        $primary=@($editors | Where-Object Name -CEQ 'Source editor text · First editor: second.rs')
        $secondary=@($editors | Where-Object Name -CEQ 'Source editor text · Second editor: main.rs')
        if ($primary.Count -ne 1 -or $secondary.Count -ne 1) {
          Deny 'b4_installed_editor_identity_mismatch'
        }
        return @($primary[0],$secondary[0])
      }
      $editors=@(Get-B4NativeEditors)
      $editors[1].Node.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('^{END}{ENTER}// B4_SECONDARY_ONLY')
      Start-Sleep -Milliseconds 650
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath $mainSource).Hash -ne $mainBefore -or
          (Get-FileHash -Algorithm SHA256 -LiteralPath $secondSource).Hash -ne $secondBefore) {
        Deny 'b4_unsaved_split_edit_modified_project_source'
      }
      Click 'Save second file'
      $deadline=(Get-Date).AddSeconds(16)
      $secondarySaved=$false
      do {
        Start-Sleep -Milliseconds 400
        $secondarySaved=[IO.File]::ReadAllText($mainSource).Contains('B4_SECONDARY_ONLY')
      } while (!$secondarySaved -and (Get-Date) -lt $deadline)
      if (!$secondarySaved -or
          (Get-FileHash -Algorithm SHA256 -LiteralPath $secondSource).Hash -ne $secondBefore) {
        Deny 'b4_secondary_save_modified_wrong_project_source'
      }
      # With the actual original primary editor physically focused, Ctrl+S
      # must save only that source, not the already-saved secondary source.
      $mainSaved=(Get-FileHash -Algorithm SHA256 -LiteralPath $mainSource).Hash
      $editors=@(Get-B4NativeEditors)
      $editors[0].Node.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('^{END}{ENTER}// B4_PRIMARY_ONLY')
      Start-Sleep -Milliseconds 500
      if ((Get-FileHash -Algorithm SHA256 -LiteralPath $secondSource).Hash -ne $secondBefore) {
        Deny 'b4_primary_draft_autosaved_without_user'
      }
      [System.Windows.Forms.SendKeys]::SendWait('^s')
      $deadline=(Get-Date).AddSeconds(16)
      $primarySaved=$false
      do {
        Start-Sleep -Milliseconds 400
        $primarySaved=[IO.File]::ReadAllText($secondSource).Contains('B4_PRIMARY_ONLY')
      } while (!$primarySaved -and (Get-Date) -lt $deadline)
      if (!$primarySaved -or
          (Get-FileHash -Algorithm SHA256 -LiteralPath $mainSource).Hash -ne $mainSaved) {
        Deny 'b4_focused_ctrl_s_saved_wrong_pane'
      }
      Write-Host 'B4_REAL_INSTALLED_WINDOWS_TWO_DISTINCT_CODEMIRROR_PANES=PASS'
      Write-Host 'B4_REAL_INSTALLED_WINDOWS_EXPLICIT_SECONDARY_AND_FOCUSED_PRIMARY_SAVE=PASS'
    }
    function Dev01-SessionReference {
      [System.Windows.Forms.Clipboard]::Clear()
      [void](Focus-ExactNativeControl 'Copy draft reference' 'Button')
      [System.Windows.Forms.SendKeys]::SendWait(' ')
      $limit=(Get-Date).AddSeconds(20)
      while ((Get-Date) -lt $limit) {
        Start-Sleep -Milliseconds 350
        if (![System.Windows.Forms.Clipboard]::ContainsText()) { continue }
        $value=[System.Windows.Forms.Clipboard]::GetText()
        if (!$value.StartsWith('ide-buffer:v1 ')) { continue }
        if ($value -cnotmatch ' path="main\.rs" buffer=([a-zA-Z0-9-]{8,128}) revision=([0-9]+) baseDisk=([a-f0-9]{64}) draftSha256=([a-f0-9]{64}) ' -or
            !$value.EndsWith(' lifetime=session-only') -or $value.Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) { Deny 'session_clipboard_reference_invalid_or_disclosing' }
        return [pscustomobject]@{buffer=$Matches[1]; revision=[long]$Matches[2]; base=$Matches[3]; digest=$Matches[4]}
      }
      Deny 'fresh_session_clipboard_reference_missing'
    }
    if ($Scenario -eq 'Dev01Modern') {
      # After the real two-editor native focus/Save witness above, checkpoint
      # another unsaved edit into OS-private app data before hard process kill.
      Click 'Toggle split editor'
      Click 'main.rs'
      $source=Join-Path $fixed 'main.rs'
      $beforeDraft=(Get-FileHash $source -Algorithm SHA256).Hash
      $editor=Find-Control 'Source editor text' '' 18
      if (!$editor.Current.IsKeyboardFocusable) { Deny 'dev01_private_editor_not_focusable' }
      $editor.SetFocus()
      [System.Windows.Forms.SendKeys]::SendWait('^{END}{ENTER}// DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')
      Start-Sleep -Milliseconds 650
      if ((Get-FileHash $source -Algorithm SHA256).Hash -ne $beforeDraft) {
        Deny 'dev01_implicit_save_before_kill'
      }
      Start-Sleep -Milliseconds 800
      $firstRef=Dev01-SessionReference
      if ($firstRef.base -ine $beforeDraft -or $firstRef.digest -ieq $beforeDraft) { Deny 'session_reference_not_exact_unsaved_base' }
      [void](Focus-ExactNativeControl 'Source editor text · First editor: main.rs')
      [System.Windows.Forms.SendKeys]::SendWait('^{END}x')
      Start-Sleep -Milliseconds 800
      $changedRef=Dev01-SessionReference
      [void](Focus-ExactNativeControl 'Source editor text · First editor: main.rs')
      [System.Windows.Forms.SendKeys]::SendWait('^z')
      Start-Sleep -Milliseconds 800
      $undoRef=Dev01-SessionReference
      if ($firstRef.buffer -cne $changedRef.buffer -or $firstRef.buffer -cne $undoRef.buffer -or
          $firstRef.revision -ge $changedRef.revision -or $changedRef.revision -ge $undoRef.revision -or
          $firstRef.digest -ceq $changedRef.digest -or $firstRef.digest -cne $undoRef.digest -or
          $undoRef.base -ine $beforeDraft -or (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $beforeDraft) { Deny 'session_reference_undo_lifecycle_not_verified' }
      $sessionEvidence=$fixed+'-r4r4-session.json'
      if (Test-Path -LiteralPath $sessionEvidence) { Deny 'stale_session_evidence' }
      $undoRef | ConvertTo-Json -Compress | Set-Content -LiteralPath $sessionEvidence -Encoding UTF8
      Write-Host 'DEV01_MODERN_WINDOWS_SESSION_REFERENCE_UNDO=PASS'
      [System.Windows.Forms.Clipboard]::Clear()
      Click 'Preserve draft + copy durable reference'
      $copy=''
      $limit=(Get-Date).AddSeconds(20)
      while ((Get-Date) -lt $limit) {
        Start-Sleep -Milliseconds 350
        if ([System.Windows.Forms.Clipboard]::ContainsText()) {
          $copy=[System.Windows.Forms.Clipboard]::GetText()
          if ($copy.StartsWith('ide-draft:v1')) { break }
        }
      }
      if (!$copy.StartsWith('ide-draft:v1') -or
          !$copy.Contains('checkpoint=') -or
          !$copy.Contains('lifetime=persistent-private-app-data') -or
          $copy.Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) {
        Deny 'dev01_durable_private_reference_missing_or_disclosing'
      }
      Click 'Remember open tabs for restart'
      Confirm-NativeOneUseDialog 'Remember tab layout' 'Remember navigation' 'Remember file tabs and pane layout only? Unsaved edits are NOT stored in this layout. Use Preserve draft for private recovery before a restart.'
      Require 'Remembered file tab layout for restart'
      if ((Get-FileHash $source -Algorithm SHA256).Hash -ne $beforeDraft) {
        Deny 'dev01_remembered_layout_wrote_unsaved_source'
      }
      # The common finally kills this real installed OS process. Recovery is
      # tested by an independent invocation in a new process.
      Write-Host 'DEV01_MODERN_WINDOWS_PRE_KILL_PRIVATE_CHECKPOINT=PASS'
    }
    if ($Scenario -eq 'Dev01ModernRecovery') {
      $source=Join-Path $fixed 'main.rs'
      if ([IO.File]::ReadAllText($source).Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) {
        Deny 'dev01_crash_implicitly_saved'
      }
      Require 'Recover preserved draft'
      Click 'Recover preserved draft'
      Require 'Recovered private checkpoint into unsaved editor only'
      if ([IO.File]::ReadAllText($source).Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) {
        Deny 'dev01_recovery_implicitly_saved'
      }
      $sessionEvidence=$fixed+'-r4r4-session.json'
      if (!(Test-Path -LiteralPath $sessionEvidence -PathType Leaf) -or (Get-Item -LiteralPath $sessionEvidence).LinkType -or (Get-Item -LiteralPath $sessionEvidence).Length -gt 1024) { Deny 'missing_or_invalid_session_evidence' }
      $previousRef=Get-Content -LiteralPath $sessionEvidence -Raw | ConvertFrom-Json
      [void](Focus-ExactNativeControl 'Source editor text · First editor: main.rs')
      [System.Windows.Forms.SendKeys]::SendWait('^{END}')
      $recoveredRef=Dev01-SessionReference
      if ($recoveredRef.buffer -ceq $previousRef.buffer -or $recoveredRef.base -cne $previousRef.base -or $recoveredRef.digest -cne $previousRef.digest) { Deny 'restarted_session_draft_identity_not_distinct' }
      Click 'Save · Ctrl+S'
      $limit=(Get-Date).AddSeconds(25)
      while ((Get-Date) -lt $limit -and
             ![IO.File]::ReadAllText($source).Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) {
        Start-Sleep -Milliseconds 450
      }
      if (![IO.File]::ReadAllText($source).Contains('DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL')) {
        Deny 'dev01_recovered_draft_not_explicitly_saved'
      }
      Require 'Saved source tab main.rs'
      [void](Focus-ExactNativeControl 'Source editor text · First editor: main.rs')
      [System.Windows.Forms.SendKeys]::SendWait('^{END}')
      $savedRef=Dev01-SessionReference
      $diskSha=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
      if ($savedRef.base -ine $diskSha -or $savedRef.digest -ine $diskSha) { Deny 'explicit_save_reference_not_rebased' }
      Remove-Item -LiteralPath $sessionEvidence
      Write-Host 'DEV01_MODERN_WINDOWS_SESSION_REFERENCE_RESTART_SAVE_REBASE=PASS'
      Write-Host 'DEV01_MODERN_WINDOWS_INSTALLED_HARD_KILL_RESTART_DRAFT=PASS'
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
  foreach($name in $workerBaseline.Keys) {
    if ((BoundedProcess $name) -gt $workerBaseline[$name]) {
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
