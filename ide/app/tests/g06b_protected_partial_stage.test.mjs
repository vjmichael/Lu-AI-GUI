import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const native = read('../../host/src/git_partial_stage.rs');
const host = read('../../host/src/lib.rs');
const reviewed = read('../../host/src/reviewed_git.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('6B03 native stage is pinned to exact source, index, head, and Git lock', () => {
  assert.match(host, /pub mod git_partial_stage;/);
  assert.match(reviewed, /fn reread_reviewed_git_source_bytes\(/);
  assert.match(native, /select_reviewed_git_hunks/);
  assert.match(native, /read_reviewed_git_hunk_buffers/);
  assert.match(native, /selected_index_bytes\(&pinned\.index_bytes, &fresh\)/);
  assert.match(native, /repo\.blob\(&projected\)/);
  assert.match(native, /Index::open\(&temp_path\)/);
  assert.match(native, /create_new\(true\)\.open\(&lock_path\)/);
  assert.match(native, /fs::rename\(&lock_path, &old_index_path\)/);
  assert.match(native, /G06B_STAGE_PREEXISTING_STAGED_CHANGES/);
  assert.match(native, /G06B_STAGE_GIT_OPERATION_OR_WORKTREE_DENIED/);
  assert.doesNotMatch(native.split('#[cfg(test)]')[0], /std::process::Command|repo\.push\(|repo\.fetch\(|repo\.commit\(/);
});

test('6B03 cancellation and durable recovery never auto delete foreign Git lock', () => {
  assert.match(native, /request_partial_stage_cancel\(/);
  assert.match(native, /compare_exchange\(0, 1, Ordering::SeqCst/);
  assert.match(native, /journal_append\(&path, &journal\)/);
  assert.match(native, /operator_reconciliation_required/);
  assert.match(native, /auto_delete_git_lock: false/);
  assert.match(native, /auto_rollback_index: false/);
  assert.match(native, /selected_hunk_only_is_published_original_worktree_and_other_lines_preserved/);
  assert.match(native, /cancellation_before_publication_denies_without_index_changes/);
  assert.match(native, /preexisting_lock_and_stale_selection_refuse_without_clobber/);
  assert.match(native, /interrupted_journal_is_detected_and_never_auto_repaired/);
});

test('6B03 only native OS dialog can authorize one stage; Tauri cancel and recovery explicit', () => {
  assert.match(tauri, /async fn ide_git_stage_selected_hunks\(/);
  assert.match(tauri, /async fn ide_git_cancel_partial_stage\(/);
  assert.match(tauri, /async fn ide_git_partial_stage_recovery\(/);
  assert.match(tauri, /MessageDialogButtons::OkCancelCustom/);
  assert.match(tauri, /if !approved/);
  assert.match(tauri, /registry\.apply_approved_partial_stage\(&refreshed, &cancel\)/);
  assert.match(tauri, /app\.manage\(Arc::new\(PartialStageSessions::default\(\)\)\)/);
  assert.match(ui, /invoke\('ide_git_stage_selected_hunks'/);
  assert.match(ui, /invoke\('ide_git_cancel_partial_stage'/);
  assert.match(ui, /invoke\('ide_git_partial_stage_recovery'/);
  assert.match(ui, /No automatic rollback or Git lock removal is authorized/);
});
