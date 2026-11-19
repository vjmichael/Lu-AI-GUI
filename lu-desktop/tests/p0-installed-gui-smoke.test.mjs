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

test('CP-05 native consent verifies original dialog scope and TaskDialog HWND, not generic pane or label',()=>{
  const gguf=readFileSync(new URL('../../scripts/cp05e5-windows-installed-gguf.ps1',import.meta.url),'utf8');
  const predicate=gguf.split('function Test-ConsentControlIdentity(')[1].split('function Consent(')[0];
  const consent=gguf.split('function Consent(')[1].split('function Start-Long')[0];
  const fixture=readFileSync(new URL('../../scripts/cp05e5-consent-contract.test.ps1',import.meta.url),'utf8');
  const closeout=readFileSync(new URL('../../.github/workflows/lu-cp05e-closeout.yml',import.meta.url),'utf8');
  assert.match(predicate,/'Allow once'\{'CommandButton_1000'\}/);
  assert.match(predicate,/'Cancel'\{'CommandButton_1001'\}/);
  assert.match(predicate,/\$current\.ClassName -ceq 'CCPushButton'/);
  assert.match(predicate,/\$current\.ControlType\.ProgrammaticName -cin @\('ControlType\.Pane','ControlType\.Button'\)/);
  assert.match(predicate,/\[int64\]\$current\.ProcessId -eq \$ownerPid/);
  assert.match(predicate,/\$current\.IsEnabled -and -not \$current\.IsOffscreen/);
  assert.match(consent,/Wait-NativeWindow 'Allow this inspection\?'/);
  assert.match(consent,/ContentText/);
  assert.match(consent,/Native consent text does not match exact synthetic request and scope/);
  assert.match(consent,/if\(\$matches\.Count -ne 1\)/);
  assert.match(consent,/OwnerPid\(\$child\)/);
  assert.match(consent,/\$childPidNow -eq \$script:process\.Id/);
  assert.match(consent,/IsChild\(\$owner,\$child\)/);
  assert.match(consent,/observedNativeControlId=\[LuCp05Window\]::GetDlgCtrlID\(\$child\)/);
  assert.doesNotMatch(consent,/GetDlgCtrlID\(\$child\) -eq \$controlId/);
  assert.match(consent,/\$freshContent\.Count -eq 1/);
  assert.match(consent,/GetForegroundWindow\(\) -eq \$owner/);
  assert.match(consent,/GetAncestor\(\$hit,2\)/);
  assert.match(consent,/AutomationElement\]::FromPoint/);
  assert.match(consent,/\$atId -ceq \$controlRuntimeId/);
  assert.match(consent,/Test-ConsentHitIdentity \$proof/);
  assert.match(consent,/Save-ConsentSnapshot \$hitProof 'hit-final'/);
  assert.match(gguf,/\$proof\.hitInModal -and \(\$proof\.nativeTargetMatch -or \$proof\.uiaTargetMatch\)/);
  assert.match(fixture,/CP05_NATIVE_CONSENT_HIT_FIXTURES_PASS=10/);
  assert.match(consent,/HitWindow\(\$x,\$y\)/);
  assert.equal((consent.match(/mouse_event\(/g)||[]).length,2);
  assert.match(consent,/original consent dialog dismissed/);
  assert.doesNotMatch(consent,/Wait-Element \$dialog|SendKeys|Invoke-UIA/);
  assert.match(fixture,/CP05_NATIVE_CONSENT_IDENTITY_FIXTURES_PASS=13/);
  assert.match(closeout,/pwsh -NoProfile -File scripts\/cp05e5-consent-contract\.test\.ps1/);
  assert.match(gguf,/Check-Journal 'approve' 180/);
  assert.match(gguf,/Check-Journal 'deny' 90/);
});
