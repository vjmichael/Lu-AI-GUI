// SEC-05 source contracts supplement, NOT replace, hostile native execution.
// Kernel adversary must fail if an escaped child survives a positive receipt.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const load=(p)=>readFileSync(new URL(p,import.meta.url),'utf8');
const task=load('../../src-tauri/lu-core/src/tasks.rs');
const taskAttacks=load('../../src-tauri/lu-core/src/p0_sec05_task_adversarial_tests.rs');
const appAttacks=load('../src-tauri/src/p0_sec05_adversarial_tests.rs');
const cancel=load('../src-tauri/src/p0_auth_custody.rs');
const recovery=load('../src-tauri/src/p0_sec_recovery.rs');
const supervisor=load('../../src-tauri/lu-core/src/native_background_process.rs');
const rustFixture=load('../../src-tauri/lu-win-worker-job/src/bin/lu-p0-sec02-linux-negative.rs');
const kernelVerifier=load('../../benchmarks/e1/p0_sec02_linux_guardian_negative.py');
const cargo=load('../../src-tauri/lu-win-worker-job/Cargo.toml');

test('SEC05 destructive kernel attack remains gated from release and model tools',()=>{
  assert.match(cargo,/name = "lu-p0-sec02-linux-negative"/);
  assert.match(cargo,/required-features = \["e1-p08-crash-fixture"\]/);
  assert.match(rustFixture,/ancestor-migration/);
  assert.match(rustFixture,/fixture_guardian_pid\(\)/);
  assert.match(kernelVerifier,/os\.pidfd_open\(escaped\)/);
  assert.match(kernelVerifier,/migration_target=target\.parent\/"cgroup.procs"/);
  assert.match(kernelVerifier,/signal\.pidfd_send_signal\(escaped_fd,signal\.SIGKILL\)/);
});
test('SEC05 migrated setsid survivor cannot masquerade as independent kernel custody',()=>{
  assert.match(rustFixture,/FAIL_ESCAPED_DESCENDANT_MINTED_FALSE_EMPTY_RECEIPT/);
  assert.match(kernelVerifier,/SEC05 MIGRATION SECURITY FAILURE/);
  assert.match(kernelVerifier,/guardian loss or migrated descendant/);
  assert.match(kernelVerifier,/MIGRATION_ESCAPE_UNVERIFIED/);
  assert.match(kernelVerifier,/MIGRATION_DENIED_NATIVE_CGROUP_EMPTY/);
  assert.match(kernelVerifier,/unrelated control process was terminated/);
});
test('SEC05 Linux and Windows use actual separate OS workers and scoped stop',()=>{
  assert.match(supervisor,/sec05_windows_two_jobs_cross_chat_cancel_cannot_terminate_foreign_worker/);
  assert.match(supervisor,/assert_eq!\(manager\.cancel_owned\("sec05-job-b",&a\)/);
  assert.match(supervisor,/assert!\(manager\.can_cancel\("sec05-job-b"\)\)/);
  assert.match(supervisor,/cancel_workspace_owned\(&a\)/);
});
test('SEC05 immutable task custody, origin and receipt resist direct SQLite writes',()=>{
  for(const name of [
    'lu_task_uncertainty_state_fence',
    'lu_task_uncertainty_parent_fence',
    'lu_task_uncertainty_identity_fence',
    'lu_task_uncertain_event_no_update',
    'lu_task_uncertain_event_no_delete'
  ]) assert.ok(task.includes(name),'missing '+name);
  assert.match(taskAttacks,/sec05_sqlite_writer_cannot_forge_verified_terminal_or_erase_uncertain_task/);
  assert.match(taskAttacks,/UPDATE lu_tasks SET task_id='sec05-forged'/);
  assert.match(taskAttacks,/UPDATE lu_task_events SET event='COMPLETED'/);
  assert.match(taskAttacks,/sec05_bounded_workspace_coverage_never_misattributes_or_discards_unknowns/);
  assert.match(taskAttacks,/sec05_interrupted_without_custody_row_is_not_automatic_restart_permission/);
});
test('SEC05 malformed cancel intent and forged cross-chat outcome fail closed',()=>{
  assert.match(cancel,/P0_SEC_CANCEL_INTENT_TAMPERED/);
  assert.match(cancel,/P0_SEC_CANCEL_ORPHANED_RESULT/);
  assert.match(cancel,/sec05_direct_sql_forged_result_without_original_request_denies_tool_dispatch/);
  assert.match(cancel,/ansf_p0_cancel_no_update/);
  assert.match(cancel,/ansf_p0_cancel_no_direct_delete/);
  assert.match(cancel,/sec05_cancel_intent_mutations_and_direct_deletes_are_rejected/);
  assert.match(cancel,/sec05_cross_chat_forged_result_cannot_close_another_cancel_request/);
  assert.match(cancel,/requested_watermark\(&conn,&chat\)\.is_err\(\)/);
});
test('SEC05 ANSF revocation collisions cannot replay or affect another chat',()=>{
  assert.match(appAttacks,/sec05_32_concurrent_revoke_complete_collisions_and_cross_chat_isolation/);
  assert.match(appAttacks,/Barrier::new\(3\)/);
  assert.match(appAttacks,/complete_reviewed_if_not_revoked/);
  assert.match(appAttacks,/assert_eq!\(chats\.p0_auth_requested_watermark\(&b\)\.unwrap\(\),b_before\)/);
  assert.match(appAttacks,/sec05_saturated_cancellation_history_is_not_an_all_clear/);
});
test('SEC05 damaged or foreign journal cannot become clean original recovery',()=>{
  assert.match(appAttacks,/sec05_wrong_chat_origin_and_corrupted_original_task_never_leak_workers/);
  assert.match(appAttacks,/sec05_corrupt_effect_journal_and_interrupted_startup_fail_closed/);
  assert.match(appAttacks,/sec05_symlinked_original_journal_cannot_read_external_target/);
  assert.match(appAttacks,/ORIGINAL_AUTHORITY_BINDING_UNVERIFIED/);
  assert.match(recovery,/ORIGINAL_SECURITY_RECOVERY_UNAVAILABLE_RECONCILE/);
  assert.match(appAttacks,/assert!\(unavailable\.human_reconciliation_required\)/);
});
