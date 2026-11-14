import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const load=(p)=>readFileSync(new URL(p,import.meta.url),'utf8');
const native=load('../../src-tauri/lu-win-worker-job/src/lib.rs');
const supervisor=load('../../src-tauri/lu-core/src/native_background_process.rs');
const rootManager=load('../../scripts/lu-sec06c-owned-kernel-manager.py');
const osGit=load('../../src-tauri/lu-win-worker-job/src/os_owned_git.rs');
const osProbe=load('../../src-tauri/lu-win-worker-job/src/os_owned_probe.rs');
const osNative=load('../../src-tauri/lu-win-worker-job/src/os_owned_native.rs');
const fixture=load('../../src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs');

test('Linux guardian handshakes are bounded and cannot strand stdout reader threads',()=>{
  assert.match(native,/fn bounded_guardian_banner\(/);
  assert.match(native,/fn poll\(fds:\*mut PollFd/);
  assert.match(native,/Linux guardian readiness deadline exceeded/);
  assert.match(native,/fn bounded_guardian_exit\(/);
  assert.doesNotMatch(native,/\.read_exact\(&mut banner\)|reader\.read_line\(&mut banner\)/);
});
test('failed readiness cleans up the original child and tests silent/invalid replies',()=>{
  assert.match(native,/struct AdmissionChild\(Option<Child>\)/);
  assert.match(native,/impl Drop for AdmissionChild/);
  assert.match(native,/let _=child\.kill\(\)/);
  assert.match(native,/match child\.try_wait\(\)/);
  assert.match(native,/let deadline=std::time::Instant::now\(\)/);
  assert.match(native,/still_alive_silent_guardian_cannot_block_admission/);
  assert.match(native,/truncated_and_malformed_guardian_banners_are_not_ready/);
  assert.match(native,/unavailable_helper_must_exit_before_no_delegation_result/);
});
test('real worker and custodian kernel membership are verified before RUNNING',()=>{
  assert.match(native,/pub fn require_live_guardian\(&mut self\)/);
  assert.match(native,/P0_SEC_CGROUP_GUARDIAN_LOST_BEFORE_SHUTDOWN/);
  assert.match(native,/pub fn verify_admitted_worker\(&mut self,pid:u32\)/);
  assert.match(native,/group_of\(pid\)\?!=expected/);
  assert.match(native,/guardian==expected\|\|guardian\.starts_with/);
  assert.match(rootManager,/child\.pid<=1 or pathlib\.Path\(f"\/proc\/\{child\.pid\}\/cgroup"\)/);
  assert.match(rootManager,/preexec_fn=lambda:preexec_leaf\(leaf,args\.uid,user\.pw_gid\)/);
  assert.doesNotMatch(supervisor,/guard\.verify_admitted_worker\(child\.id\(\)\)/);
  assert.match(fixture,/custodian\.verify_admitted_worker\(worker\.id\(\)\)/);
});
test('retired guardian cannot issue success; missing SEC06C root-owned receipt is process-uncertain',()=>{
  assert.doesNotMatch(supervisor,/guard\.require_live_guardian\(\)\.is_err\(\)/);
  assert.match(supervisor,/os_unit\.verify_original_kernel_receipt\(\)\.is_err\(\)/);
  assert.match(supervisor,/record_process_uncertain\(/);
  assert.match(supervisor,/return Err\(BackgroundProcessError::ProcessUncertain\)/);
  assert.match(rootManager,/if not kernel_verified:refuse\("kernel leaf was never verified empty"\)/);
  assert.match(fixture,/custodian\.verify_admitted_worker\(worker\.id\(\)\)/);
});
test('OS cleanup precedes removal of exact task owner in poll',()=>{
  const loop=supervisor.indexOf('for (id,worker) in workers.iter_mut()');
  const remove=supervisor.indexOf('let mut owner=workers.remove(&id)');
  const git=supervisor.indexOf('os_unit.verify_original_kernel_receipt()',loop);
  const probe=supervisor.indexOf('worker.os_owned_probe.as_mut()',git);
  const native=supervisor.indexOf('worker.os_owned_native.as_mut()',probe);
  assert.ok(loop>=0 && git>loop && probe>git && native>probe && remove>native,
    'kernel-empty receipt must precede removal of original task ownership');
  for(const proof of [osGit,osProbe,osNative]){
    assert.match(proof,/pub fn verify_original_kernel_receipt\(&mut self\)/);
    assert.match(proof,/pub fn stop_and_verify\(&mut self,/);
    assert.match(proof,/KERNEL_EMPTY=1/);
  }
});

test('a failed OS stop cannot become verified on a later retry',()=>{
  assert.match(native,/verified_signal:bool/);
  assert.match(native,/verified_empty:bool/);
  assert.match(native,/if self\.verified_signal\{Ok\(\(\)\)\}/);
  assert.match(native,/if self\.verified_empty\{Ok\(\(\)\)\}/);
  assert.match(native,/failed_or_incomplete_guardian_stop_cannot_turn_into_success_on_retry/);
});
const tasks=load('../../src-tauri/lu-core/src/tasks.rs');
test('durable uncertainty blocks terminal statuses and persists across restart',()=>{
  assert.match(tasks,/CREATE TABLE IF NOT EXISTS lu_task_custody_uncertainty/);
  assert.match(tasks,/pub fn record_process_uncertain\(/);
  assert.match(tasks,/pub fn process_uncertainty\(/);
  assert.match(tasks,/Immutable OS custody uncertainty/);
  assert.match(tasks,/uncertainty_is_immutable_durable_and_blocks_false_terminal_success/);
  assert.match(tasks,/NOT EXISTS\(SELECT 1 FROM lu_task_custody_uncertainty/);
  assert.match(supervisor,/record_process_uncertain\(/);
  assert.match(supervisor,/OS_CUSTODY_UNVERIFIED/);
  assert.doesNotMatch(supervisor,/LINUX_GUARDIAN_LOST/);
});

const sec02=load('../../src-tauri/lu-win-worker-job/src/bin/lu-p0-sec02-linux-negative.rs');
const sec02Verifier=load('../../benchmarks/e1/p0_sec02_linux_guardian_negative.py');
const cargo=load('../../src-tauri/lu-win-worker-job/Cargo.toml');
test('SEC-02 executable destructive cases use real kernel fixture and PIDFD oracle',()=>{
  assert.match(cargo,/name = "lu-p0-sec02-linux-negative"/);
  for(const mode of ['guardian-death','guardian-stall','worker-stall']){
    assert.ok(sec02.includes('"'+mode+'"'));
    assert.ok(sec02Verifier.includes('"'+mode+'"'));
  }
  assert.match(sec02,/custody\.verify_admitted_worker\(worker\.id\(\)\)/);
  assert.match(sec02,/custody\.fixture_guardian_pid\(\)/);
  assert.match(sec02,/start_new_session=True/);
  assert.match(sec02Verifier,/os\.pidfd_open\(guard\)/);
  assert.match(sec02Verifier,/signal\.pidfd_send_signal\(guard_fd,signal\.SIGKILL\)/);
  assert.match(sec02Verifier,/signal\.pidfd_send_signal\(guard_fd,signal\.SIGSTOP\)/);
  assert.match(sec02Verifier,/GUARDIAN_LOST_UNVERIFIED/);
  assert.match(sec02Verifier,/GUARDIAN_STALLED_UNVERIFIED/);
  assert.match(sec02Verifier,/WORKER_STALL_NATIVE_CGROUP_EMPTY/);
  assert.match(sec02Verifier,/unrelated_survived=1/);
});
const anfs=load('../src-tauri/src/ansf_chat_store.rs');
const revocations=load('../src-tauri/src/p0_auth_custody.rs');
const reviewed=load('../src-tauri/src/reviewed_app_desktop.rs');
test('SEC-03 durable revocation is ordered with native reviewed publication',()=>{
  assert.match(revocations,/fn requested_watermark\(/);
  assert.match(anfs,/fn complete_reviewed_if_not_revoked\(/);
  assert.match(anfs,/sec03_revoke_before_completion_denies_old_turn_but_not_fresh_approval/);
  assert.match(anfs,/sec03_concurrent_native_revoke_and_completion_have_one_catalog_order/);
  assert.match(reviewed,/let revocation_watermark=host\.chats\.p0_auth_requested_watermark/);
  assert.match(reviewed,/complete_reviewed_if_not_revoked\(/);
  assert.match(reviewed,/reviewed app original permission revoked during tool dispatch/);
  assert.match(reviewed,/if self\.cancellation\.is_cancelled\(\)/);
});
test('SEC-03 host process cancel and poll collide on actual original PID once',()=>{
  assert.match(supervisor,/sec03_real_os_cancel_poll_collision_has_one_terminal_receipt/);
  assert.match(supervisor,/a\.cancel_owned\("sec03-sleep","c-human"\)/);
  assert.match(supervisor,/a\.poll\(\)/);
  assert.match(supervisor,/assert_eq!\(terminal\.len\(\),1,/);
});

const sec04Core=load('../../src-tauri/lu-core/src/tasks.rs');
const sec04View=load('../src-tauri/src/p0_sec_recovery.rs');
const sec04Read=load('../src-tauri/src/reviewed_task_continuity.rs');
const sec04Host=load('../src-tauri/src/p0_auth_desktop.rs');
const desktopLib=load('../src-tauri/src/lib.rs');
const desktopUI=load('../web/main.jsx');

test('SEC-04 original uncertain PENDING and RUNNING states remain fenced on restart',()=>{
  assert.match(sec04Core,/WHERE task_id=\?1 AND state='PAUSED'\s+AND NOT EXISTS/);
  assert.match(sec04Core,/if matches!\(to,TaskState::Running\|TaskState::Completed/);
  assert.match(sec04Core,/WHERE state='RUNNING' OR \(state='PENDING' AND EXISTS\(/);
  assert.match(sec04Core,/pub fn security_recovery_rows\(/);
  assert.match(sec04Core,/An old interruption record is never a credential to restart work/);
  assert.match(sec04Core,/LEFT JOIN lu_task_custody_uncertainty/);
  assert.match(sec04View,/sec04_pending_uncertainty_is_interrupted_and_unresumable/);
});

test('SEC-04 original ANSF turn and read-only execution journals bind uncertainty to owner',()=>{
  assert.match(sec04Read,/fn sec04_reviewed_turn_security\(/);
  assert.match(sec04Read,/Connection::open_with_flags\(path,flags\)/);
  assert.match(sec04Read,/lu_reviewed_turn_origin/);
  assert.match(sec04Read,/ORIGINAL_ORIGIN_MISMATCH/);
  assert.match(sec04Read,/lu_task_custody_uncertainty/);
  assert.match(sec04View,/sec04_original_guardian_loss_effect_and_restart_are_never_cleared/);
  assert.match(sec04View,/reconcile_reviewed_application_journals/);
  assert.match(sec04View,/HISTORICAL_EFFECTS_REQUIRE_RECONCILIATION/);
});

test('SEC-04 original cancellation and effects shown in operator-owned chat UI only',()=>{
  assert.match(desktopLib,/mod p0_sec_recovery;/);
  assert.match(sec04Host,/pub security_recovery:crate::p0_sec_recovery::SecurityRecoveryView/);
  assert.match(sec04Host,/security_recovery,/);
  assert.match(sec04View,/chats.require\(chat\)\?/);
  assert.match(sec04View,/chats.prior_authenticated_turn_ids/);
  assert.match(sec04View,/CANCELLATION_REQUESTED_OUTCOME_UNKNOWN/);
  assert.match(sec04View,/ORIGINAL_SECURITY_RECOVERY_UNAVAILABLE_RECONCILE/);
  assert.match(desktopUI,/SEC-04 · Original security recovery/);
  assert.match(desktopUI,/Original security exceptions/);
  assert.match(desktopUI,/Automatic replay: DISALLOWED/);
  assert.match(sec04View,/prior_authority_reusable:false,automatic_retry_allowed:false/);
});
