import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
test('B3 native OS UI stale search must refuse changed disk and refresh before opening', () => {
  const ui=read('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const scenario=ui.split("if ($Scenario -eq 'SearchProjectSaved') {",2)[1]?.split(
    "if ($Scenario -eq 'EditorPersistence') {",1)[0];
  assert.ok(scenario, 'missing B3 real installed scenario');
  for (const term of ["Click 'Find in project'","Search project text",
    "P05_B3_CHANGED_AFTER_SEARCH","Could not open this exact saved search result",
    "Click 'Search project'","Source explorer and editor",
    "stale_saved_search_result_mutated_source","fresh_saved_project_search_mutated_source"]) {
      assert.ok(scenario.includes(term),term);
  }
  assert.doesNotMatch(scenario,/window\.__TAURI_INTERNALS__|invoke_tauri|shell_exec/i);
});
test('B3 installed NSIS workflow verifies source-bound upstream notices, real uninstall and no signing claim',()=>{
  const ci=read('../../../.github/workflows/ide-p05-b3-installed-native.yml');
  for(const term of ["runs-on: windows-2022", "SearchProjectSaved",
    "npm ci --ignore-scripts", "verify_p06_candidate_lock.py",
    "collect_verification_licenses.py", "verify_p04_license_bundle.py",
    "uninstallers", "P05_B3_INSTALLED_NATIVE_WINDOWS_REVISION_SAFE_SEARCH=PASS"]) {
      assert.ok(ci.includes(term),term);
  }
  assert.doesNotMatch(ci,/New-SelfSignedCertificate|releaseApproved: true/i);
});
