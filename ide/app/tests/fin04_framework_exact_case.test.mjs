// FIN-04H static integration contracts. These tests are not protected OS
// execution, framework assertion evidence, or installed-application acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const root=new URL('../',import.meta.url);
const host=readFileSync(new URL('src-tauri/src/lib.rs',root),'utf8');
const web=readFileSync(new URL('web/main.jsx',root),'utf8');
const supervisor=readFileSync(new URL('../../supervisor/src/lib.rs',root),'utf8');
const build=readFileSync(new URL('src-tauri/src/build_backend.rs',root),'utf8');
test('FIN04H native Tauri action is explicit and remains feature-gated',()=>{
  assert.match(host,/async fn ide_run_protected_framework_test_case\(/);
  assert.match(host,/feature="native-multilang"/);
  assert.match(host,/FIN04H_OS_FRAMEWORK_WORKER_NOT_INSTALLED/);
  assert.match(host,/ide_run_protected_framework_test_case,/);
});
test('FIN04H reviewed symbol and sealed manifest gate precede worker authorization',()=>{
  assert.match(build,/pub fn run_selected_framework_case\(/);
  assert.match(build,/inspect_test_symbol_candidates/);
  assert.match(build,/FIN04H_ORIGINAL_MANIFEST_STALE/);
  assert.match(build,/Some\(\(relative_path.to_owned\(\),expected_source_revision.to_owned\(\)\)\)/);
  assert.match(build,/exact_framework_case\(recipe_id\)\.is_some\(\)/);
});
test('FIN04H restricted selectors never become arbitrary executable recipes',()=>{
  assert.match(supervisor,/pub fn exact_framework_case\(/);
  assert.match(supervisor,/pytest_exact_case:/);
  assert.match(supervisor,/node_exact_case:/);
  assert.match(supervisor,/cfg!\(feature = "multilang-worker"\) && exact_framework_case/);
});
test('FIN04H exact-case UI checks native output and never asserts test pass',()=>{
  assert.match(web,/function executeProtectedSavedFrameworkCase\(/);
  assert.match(web,/invoke\('ide_run_protected_framework_test_case'/);
  assert.match(web,/result\.recipeId!==exactRecipe/);
  assert.match(web,/result\.terminalReceiptSha256/);
  assert.match(web,/Runner output is reported evidence, not independent assertion proof/);
});
