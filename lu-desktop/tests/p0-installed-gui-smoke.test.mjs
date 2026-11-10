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
  assert.match(script,/PASS_INSTALLED_NATIVE_WINDOW_AND_BOTH_DIALOGS/);
  assert.match(script,/genuineGgufInference='NOT_TESTED'/);
  assert.match(script,/activeGpuOffload='NOT_TESTED'/);
});
test('GUI smoke is independent from strict real install-uninstall/AppData and only P0 opted in',()=>{
  assert.match(cycle,/ExerciseInstalledGui/);
  assert.match(cycle,/packageRemoval -ne 'PASS_BINARIES_ABSENT'/);
  assert.match(cycle,/ExerciseInstalledGui -and \$record\.installedUiSmoke/);
  assert.match(workflow,/LU_PR_NUMBER/);
  assert.match(workflow,/ExerciseInstalledGui:\$verifyUi/);
  assert.match(ui,/className="quietSettings" title="Settings" aria-label="Settings"/);
  assert.match(ui,/className="modelSwitcher" aria-label="Choose a model"/);
});

test('WebView2 accessibility activates through supported UIA or independently verified OS click, never simulation',()=>{
  assert.match(script,/TryGetCurrentPattern\(\[System\.Windows\.Automation\.InvokePattern\]::Pattern/);
  assert.match(script,/TryGetCurrentPattern\(\[System\.Windows\.Automation\.LegacyIAccessiblePattern\]::Pattern/);
  assert.match(script,/GetClickablePoint\(\)/);
  assert.match(script,/WebView clickable point not within reviewed installed native window/);
  assert.match(script,/SetCursorPos/);
  assert.match(script,/activationMethods/);
  assert.match(script,/Wait-Element \$window 'Model Library'/);
});
