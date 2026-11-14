import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const host = readFileSync('src-tauri/src/lib.rs', 'utf8');
const ui = readFileSync('web/main.jsx', 'utf8');
const native = readFileSync('../host/src/git_conflict_inventory.rs', 'utf8');

test('6D01 conflict inspection is registered read-only with no approval route', () => {
  assert.match(host, /async fn ide_git_inspect_native_conflicts\(/);
  assert.match(host, /registry\.inspect_native_git_conflicts\(/);
  assert.match(ui, /invoke\('ide_git_inspect_native_conflicts'/);
  assert.match(ui, /result\.authorityGranted !== false/);
  assert.match(ui, /result\.resolutionWritten !== false/);
  assert.match(ui, /Inspect unresolved Git conflicts \(read only\)/);
});
test('6D01 reads original index conflict stages and validates evidence', () => {
  assert.match(native, /index\.conflicts\(\)/);
  assert.match(native, /c\.ancestor\.as_ref\(\)/);
  assert.match(native, /c\.our\.as_ref\(\)/);
  assert.match(native, /c\.their\.as_ref\(\)/);
  assert.match(native, /raw_index\(repo\.path\(\)\)\? != before_index/);
  assert.match(native, /changed_path_is_plain_safe\(\&root, first\)/);
  assert.match(native, /authority_granted: false, resolution_written: false/);
  assert.match(native, /real_unresolved_stage_one_two_three_is_inspected_without_mutation/);
});
