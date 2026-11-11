import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url),'utf8');
const plan = read('../../host/src/git_rebase_cherry_plan.rs');
const quarantine = read('../../host/src/git_replay_quarantine.rs');
const protectedReplay = read('../../host/src/git_protected_replay.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');
const lib = read('../../host/src/lib.rs');

test('6C04B prospective replay uses isolated bare ODB, no original project mutation',()=>{
  assert.match(lib,/pub mod git_replay_quarantine;/);
  assert.match(quarantine,/Repository::init_bare\(/);
  assert.match(quarantine,/\.add_disk_alternate\(/);
  assert.match(quarantine,/quarantine\.cherrypick_commit\(/);
  assert.match(quarantine,/idx\.has_conflicts\(\)/);
  assert.match(quarantine,/idx\.write_tree_to\(quarantine\)/);
  assert.match(quarantine,/quarantine\.commit\(None,/);
  assert.match(quarantine,/G06C04B_SOURCE_CHANGED_DURING_QUARANTINE/);
  assert.match(quarantine,/original_project_unchanged: true/);
  assert.match(quarantine,/project_odb_modified: false/);
  assert.match(quarantine,/execution_route_available: false/);
  const production=quarantine.split(/#\[cfg\(test\)\]\r?\nmod tests\s*\{/)[0];
  assert.doesNotMatch(production,/original\.commit\(/);
  assert.doesNotMatch(production,/original\.checkout_tree\(/);
  assert.doesNotMatch(production,/original\.reference_matching\(/);
  assert.doesNotMatch(production,/original\.index\(\)\.write\(/);
});
test('6C04C/D ref CAS + original rebase ref backup, no replay without OS consent',()=>{
  assert.match(lib,/pub mod git_protected_replay;/);
  assert.match(protectedReplay,/pub fn revalidate_protected_replay\(/);
  assert.match(protectedReplay,/pub fn execute_os_approved_replay\(/);
  assert.match(protectedReplay,/candidate_is_authorizable\(/);
  assert.match(protectedReplay,/copy_quarantine_objects\(/);
  assert.match(protectedReplay,/preserved_original_ref/);
  assert.match(protectedReplay,/refs\/lu-ide\/g06c04\/original\//);
  assert.match(protectedReplay,/repo\.reference_matching\(/);
  assert.match(protectedReplay,/G06C04_EXACT_REF_CAS_FAILED_MANUAL_REPAIR/);
  assert.match(protectedReplay,/G06C04_FOREIGN_INDEX_LOCK/);
  assert.match(protectedReplay,/G06C04_REVIEW_STALE_OR_FORGED/);
  assert.match(protectedReplay,/cancel\.compare_exchange\(0,1,Ordering::SeqCst,Ordering::SeqCst\)/);
  assert.match(protectedReplay,/automatic_ref_retry:false/);
  assert.match(protectedReplay,/automatic_history_rewrite:false/);
  assert.match(protectedReplay,/automatic_checkout:false/);
  assert.match(protectedReplay,/automatic_rollback:false/);
  assert.match(protectedReplay,/automatic_foreign_lock_deletion:false/);
  assert.match(protectedReplay,/test_abort_disposable/);
  assert.match(protectedReplay,/actual_process_abort_before_ref_cas_never_retries_history_rewrite/);
  assert.match(protectedReplay,/actual_process_abort_after_cas_can_only_reconcile_original_history_receipt/);
  const production=protectedReplay.split(/#\[cfg\(test\)\]\r?\nfn test_abort_disposable/)[0];
  for(const forbidden of ['std::process::Command','repo.push(','repo.fetch(',
    'repo.reset(','repo.checkout_head(','repo.rebase(','repo.cherrypick(']){
    assert.ok(!production.includes(forbidden),forbidden);
  }
});
test('6C04C/D separately gated native OS consent, no page-provided authority',()=>{
  for (const name of ['ide_git_preview_isolated_replay','ide_git_execute_cherry_pick',
    'ide_git_execute_rebase','ide_git_cancel_protected_replay',
    'ide_git_inspect_replay_recovery','ide_git_reconcile_replay_receipt']){
    assert.match(tauri,new RegExp('async fn '+name+'\\('));
    assert.match(tauri,new RegExp('            '+name+','));
  }
  assert.match(tauri,/EXTRA history-rewrite consent for local rebase/);
  assert.match(tauri,/REBASE REWRITES CURRENT LOCAL BRANCH HISTORY/);
  assert.match(tauri,/Authorize local history rewrite only/);
  assert.match(tauri,/Authorize this exact cherry-pick only/);
  assert.match(tauri,/app\.dialog\(\)\.message\(format!\(/);
  assert.match(tauri,/G06C04_HUMAN_DENIED/);
  assert.match(tauri,/sessions\.begin\(&project_id\)/);
  assert.match(tauri,/registry\.execute_os_approved_replay\(/);
  assert.match(tauri,/G06C04_RECONCILIATION_HUMAN_DENIED/);
});
test('6C04 result UI demands exact output OIDs and only native OS dialog can execute',()=>{
  assert.match(ui,/invoke\('ide_git_preview_isolated_replay'/);
  assert.match(ui,/invoke\('ide_git_execute_rebase'/);
  assert.match(ui,/ide_git_execute_cherry_pick/);
  assert.match(ui,/invoke\('ide_git_cancel_protected_replay'/);
  assert.match(ui,/invoke\('ide_git_inspect_replay_recovery'/);
  assert.match(ui,/invoke\('ide_git_reconcile_replay_receipt'/);
  assert.match(ui,/G06C04B_RESULT_UNTRUSTED_OR_STALE/);
  assert.match(ui,/G06C04_EXECUTION_RECEIPT_UNTRUSTED/);
  assert.match(ui,/result\.projectOdbModified !== false/);
  assert.match(ui,/result\.quarantineOnlyResultObjects !== true/);
  assert.match(ui,/receipt\.exactRefCas !== true/);
  assert.match(ui,/replayResultRequest\.current \+= 1/);
  assert.match(ui,/Rebase REWRITES the current local branch history/);
  assert.match(ui,/No automatic ref retry, history rewrite, checkout, rollback/);
  assert.match(plan,/G06C04_REBASE_MERGE_OR_ROOT_IN_RANGE/);
});