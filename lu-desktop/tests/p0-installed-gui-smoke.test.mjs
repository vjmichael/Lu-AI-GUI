import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const script=readFileSync(new URL('../../scripts/p0-windows-installed-gui-smoke.ps1',import.meta.url),'utf8');
const cycle=readFileSync(new URL('../../scripts/m3-windows-nsis-cycle.ps1',import.meta.url),'utf8');
const workflow=readFileSync(new URL('../../.github/workflows/lu-repo01-full-source.yml',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
test('actual installed GUI uses OS window, process identity, UIA and screen capture; no mock Tauri claim',()=>{
  assert.match(script,/MainWindowHandle/);
  assert.match(script,/FromHandle/);
  assert.match(script,/ProcessId/);
  assert.match(script,/UIAutomationClient/);
  assert.match(script,/CopyFromScreen/);
  assert.match(script,/PASS_INSTALLED_UNIFIED_TABS/);
  assert.match(script,/Save-UIA-Failure/);
  assert.match(script,/inspectedCount/);
  assert.match(script,/failed-/);
  assert.match(script,/SetFocus\(\)/);
  assert.match(script,/genuineGgufInference='NOT_TESTED'/);
  assert.match(script,/activeGpuOffload='NOT_TESTED'/);
});
test('GUI smoke is independent from strict real install-uninstall/AppData and only P0 opted in',()=>{
  assert.match(cycle,/ExerciseInstalledGui/);
  assert.match(cycle,/packageRemoval -ne 'PASS_BINARIES_ABSENT'/);
  assert.match(cycle,/ExerciseInstalledGui -and \$record\.installedUiSmoke/);
  assert.match(workflow,/LU_PR_NUMBER/);
  assert.match(workflow,/ExerciseInstalledGui:\$verifyUi/);
  assert.match(ui,/className="luTopTabs"/);
  assert.match(ui,/className="modelSwitcher" aria-label="Choose a model"/);
});

test('unified tabs in a real Tauri window activate through supported UIA or verified physical OS click',()=>{
  assert.match(script,/TryGetCurrentPattern\(\[System\.Windows\.Automation\.InvokePattern\]::Pattern/);
  // The optional legacy provider type does not exist on every current
  // PowerShell/.NET UIAutomationClient runtime. Its absence must NOT
  // abort the genuine OS click + installed-dialog evidence path.
  assert.match(script,/LegacyIAccessiblePattern' -as \[type\]/);
  assert.match(script,/\$null -ne \$legacyType/);
  assert.doesNotMatch(script,/\[System\.Windows\.Automation\.LegacyIAccessiblePattern\]::Pattern/);
  assert.match(script,/GetClickablePoint\(\)/);
  assert.match(script,/WebView clickable point not within reviewed installed native window/);
  assert.match(script,/SetCursorPos/);
  assert.match(script,/activationMethods/);
  assert.match(script,/Wait-Element \$window 'Settings' \$true/);
  assert.match(script,/Wait-Element \$window 'Models' \$true/);
  assert.match(script,/Wait-Element \$window 'History' \$true/);
  assert.match(script,/Wait-Element \$window 'Tasks' \$true/);
  assert.match(script,/NativeWindowHandle/);
  assert.match(script,/PASS_SAME_NATIVE_WINDOW/);
  assert.match(script,/PASS_ONE_INSTALLED_WINDOW/);
  assert.doesNotMatch(script,/PASS_NATIVE_WEBVIEW_DIALOG/);
});

test('CP-05 installed GGUF witness waits for ready WebView descendants rather than requiring focus of the first HWND',()=>{
  const gguf=readFileSync(new URL('../../scripts/cp05e5-windows-installed-gguf.ps1',import.meta.url),'utf8');
  const startup=gguf.split('function Start-Lu(')[1].split('function Engine-Probe')[0];
  assert.match(startup,/Wait-Enabled 'Choose a model' 60/);
  assert.match(startup,/MainWindowHandle\.ToInt64\(\)/);
  assert.match(startup,/webViewDescendantReady=\$true/);
  assert.doesNotMatch(startup,/\$script:window\.SetFocus\(\)/);
  const composer=gguf.split('function Send-Fixture(')[1].split('function Check-Journal')[0];
  assert.match(composer,/GetRuntimeId\(\)/);
  assert.match(composer,/AutomationElement\]::FocusedElement/);
  assert.match(composer,/GetForegroundWindow\(\)/);
  assert.match(composer,/GetClickablePoint\(\)/);
  assert.match(composer,/if\(-not \(Test-ComposerFocus\)\)\{throw 'Composer focus lost before paste'\}/);
  assert.match(composer,/Wait-Enabled 'Send' 20/);
  assert.match(gguf,/Check-Journal 'approve'/);
  assert.match(gguf,/Check-Journal 'deny'/);
  assert.match(gguf,/Slot-State \$true/);
  assert.match(gguf,/Close-Exact \$true 'forced'/);
});
