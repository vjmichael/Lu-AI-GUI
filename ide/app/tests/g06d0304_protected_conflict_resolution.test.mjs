import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const native = readFileSync('../host/src/git_conflict_resolution.rs', 'utf8');
const host = readFileSync('src-tauri/src/lib.rs', 'utf8');
const ui = readFileSync('web/main.jsx', 'utf8');

test('6D03 proposal is source-bound but never a grant or automatic mutation', () => {
  assert.match(native, /pub fn review_native_conflict_resolution\(/);
  assert.match(native, /inventory\.evidence_sha256 != inventory_sha/);
  assert.match(native, /inventory\.index_sha256 != index_sha_claim/);
  assert.match(native, /worktree_bytes\(&root, relative\)\?/);
  assert.match(native, /other_entries_sha\(&repo, relative\)\?/);
  assert.match(native, /G06D03_MODE_CHANGE_REQUIRES_SPECIALIST/);
  assert.match(native, /automatically_applied: false, authority_granted: false/);
  assert.match(host, /async fn ide_git_review_conflict_resolution\(/);
  assert.match(ui, /invoke\('ide_git_review_conflict_resolution'/);
  assert.match(ui, /reviewed\.requiresSeparateNativeOsApproval !== true/);
  assert.match(ui, /reviewed\.authorityGranted !== false/);
});
test('6D04 OS gate is independent of source-reviewed proposal', () => {
  assert.match(host, /async fn ide_git_apply_conflict_resolution\(/);
  assert.match(host, /sessions\.begin\(&project_id\)/);
  assert.match(host, /app\.dialog\(\)\.message\(format!\(/);
  assert.match(host, /G06D04_NATIVE_HUMAN_REFUSED/);
  assert.match(host, /registry\.apply_os_approved_conflict_resolution\(/);
  assert.match(native, /pub fn apply_os_approved_conflict_resolution\(/);
  assert.match(native, /cancel\.compare_exchange\(0, 1, Ordering::SeqCst, Ordering::SeqCst\)/);
  assert.match(native, /journal_append\(&journal, &j\)\?/);
  assert.match(native, /idx\.add_frombuffer\(&basis, candidate\.as_bytes\(\)\)/);
  assert.match(native, /basis\.flags &= !0x3000/);
  assert.match(native, /other_entries_sha\(&repo, relative\)\? != j\.other_index_entries_sha256/);
  assert.match(ui, /invoke\('ide_git_apply_conflict_resolution'/);
});
test('6D04 restart recovery never implicitly restores or repeats execution', () => {
  assert.match(native, /pub fn inspect_protected_conflict_recovery\(/);
  assert.match(native, /pub fn reconcile_protected_conflict_receipt\(/);
  assert.match(native, /partial_or_drifted_manual_repair/);
  assert.match(native, /resolved_requires_independent_receipt/);
  assert.match(native, /automatic_retry: false, automatic_restore: false/);
  assert.match(native, /automatic_index_write: false, automatic_ref_update: false/);
  assert.match(native, /genuine_abort_after_file_replace_is_manual_repair_only/);
  assert.match(native, /genuine_abort_after_index_write_requires_receipt_only/);
  assert.match(host, /async fn ide_git_reconcile_conflict_receipt\(/);
  assert.match(host, /G06D04_RECOVERY_HUMAN_REFUSED/);
  assert.match(ui, /invoke\('ide_git_inspect_conflict_recovery'/);
  assert.match(ui, /invoke\('ide_git_reconcile_conflict_receipt'/);
});
