import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (relative) => readFileSync(new URL(relative, import.meta.url), 'utf8');
const native = read('../../host/src/git_hunk_review.rs');
const pinned = read('../../host/src/reviewed_git.rs');
const host = read('../../host/src/lib.rs');
const desktop = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06B02 hunk bytes come only from registered native project and clean index', () => {
  assert.match(host, /pub mod git_hunk_review;/);
  assert.match(pinned, /fn read_reviewed_git_hunk_buffers\(/);
  assert.match(pinned, /self\.preview_reviewed_git_file\(project_id, relative_path\)/);
  assert.match(pinned, /read_pinned_source\(self, project_id, relative_path\)/);
  assert.match(pinned, /index_fingerprint\(&latest_index\) != reviewed\.index_fingerprint/);
  assert.match(native, /self\.read_reviewed_git_hunk_buffers\(project_id, relative_path\)/);
  assert.match(native, /Patch::from_buffers\(old, Some\(path\), new, Some\(path\)/);
});

test('G06B02 rejects binary, oversize, duplicate IDs and stale or tampered selection', () => {
  assert.match(native, /old\.contains\(&0\)\s*\|\|\s*new\.contains\(&0\)/);
  assert.match(native, /MAX_HUNKS: usize = 48/);
  assert.match(native, /MAX_LINES_PER_HUNK: usize = 256/);
  assert.match(native, /MAX_PATCH_BYTES: usize = 96 \* 1024/);
  assert.match(native, /G06B_HUNK_SELECTION_DUPLICATE_ID/);
  assert.match(native, /G06B_HUNK_PLAN_STALE_OR_TAMPERED/);
  assert.match(native, /two_separate_exact_hunks_are_selectable_without_index_mutation/);
  assert.match(native, /stale_source_forged_hunk_or_duplicate_selection_is_denied/);
  assert.match(native, /binary_and_unregistered_or_untracked_paths_fail_closed/);
  const prod = native.split('#[cfg(test)]')[0];
  for (const forbidden of ['Command::new', 'std::process::', 'repo.checkout',
    'repo.fetch', 'repo.push', 'index.write()', 'repo.commit(',
    'stage_authorized: true', 'commit_authorized: true', 'operation_performed: true']) {
    assert.ok(!prod.includes(forbidden), forbidden);
  }
});

test('G06B02 Tauri and UI expose preview and recheck, never stage a chosen hunk', () => {
  assert.match(desktop, /async fn ide_git_preview_hunks\(/);
  assert.match(desktop, /async fn ide_git_select_reviewed_hunks\(/);
  assert.match(desktop, /registry\.preview_reviewed_git_hunks\(&project_id, &relative_path\)/);
  assert.match(desktop, /registry\.select_reviewed_git_hunks\(&expected, &selected_ids\)/);
  assert.match(desktop, /ide_git_preview_hunks,\s*ide_git_select_reviewed_hunks,/);
  assert.match(ui, /invoke\('ide_git_preview_hunks'/);
  assert.match(ui, /invoke\('ide_git_select_reviewed_hunks'/);
  assert.match(ui, /gitHunkPlan\.hunks\.map/);
  assert.match(ui, /gitHunkRequest\.current \+= 1/);
  assert.match(ui, /Stage authorized: <strong>No<\/strong>/);
  assert.doesNotMatch(ui, /invoke\('ide_git_apply_selected_hunks'/);
  assert.doesNotMatch(ui, /invoke\('ide_git_stage_selected_hunks'/);
});
