import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(p)=>readFileSync(new URL(p,import.meta.url),'utf8');
const helper=read('../../src-tauri/lu-win-worker-job/src/bin/lu-linux-cgroup-guard.rs');
const native=read('../../src-tauri/lu-win-worker-job/src/lib.rs');
const host=read('../../src-tauri/lu-core/src/native_background_process.rs');
const receiver=read('../../src-tauri/lu-core/src/agent_os_custody.rs');
const desktop=read('../src-tauri/src/reviewed_app_desktop.rs');
const fixture=read('../../src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs');
const acceptance=read('../../benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py');

test('native OS custodian obtains only its actual /proc self cgroup without sudo',()=>{
  assert.match(helper,/\/proc\/self\/cgroup/);
  assert.match(helper,/0::/);
  assert.match(helper,/fs::create_dir\(&group\)/);
  assert.match(helper,/UNAVAILABLE/);
  assert.doesNotMatch(helper,/Command::new\("sudo"|systemd-run|pkexec|bash -c/);
});
test('custodian itself stays outside worker subtree and outlives parent SIGKILL',()=>{
  assert.match(helper,/itself is outside this path/);
  assert.match(helper,/stdin\.read\(&mut buf\)/);
  assert.match(helper,/Ok\(0\)=>break/);
  assert.match(helper,/fs::write\(path\.join\("cgroup\.kill"\),b"1"\)/);
  assert.match(helper,/populated_zero\(path\)/);
});
test('only kernel recursive populated-zero produces independently durable scoped receipt',()=>{
  assert.match(helper,/cgroup\.events/);
  assert.match(helper,/Some\("0"\)=>Some\(true\)/);
  assert.match(helper,/create_new\(true\)/);
  assert.match(helper,/mode\(0o600\)/);
  assert.match(helper,/file\.sync_all\(\)/);
  assert.match(helper,/File::open\(profile\)\?\.sync_all\(\)/);
  assert.match(helper,/NONCE=\{nonce\}/);
});
test('worker migrates into private cgroup before exec, never after untrusted code',()=>{
  assert.match(native,/fn bind_worker_to_cgroup_before_exec\(/);
  assert.match(native,/cmd\.pre_exec\(move\|\|/);
  assert.match(native,/group\.subtree\.join\("cgroup\.procs"\)/);
  assert.match(native,/write\(fd,b"0"\.as_ptr\(\)\.cast\(\),1\)/);
  assert.match(host,/bind_worker_to_cgroup_before_exec\(/);
  assert.match(host,/bind_owned_worker_pre_exec\(/);
});
test('one separately supervised custodian gets real host-only turn and profile',()=>{
  assert.match(host,/new_for_reviewed_host\(/);
  assert.match(host,/reviewed_human_origin\(\)/);
  assert.match(host,/origin\.1!=plan\.workspace_id/);
  assert.match(host,/LinuxCgroupCustodian::spawn_if_delegated\(/);
  assert.match(desktop,/new_for_reviewed_host\(/);
});
test('no delegation retains lower-assurance group but can never mint full-tree proof',()=>{
  assert.match(host,/typed_capture\.is_some\(\)&&cgroup\.is_none\(\)/);
  assert.match(native,/banner=="UNAVAILABLE\\n"/);
  assert.match(receiver,/linux_original_cgroup_empty\(/);
  assert.match(receiver,/LU_CGROUP_V2_EMPTY_V1/);
  assert.match(receiver,/saved_chat==chat/);
  assert.match(receiver,/saved_turn==turn/);
});
test('host death and ordinary completion both require actual detached custodian cleanup',()=>{
  assert.match(native,/pub fn stop_and_verify\(&mut self\)/);
  assert.match(native,/drop\(self\.stdin\.take\(\)\)/);
  assert.match(native,/self\.child\.try_wait\(\)\?/);
  assert.match(host,/guard\.stop_and_verify\(\)/);
  assert.match(native,/Cgroup custodian cleanup timed out/);
});
test('real Linux escaped setsid grandchild crash test proves protected process ended',()=>{
  assert.match(fixture,/start_new_session=True/);
  assert.match(fixture,/bind_worker_to_cgroup_before_exec\(/);
  assert.match(acceptance,/os\.kill\(host\.pid,signal\.SIGKILL\)/);
  assert.match(acceptance,/os\.pidfd_open\(escaped_pid\)/);
  assert.match(acceptance,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
  assert.match(acceptance,/AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1/);
  assert.match(acceptance,/unrelated_process_survived=1/);
});
test('cgroup receipt never acts as tool grant or automatic task replay',()=>{
  assert.doesNotMatch(receiver,/ConversationAuthority::new|TaskAuthority\{|\.spawn\(/);
  assert.doesNotMatch(helper,/TaskAuthority|open_user_task|run_journaled_observed/);
  assert.match(receiver,/state=="COMPLETED"/);
  assert.match(receiver,/lines\[5\]!="POPULATED=0"/);
});
