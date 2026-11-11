import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const get = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const source = get('../../host/src/git_protected_merge.rs');
const mergePlan = get('../../host/src/git_merge_plan.rs');
const lib = get('../../host/src/lib.rs');
const tauri = get('../src-tauri/src/lib.rs');
const ui = get('../web/main.jsx');

test('G06C03 source execution is fast-forward-only and protects Git worktree/ref integrity', () => {
  assert.match(lib, /pub mod git_protected_merge;/);
  assert.match(source, /pub fn execute_os_approved_fast_forward\(/);
  assert.match(source, /pub fn revalidate_protected_fast_forward\(/);
  assert.match(source, /pub fn inspect_protected_merge_recovery\(/);
  assert.match(source, /pub fn reconcile_protected_merge_receipt\(/);
  assert.match(source, /G06C03_NON_FF_OR_BLOCKED_MERGE_DENIED/);
  assert.match(source, /G06C03_STALE_OR_FORGED_MERGE_PLAN/);
  assert.match(source, /G06C03_DANGEROUS_PATH_COMPONENT/);
  assert.match(source, /G06C03_SYMLINK_IN_WORKTREE/);
  assert.match(source, /repo\.checkout_tree\(&target, Some\(&mut checkout\)\)/);
  assert.match(source, /checkout\s*\.safe\(\)[\s\S]{0,180}\.recreate_missing\(true\)[\s\S]{0,180}\.remove_untracked\(false\)/);
  assert.match(source, /\.overwrite_ignored\(false\)[\s\S]{0,100}\.update_index\(true\)/);
  assert.match(source, /repo\.reference_matching\(/);
  assert.match(source, /G06C03_REF_CAS_FAILED_RECOVERY_REQUIRED/);
  assert.match(source, /index_matches_tree\(&repo, &plan.selected_tree\)/);
  assert.match(source, /journal\.phase = "checkout_intent"/);
  assert.match(source, /journal\.phase = "publishing"/);
  assert.match(source, /journal\.phase = "verified"/);
  assert.match(source, /cancel\s*\.compare_exchange\(0, 1, Ordering::SeqCst, Ordering::SeqCst\)/);
  const prod = source.split('#[cfg(test)]\nmod tests')[0];
  for (const forbidden of [
    'repo.reset(', 'repo.checkout_head(', 'repo.merge(', 'repo.rebase(',
    'repo.fetch(', 'repo.push(', 'repo.commit(', 'repo.cherrypick(',
    'repo.stash_save(', 'std::process::Command',
  ]) assert.ok(!prod.includes(forbidden), forbidden);
});

test('G06C03 authorizations use actual one-use OS dialog rather than WebView plan digest', () => {
  for (const name of [
    'ide_git_execute_fast_forward',
    'ide_git_cancel_protected_merge',
    'ide_git_inspect_protected_merge_recovery',
    'ide_git_reconcile_protected_merge_receipt',
  ]) assert.match(tauri, new RegExp('async fn '+name+'\\('));
  assert.match(tauri, /sessions\.begin\(&project_id\)/);
  assert.match(tauri, /revalidate_protected_fast_forward\(/);
  assert.match(tauri, /app\.dialog\(\)\.message\(format!\(/);
  assert.match(tauri, /G06C03_HUMAN_DENIED/);
  assert.match(tauri, /execute_os_approved_fast_forward\(/);
  assert.match(tauri, /G06C03_RECOVERY_HUMAN_DENIED/);
  assert.match(tauri, /reconcile_protected_merge_receipt\(/);
  assert.match(tauri, /ide_git_execute_fast_forward,/);
  assert.match(mergePlan, /authorization_granted: false/);
});

test('G06C03 recovery is observational and refuses unsafe locks and uncertain effects', () => {
  assert.match(source, /G06C03_RECOVERY_FOREIGN_LOCK/);
  assert.match(source, /G06C03_RECOVERY_MANUAL_REPAIR_REQUIRED/);
  assert.match(source, /checkout_completed_ref_not_published/);
  assert.match(source, /published_ref_requires_operator_receipt/);
  assert.match(source, /prepublication_no_index_change_pending_review/);
  assert.match(source, /automatic_retry: false/);
  assert.match(source, /automatic_ref_rewrite: false/);
  assert.match(source, /automatic_checkout: false/);
  assert.match(source, /automatic_lock_deletion: false/);
  assert.match(source, /test.*real_fast_forward_advances_head_index_and_worktree/s);
  assert.match(source, /recovery_rejects_unpublished_checkout_and_foreign_index_lock/);
});

test('G06C03 UI offers fast-forward only after exact clean merge preview; never fake OS success', () => {
  assert.match(ui, /invoke\('ide_git_execute_fast_forward'/);
  assert.match(ui, /invoke\('ide_git_cancel_protected_merge'/);
  assert.match(ui, /invoke\('ide_git_inspect_protected_merge_recovery'/);
  assert.match(ui, /invoke\('ide_git_reconcile_protected_merge_receipt'/);
  assert.match(ui, /G06C03_FAST_FORWARD_RECEIPT_UNTRUSTED/);
  assert.match(ui, /mergePlan\.classification === 'fast_forward_candidate'/);
  assert.match(ui, /mergePlan\.blockers\.length === 0/);
  assert.match(ui, /mergePlanRequest\.current \+= 1/);
  assert.match(ui, /No automatic checkout, branch rewrite, rollback or lock deletion/);
  assert.doesNotMatch(ui, /invoke\('ide_git_execute_divergent_merge'/);
});
