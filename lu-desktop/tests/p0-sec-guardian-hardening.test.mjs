import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const load=(p)=>readFileSync(new URL(p,import.meta.url),'utf8');
const native=load('../../src-tauri/lu-win-worker-job/src/lib.rs');
const supervisor=load('../../src-tauri/lu-core/src/native_background_process.rs');
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
  assert.match(native,/let _=child\.wait\(\)/);
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
  assert.match(supervisor,/guard\.verify_admitted_worker\(child\.id\(\)\)/);
  assert.match(fixture,/custodian\.verify_admitted_worker\(worker\.id\(\)\)/);
});
test('premature guardian death is process-uncertain, never worker completion',()=>{
  assert.match(supervisor,/guard\.require_live_guardian\(\)\.is_err\(\)/);
  assert.match(supervisor,/let _=worker\.child\.kill\(\)/);
  assert.match(supervisor,/return Err\(BackgroundProcessError::ProcessUncertain\)/);
});
test('OS cleanup precedes removal of exact task owner in poll',()=>{
  const loop=supervisor.indexOf('for (id,worker) in workers.iter_mut()');
  const remove=supervisor.indexOf('let mut owner=workers.remove(&id)');
  const stop=supervisor.indexOf('guard.stop_and_verify()',loop);
  assert.ok(loop>=0 && stop>loop && remove>stop);
  assert.match(supervisor,/Never remove a worker from the owner map/);
});
