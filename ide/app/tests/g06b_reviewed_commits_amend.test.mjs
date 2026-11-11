import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const native = read('../../host/src/git_reviewed_commit.rs');
const partial = read('../../host/src/git_partial_stage.rs');
const host = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06B04 only a private verified staged-hunk journal may be committed', () => {
  assert.match(host, /pub mod git_reviewed_commit;/);
  assert.match(partial, /fn verify_partial_stage_for_commit\(/);
  assert.match(partial, /pub fn restore_verified_partial_stage_receipt\(/);
  assert.match(partial, /G06B_COMMIT_UNTRUSTED_STAGE_RECEIPT/);
  assert.match(partial, /last\.source_sha256 != claimed\.source_sha256/);
  assert.match(partial, /last\.selected_hunks != claimed\.selected_hunks/);
  assert.match(partial, /G06B_COMMIT_ORIGINAL_BLOB_RECEIPT_FORGED/);
  assert.match(partial, /G06B_COMMIT_UNREVIEWED_STAGED_PATHS/);
  assert.match(native, /self\.verify_partial_stage_for_commit\(stage\)/);
  const previewOnly = native.split('pub fn preview_reviewed_partial_commit(')[1]
    .split('pub fn execute_approved_reviewed_commit(')[0];
  assert.doesNotMatch(previewOnly, /\.write_tree_to\(/);
  assert.match(native, /let tree_oid = index\.write_tree_to\(&repo\)/);
  assert.match(ui, /Materialized only after separate native OS approval/);
  assert.match(native, /G06B_COMMIT_STALE_OR_FORGED_PLAN/);
  assert.match(native, /G06B_COMMIT_INDEX_TREE_CHANGED/);
  assert.match(native, /G06B_COMMIT_SOURCE_CHANGED_BEFORE_CAS/);
});

test('G06B04 exact branch ref CAS, authored tree and explicit amend parent', () => {
  assert.match(native, /pub enum ReviewedCommitMode \{ Create, Amend \}/);
  assert.match(native, /G06B_AMEND_ROOT_OR_MERGE_DENIED/);
  assert.match(native, /G06B_AMEND_UPSTREAM_UNVERIFIABLE/);
  assert.match(native, /repo\.reference_matching\(/);
  assert.match(native, /G06B_COMMIT_CONCURRENT_REF_UPDATE_DENIED/);
  assert.match(native, /repo\.commit\(None,/);
  assert.match(native, /G06B_COMMIT_PUBLISHED_TREE_OR_PARENT_MISMATCH/);
  assert.match(native, /local_commit_contains_only_selected_hunk_with_unchanged_worktree/);
  assert.match(native, /amend_requires_distinct_review_and_rewrites_only_exact_current_head/);
  const production = native.split('#[cfg(test)]')[0];
  for (const forbidden of [
    'std::process::Command', 'repo.push(', 'repo.fetch(',
    'repo.checkout(', 'repo.reset(', 'repo.commit(Some("HEAD")',
  ]) assert.ok(!production.includes(forbidden), forbidden);
});

test('G06B04 distinct native OS authorizations, double-confirm amend and host cancellation', () => {
  for (const cmd of [
    'ide_git_preview_reviewed_commit',
    'ide_git_commit_selected_hunks',
    'ide_git_amend_reviewed_commit',
    'ide_git_cancel_reviewed_commit',
    'ide_git_inspect_commit_recovery',
    'ide_git_reconcile_commit_receipt',
  ]) {
    assert.ok(tauri.includes('async fn ' + cmd + '('), cmd);
    assert.ok(ui.includes("invoke('" + cmd + "'") ||
      ['ide_git_commit_selected_hunks', 'ide_git_amend_reviewed_commit']
        .includes(cmd) && ui.includes("'" + cmd + "'"), cmd);
  }
  assert.match(tauri, /G06B_COMMIT_MODE_NOT_AUTHORIZED/);
  assert.match(tauri, /async fn ide_git_restore_reviewed_stage_receipt\(/);
  assert.match(ui, /invoke\('ide_git_restore_reviewed_stage_receipt'/);
  assert.match(tauri, /FINAL HISTORY-REWRITE CONSENT/);
  assert.match(tauri, /if !rewrite/);
  assert.match(tauri, /if !allowed/);
  assert.match(tauri, /sessions\.begin\(&project_id\)/);
  assert.match(ui, /Request separate OS authorization to amend local HEAD/);
  assert.match(ui, /Amendment requires two separate OS confirmations/);
  assert.doesNotMatch(ui, /stageReviewedSelectedHunks\(\)[\s\S]{0,1100}invoke\('ide_git_commit_selected_hunks'/);
});

test('G06B04 durable restart evidence cannot auto force update branch', () => {
  assert.match(native, /fn append_journal\(/);
  assert.match(native, /pub fn inspect_reviewed_commit_recovery\(/);
  assert.match(native, /pub fn reconcile_reviewed_commit_journal\(/);
  assert.match(native, /automatic_ref_rewrite: false/);
  assert.match(native, /automatic_retry: false/);
  assert.match(native, /interrupted_prepublication_journal_is_only_reconciled_after_new_review/);
  assert.match(native, /restart_restores_exact_staged_receipt_but_never_reuses_os_commit_grant/);
  assert.match(native, /forged_stage_stale_index_and_cancel_cannot_publish_a_ref/);
  assert.match(ui, /No automatic\s+retry, history rewrite, or lock deletion is authorized/);
});
