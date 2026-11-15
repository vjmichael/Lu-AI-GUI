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
const rootManager=read('../../scripts/lu-sec06c-owned-kernel-manager.py');
const osGit=read('../../src-tauri/lu-win-worker-job/src/os_owned_git.rs');
const osProbe=read('../../src-tauri/lu-win-worker-job/src/os_owned_probe.rs');
const osNative=read('../../src-tauri/lu-win-worker-job/src/os_owned_native.rs');

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
  assert.match(fixture,/bind_worker_to_cgroup_before_exec\(/);
  assert.match(rootManager,/stage="cgroup-migrate"/);
  assert.match(rootManager,/preexec_fn=lambda:preexec_leaf\(leaf,args\.uid,user\.pw_gid\)/);
  assert.doesNotMatch(host,/bind_worker_to_cgroup_before_exec\(/);
  assert.match(host,/bind_owned_worker_pre_exec\(/);
});
test('one separately supervised custodian gets real host-only turn and profile',()=>{
  assert.match(host,/new_for_reviewed_host\(/);
  assert.match(host,/reviewed_human_origin\(\)/);
  // The production host destructures authenticated (chat, turn) and then
  // rejects a different workspace before spawning the delegated worker.
  assert.match(host,/reviewed_human_origin\(\)[\s\S]{0,400}if turn!=plan\.workspace_id/);
  assert.match(fixture,/LinuxCgroupCustodian::spawn_if_delegated\(/);
  assert.doesNotMatch(host,/LinuxCgroupCustodian::spawn_if_delegated\(/);
  assert.match(host,/LinuxOsOwnedReviewedGit::spawn_if_installed\(/);
  assert.match(host,/LinuxOsOwnedReviewedProbe::spawn_if_installed\(/);
  assert.match(host,/LinuxOsOwnedReviewedNative::spawn_if_installed\(/);
  assert.match(desktop,/new_for_reviewed_host\(/);
});
test('missing SEC06C systemd broker denies reviewed work and old profile receipt proves nothing',()=>{
  assert.match(host,/os_owned_git_spawn\.is_none\(\)&&os_owned_probe_spawn\.is_none\(\)/);
  assert.match(host,/os_owned_native_spawn\.is_none\(\)/);
  assert.match(host,/self\.store\.fail_pending\(&plan\.task_id\)/);
  assert.match(host,/BackgroundProcessError::SpawnFailed/);
  assert.doesNotMatch(host,/spawn_if_delegated|LinuxOwnedGroupGuardian/);
  assert.match(native,/banner=="UNAVAILABLE\\n"/); // Legacy destructive fixture only.
  assert.match(receiver,/linux_original_cgroup_empty\(/);
  assert.doesNotMatch(receiver,/LU_CGROUP_V2_EMPTY_V1/);
  assert.match(receiver,/linux_os_owned_git_original_receipt/);
  assert.match(receiver,/linux_os_owned_probe_original_receipt/);
  assert.match(receiver,/linux_os_owned_native_original_receipt/);
  assert.match(receiver,/saved_chat==chat/);
  assert.match(receiver,/saved_turn==turn/);
});
test('host death and ordinary completion both require actual detached custodian cleanup',()=>{
  assert.match(native,/pub fn stop_and_verify\(&mut self\)/);
  assert.match(native,/drop\(self\.stdin\.take\(\)\)/);
  assert.match(native,/self\.child\.try_wait\(\)\?/);
  assert.match(osGit,/os_unit|stop_and_verify/);
  assert.match(host,/os_unit\.stop_and_verify\(&mut worker\.child\)/);
  assert.match(host,/native\.stop_and_verify\(&mut worker\.child\)/);
  assert.match(rootManager,/wait_kernel_zero\(events,15\)/);
  assert.match(rootManager,/original_root_file\(receipt,evidence\)/);
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
  assert.match(receiver,/linux_os_owned_native_original_receipt/);
  assert.match(rootManager,/KERNEL_EMPTY=1/);
});

test('SEC06C refuses writable same-UID ancestor before first worker admission and rechecks before receipt',()=>{
  assert.match(helper,/fn protected_ancestor\(root:&Path\)/);
  assert.match(helper,/meta\.uid\(\)==unsafe\{geteuid\(\)\}/);
  assert.match(helper,/OpenOptions::new\(\)\.write\(true\)\.open\(&procs\)/);
  const source=helper.indexOf('if !protected_ancestor(&group_root)?');
  const admitted=helper.indexOf('writeln!(io::stdout(),"READY');
  const finalCheck=helper.lastIndexOf('if !protected_ancestor(&group_root)?');
  const receipt=helper.indexOf('save_receipt(&profile,chat,turn,call_id,&nonce)?');
  assert.ok(source>=0&&admitted>source,'admission must be blocked before READY');
  assert.ok(finalCheck>admitted&&receipt>finalCheck,
    'final witness must reject writable ancestor before writing receipt');
  assert.match(helper,/UNSAFE_ANCESTOR/);
  assert.match(native,/SEC06C_BLOCKED_UNSAFE_WRITABLE_ANCESTOR/);
  assert.match(native,/fn securely_owned_cgroup_parent\(/);
  assert.match(native,/meta\.uid\(\)==uid/);
  assert.match(native,/if !securely_owned_cgroup_parent\(&group\)?/);
});

test('native cgroup preflight reads only original kernel facts and blocks unsafe same-UID migration',()=>{
  const preflight=read('../../scripts/lu-agent01l-native-cgroup-preflight.py');
  assert.match(preflight,/\/proc\/self\/cgroup/);
  assert.match(preflight,/\/proc\/self\/mountinfo/);
  assert.match(preflight,/SOURCE_REQUIRES_ROOT_MEDIATED_ADMISSION/);
  assert.match(preflight,/UNSAFE_WRITABLE_ANCESTOR/);
  assert.match(preflight,/parent_procs_write_open_succeeded/);
  assert.match(preflight,/changed_kernel_state": False/);
  assert.match(preflight,/positive_cgroup_custody_accepted": False/);
  assert.doesNotMatch(preflight,/os\.kill\(|subprocess\.|os\.setuid\(|os\.mkdir\(|os\.chown\(|os\.chmod\(/);
  assert.match(native,/fn bind_worker_to_cgroup_before_exec\(/);
  assert.match(helper,/fn protected_ancestor\(root:&Path\)/);
  assert.match(helper,/OpenOptions::new\(\)\.write\(true\)\.open\(&procs\)/);
});

const rootFixture=read('../../scripts/lu-sec06c-agent01l-fixture-manager.py');
const fixedLauncher=read('../../scripts/lu-sec06c-agent01l-fixture-launcher.py');
const adminInstaller=read('../../scripts/install-lu-agent01l-native-fixture.sh');

test('AGENT-01L positive native fixture uses only fixed SEC-06C root-owned systemd service',()=>{
  assert.match(fixture,/lu-sec06c-launch-agent01l-fixture/);
  assert.match(fixture,/LU_AGENT01L_REQUIRE_OS_OWNED_FIXTURE/);
  assert.match(fixture,/Command::new\("\/usr\/bin\/sudo"\)/);
  assert.match(fixedLauncher,/SUDO_UID/);
  assert.match(fixedLauncher,/HOST_MANIFEST/);
  assert.match(fixedLauncher,/lu-agent01l-fixture-host\.sha256/);
  assert.match(fixedLauncher,/\/proc\/\{pid\}\/exe/);
  assert.match(fixedLauncher,/h\.hexdigest\(\)!=pinned\[:-1\]/);
  assert.match(fixedLauncher,/lu-e1-p08-linux-cgroup-crash-parent/);
  assert.match(fixedLauncher,/lu-sec06c-fixture-/);
  assert.match(fixedLauncher,/AGENT01L_ROOT_SERVICE_DIAGNOSTIC_V1/);
  assert.match(fixedLauncher,/MAX_SERVICE_STDERR=4096/);
  assert.match(fixedLauncher,/os.O_EXCL\|os.O_NOFOLLOW/);
  for(const required of ['--property=User=root','--property=Delegate=yes',
      '--property=PrivateNetwork=yes','--property=KillMode=control-group']){
    assert.ok(fixedLauncher.includes(required),required);
  }
  assert.match(fixedLauncher,/str\(FIXTURE_MANAGER\)/);
  assert.doesNotMatch(fixedLauncher,/shell=True|os\.system\(|eval\(/);
  assert.match(adminInstaller,/git hash-object/);
  assert.match(adminInstaller,/visudo -cf/);
  assert.match(adminInstaller,/AGENT01L_POSITIVE_KERNEL_ACCEPTANCE=OPEN/);
});

test('root manager migrates known original leader BEFORE relying on nonroot cgroup custody',()=>{
  assert.match(rootFixture,/shared\.first_party_unit\(args\.nonce,"fixture"\)/);
  assert.match(rootFixture,/shared\.preexec_leaf\(nested,args\.uid,user\.pw_gid\)/);
  assert.match(rootFixture,/--attack-leaf/);
  assert.match(rootFixture,/SEC06C_ORIGINAL_SAME_UID_ANCESTOR_ATTACK_BLOCKED/);
  assert.match(rootFixture,/parent_of\(child\)!=leader\.pid/);
  const attack=rootFixture.indexOf('attacker=subprocess.Popen');
  const child=rootFixture.indexOf('leader=subprocess.Popen');
  const moved=rootFixture.indexOf('(leaf/"cgroup.procs").write_text(str(leader.pid)');
  const proof=rootFixture.indexOf('result="\\n".join([');
  assert.ok(attack>=0&&attack<child&&child<moved&&moved<proof,
    'root first proves hostile-ancestor denial, creates descendant and migrates only its leader before READY');
  assert.match(rootFixture,/os\.pidfd_open\(pid\)/);
  assert.match(rootFixture,/hostpoll\.register\(hostfd,select\.POLLIN\)/);
  assert.match(rootFixture,/\(leaf\/"cgroup\.kill"\)\.write_text\("1",encoding="ascii"\)/);
  assert.match(rootFixture,/shared\.wait_kernel_zero\(events,8\)/);
  assert.match(rootFixture,/nested\.rmdir\(\)/);
  assert.match(rootFixture,/leaf\.rmdir\(\)/);
  assert.match(rootFixture,/shared\.original_root_file\(receipt,/);
  assert.match(rootFixture,/\/var\/lib\/lu-sec06c\/fixture-receipts/);
  assert.doesNotMatch(rootFixture,/shell=True|os\.system\(|eval\(/);
});

test('old original destructive acceptance and native Git/restart fixtures remain unmodified contracts',()=>{
  assert.match(acceptance,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
  assert.match(acceptance,/os\.kill\(host\.pid,signal\.SIGKILL\)/);
  assert.match(acceptance,/os\.pidfd_open\(escaped_pid\)/);
  assert.match(acceptance,/recursive_populated_zero=1/);
  assert.match(host,/fn agent01l_linux_real_git_cgroup_custody_after_restart\(\)/);
  assert.match(host,/HostOriginalJobCustody::from_host_profile\(&root\)/);
  // Original legacy fixture source remains available for historical
  // comparisons, but a native positive requires the root-owned service.
  assert.match(fixture,/LinuxCgroupCustodian::spawn_if_delegated\(/);
  assert.match(fixture,/bind_worker_to_cgroup_before_exec\(/);
  assert.match(fixture,/fs::write\(&output,"NOT_AVAILABLE\\n"\)/);
});
