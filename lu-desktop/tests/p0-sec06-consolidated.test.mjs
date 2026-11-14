// SEC-06: never silently convert source or fake native receipts into
// OS custody or installed-human security acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {canonicalSourceLockSha256} from '../../scripts/sec06-source-lock-digest.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const gate=readFileSync(join(root,'scripts/p0-sec06-consolidated.mjs'),'utf8');
const guardian=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/lib.rs'),'utf8');
const negative=readFileSync(join(root,'benchmarks/e1/p0_sec02_linux_guardian_negative.py'),'utf8');
const run=(args)=>spawnSync(process.execPath,[
  'scripts/p0-sec06-consolidated.mjs','--mode','verify',...args
],{cwd:root,encoding:'utf8',timeout:30000});

test('SEC06 refuses absent OS and physical owner proof, not empty success',()=>{
  const r=run([]);
  assert.equal(r.status,1,r.stderr);
  const output=JSON.parse(r.stdout);
  assert.equal(output.securityDecision,'BLOCKED_MISSING_OR_UNTRUSTED_NATIVE_PROOFS');
  assert.equal(output.ownerOperatedInstalledAppAccepted,false);
  assert.equal(output.releaseOrMergeAuthorized,false);
  for(const key of ['source','linux','windows']){
    assert.equal(output.proofs[key].status,'NOT_AVAILABLE');
  }
});
test('SEC06 rejects deliberately forged native receipt even with passing case labels',()=>{
  const temp=mkdtempSync(join(tmpdir(),'lu-sec06-forged-'));
  try{
    const file=join(temp,'forged.json');
    writeFileSync(file,JSON.stringify({
      schema:'lu-sec06-native-evidence-v1',target:'linux',
      head:'a'.repeat(40),sourceLockSha256:'b'.repeat(64),
      status:'PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING',
      allNativeCasesPassed:true,
      machine:{platform:'linux'},cases:[]
    }),{mode:0o600});
    const r=run(['--linux',file,'--source',file,'--windows',file]);
    assert.equal(r.status,1,r.stderr);
    const result=JSON.parse(r.stdout);
    assert.equal(result.securityDecision,'BLOCKED_MISSING_OR_UNTRUSTED_NATIVE_PROOFS');
    assert.equal(result.proofs.linux.status,'UNTRUSTED_OR_INCOMPLETE');
  }finally{rmSync(temp,{recursive:true,force:true})}
});
test('SEC06 requires specific native cases and re-hashes original logs',()=>{
  assert.match(gate,/sourceLockSha256/);
  assert.match(gate,/required\.some\(id=>!p\.cases\.some/);
  assert.match(gate,/hash\(readFileSync\(pathToLog\)\)!==entry\.sha256/);
  assert.match(gate,/windows-real-two-jobs/);
  assert.match(gate,/linux-ancestor-migration/);
  assert.match(gate,/BLOCKED_MISSING_OR_UNTRUSTED_NATIVE_PROOFS/);
  assert.match(gate,/SOURCE_CONTRACTS_PASS_NATIVE_UNPROVEN/);
  assert.match(gate,/ownerOperatedInstalledAppAccepted:false/);
  assert.match(gate,/process\.exitCode=1/);
});
test('SEC06 Linux original Core suite receives absolute host-pinned Git without relaxing cgroup gates',()=>{
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8').replace(/\r\n/g,'\n');
  const section=workflow.split('  core01e-sec06-native:\n')[1]
    ?.split('  p0-gate-installed-windows:')[0];
  assert.ok(section,'missing original SEC-06 job');
  const linuxPin=section.indexOf('Pin real Linux Git executable for SEC-06');
  const windowsPin=section.indexOf('Pin real Windows Git executable for reviewed native worker');
  const originalRun=section.indexOf('Run independent SEC-06 including Linux ancestor-migration containment');
  assert.ok(linuxPin>=0 && windowsPin>linuxPin && originalRun>windowsPin);
  for(const snippet of [
    'native="$(readlink -e -- "$selected")"',
    'test -f "$native" && test -x "$native"',
    'LU_TOOLS03_TEST_GIT_PATH=$native',
    'LU_TOOLS03_TEST_GIT_PATH=$path',
    'node scripts/p0-sec06-consolidated.mjs --mode'
  ]){
    assert.ok(section.includes(snippet),'Missing native admission contract '+snippet);
  }
  assert.equal(section.includes('SEC06_RELEASE_ACCEPTED=1'),false);
  assert.equal(section.includes('--skip-ancestor-migration'),false);
});

test('SEC06C fixed Git path binds original immutable journal nonce and finite nonroot worker',()=>{
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_git.rs'),'utf8');
  const tasks=readFileSync(join(root,'src-tauri/lu-core/src/tasks.rs'),'utf8');
  const historical=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-git-launcher.py'),'utf8');
  const worker=readFileSync(join(root,'scripts/lu-sec06c-owned-git-worker.py'),'utf8');
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  assert.match(host,/LinuxOsOwnedReviewedGit::spawn_if_installed/);
  assert.match(host,/bind_linux_os_owned_git_admission/);
  assert.match(host,/verify_original_kernel_receipt/);
  assert.match(host,/sec06c_root_owned_git_cancel_original_journal/);
  assert.match(host,/sec06c_owned_git_host_sigkill_recovery_original_journal/);
  assert.match(host,/reconcile_interrupted\(\)/);
  assert.match(host,/os_unit\.stop_and_verify/);
  assert.match(native,/\/usr\/libexec\/lu\/lu-sec06c-launch-git/);
  assert.match(native,/\/var\/lib\/lu-sec06c\/receipts/);
  assert.match(native,/SEC06C forbids arbitrary Git commands/);
  assert.match(native,/NOFOLLOW/);
  assert.match(tasks,/lu_task_linux_os_owned_git_admission/);
  assert.match(tasks,/Immutable Linux OS-owned admission/);
  assert.match(historical,/linux_os_owned_git_original_receipt/);
  assert.match(historical,/KERNEL_EMPTY=1/);
  for(const attr of [
    '--property=Delegate=yes','--property=User=root',
    '--property=KillMode=control-group','--property=PrivateNetwork=yes',
    '--property=NoNewPrivileges=yes','--property=TasksMax=32',
    '--property=MemoryMax=536870912','--property=RuntimeMaxSec=90',
    'MANAGER','root_owned_file(file)','SUDO_UID','str(MANAGER)',
    'SEC06C_OS_BROKER_DENIED'
  ])assert.ok(launcher.includes(attr),'missing fixed root-owned manager contract '+attr);
  assert.match(worker,/core\.fsmonitor=false/);
  assert.match(worker,/Popen/);
  assert.match(worker,/authorized_go\(args\.nonce\)/);
  assert.match(worker,/original Lu host died before one-use admission/);
  assert.match(worker,/original Lu host cancelled before reviewed Git spawn/);
  assert.match(worker,/exact separate root-owned worker leaf/);
  assert.match(launcher,/for file in \(pathlib\.Path\(__file__\)\.resolve\(\),WORKER,WORKER_PYTHON,/);
  assert.match(manager,/GIT_WORKER/);
  assert.match(manager,/PROBE_WORKER/);
  assert.match(manager,/OPS=\{/);
  assert.match(worker,/SEC06C_GIT_WORKER_BLOCKED/);
  assert.doesNotMatch(launcher,/shell=True|subprocess\.getoutput|os\.system\(/);
  assert.doesNotMatch(worker,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-git-syntax/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C trusted UID drop retains only supervisor CAP_SETUID and removes all worker capabilities',()=>{
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  for(const domain of ['git','probe','native']){
    const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-'+domain+'-launcher.py'),'utf8');
    assert.match(launcher,/--property=AmbientCapabilities=CAP_SETUID/);
    assert.match(launcher,/--property=CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_KILL/);
    assert.match(launcher,/--property=NoNewPrivileges=yes/);
    assert.match(launcher,/--property=RestrictSUIDSGID=yes/);
    assert.match(launcher,/--property=Delegate=yes/);
    assert.match(launcher,/--property=KillMode=control-group/);
    assert.doesNotMatch(launcher,/AmbientCapabilities=CAP_SYS_ADMIN|AmbientCapabilities=CAP_SETGID|AmbientCapabilities=CAP_KILL/);
  }
  assert.match(manager,/systemd supervisor lacks effective and permitted CAP_SETUID/);
  assert.match(manager,/PR_SET_KEEPCAPS=8/);
  assert.match(manager,/libc.prctl\(PR_SET_KEEPCAPS,0,0,0,0\)/);
  assert.match(manager,/verify-complete-nonroot-drop/);
  assert.match(manager,/os.getresuid\(\)!=\(uid,uid,uid\)/);
  assert.match(manager,/os.getresgid\(\)!=\(gid,gid,gid\)/);
  assert.match(manager,/\("CapEff","CapPrm","CapAmb"\)/);
  assert.match(manager,/SEC06C_ANCESTOR_ESCAPE_SUCCEEDED/);
  assert.match(manager,/leaf\/"cgroup.kill"/);
});

test('SEC06C trusted root supervisor pins the separate leaf and proves populated-zero while it exists',()=>{
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  assert.match(manager,/def first_party_unit\(/);
  assert.match(manager,/Delegate=yes/);
  assert.match(manager,/original systemd service or ancestor is not root-owned/);
  assert.match(manager,/def preexec_leaf\(/);
  assert.match(manager,/with \(leaf\/"cgroup\.procs"\)\.open\("w"/);
  assert.match(manager,/CLONE_NEWNS/);
  assert.match(manager,/MS_REMOUNT\|MS_BIND\|MS_RDONLY/);
  assert.match(manager,/PR_SET_NO_NEW_PRIVS/);
  assert.match(manager,/SEC06C_KERNEL_PREEXEC_DENIED stage=/);
  assert.match(manager,/errno=/);
  assert.match(manager,/stage="cgroup-migrate"/);
  assert.match(manager,/stage="unshare-private-mount"/);
  assert.match(manager,/stage="readonly-kernel-cgroup-mount"/);
  assert.match(manager,/stage="drop-user-identity"/);
  assert.match(manager,/os\.setgroups\(\[\]\)/);
  assert.match(manager,/os\.setgid\(gid\)/);
  assert.match(manager,/os\.setuid\(uid\)/);
  assert.match(manager,/def wait_kernel_zero\(/);
  assert.match(manager,/if not wait_kernel_zero\(events,15\)/);
  assert.match(manager,/leaf\/"cgroup\.kill"/);
  assert.match(manager,/os\.pidfd_open\(child\.pid\)/);
  assert.match(manager,/def negative_original_ancestor_probe\(/);
  assert.match(manager,/start_new_session=True/);
  assert.match(manager,/parent\/"cgroup\.procs"/);
  assert.match(manager,/SEC06C_ANCESTOR_ESCAPE_SUCCEEDED/);
  assert.match(manager,/attacker\.wait\(timeout=10\)/);
  assert.match(manager,/same-UID hostile-ancestor attack/);
  assert.match(manager,/original user\/call identity invalid|original chat\/turn hashes invalid/);
  assert.match(manager,/kernel_verified=True/);
  assert.match(manager,/if kernel_verified:leaf\.rmdir\(\)/);
  assert.match(manager,/original_root_file\(receipt,evidence\)/);
  assert.match(manager,/NoNewPrivileges|no new privileges/);
  assert.match(manager,/no arbitrary executable|No arbitrary executable/);
  const observed=manager.indexOf('kernel_verified=True');
  const removed=manager.indexOf('if kernel_verified:leaf.rmdir()');
  const receipt=manager.indexOf('original_root_file(receipt,evidence)');
  assert.ok(observed>=0&&removed>observed&&receipt>removed,
    'no release receipt before genuine kernel leaf emptiness and removal');
  assert.doesNotMatch(manager,/shell=True|os\.system\(/);
  assert.match(gate,/linux-guardian-death/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C finite reviewed FFprobe/PDFInfo/ExifTool remain separate non-generic OS operations',()=>{
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_probe.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-probe-launcher.py'),'utf8');
  const worker=readFileSync(join(root,'scripts/lu-sec06c-owned-probe-worker.py'),'utf8');
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const tasks=readFileSync(join(root,'src-tauri/lu-core/src/tasks.rs'),'utf8');
  const custody=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  assert.match(native,/SEC06C forbids arbitrary media\/metadata argv/);
  for(const item of ['ffprobe-wav','ffprobe-mp4','pdfinfo-pdf','exiftool-image']){
    assert.ok(native.includes(item));assert.ok(manager.includes(item));assert.ok(worker.includes(item));
  }
  assert.match(native,/\/usr\/libexec\/lu\/lu-sec06c-launch-probe/);
  assert.match(host,/LinuxOsOwnedReviewedProbe::spawn_if_installed/);
  assert.match(host,/bind_linux_os_owned_probe_admission/);
  assert.match(tasks,/Immutable Linux OS-owned probe admission/);
  assert.match(custody,/linux_os_owned_probe_original_receipt/);
  for(const attr of ['Delegate=yes','KillMode=control-group',
    'User=root','PrivateNetwork=yes','NoNewPrivileges=yes',
    'CapabilityBoundingSet=','TasksMax=32','MemoryMax=536870912',
    'secure_file','secure_directory','str(MANAGER)']){
    assert.ok(launcher.includes(attr),'missing finite root manager contract '+attr);
  }
  assert.match(worker,/os\.O_NOFOLLOW/);
  assert.match(worker,/pass_fds=\(fd,\)/);
  assert.match(worker,/hashlib\.sha256/);
  assert.match(worker,/select\.select/);
  assert.match(worker,/approved_first_launch/);
  assert.match(worker,/exact separate root-owned media leaf/);
  assert.match(manager,/input_resource\(/);
  assert.match(manager,/Root never opens user file bytes/);
  assert.doesNotMatch(launcher,/shell=True|os\.system\(/);
  assert.doesNotMatch(worker,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-probe-syntax/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C finite scanner archive and utility root-owned native contracts are not an arbitrary exec broker',()=>{
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_native.rs'),'utf8');
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const tasks=readFileSync(join(root,'src-tauri/lu-core/src/tasks.rs'),'utf8');
  const oracle=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-native-launcher.py'),'utf8');
  const worker=readFileSync(join(root,'scripts/lu-sec06c-owned-native-worker.py'),'utf8');
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  for(const kind of ['scan-file','archive-preview','archive-extract',
     'fixed-version','fixed-artifact','app-version']){
    for(const source of [native,launcher,worker,manager]){
      assert.ok(source.includes(kind),'missing native finite enum '+kind);
    }
  }
  assert.match(native,/finite_kind/);
  assert.match(native,/SEC06C native reviewed worker finite argv contract denied/);
  assert.match(native,/--lu-file-scan-worker/);
  assert.match(native,/--lu-archive-preview-worker/);
  assert.match(native,/--lu-archive-extract-worker/);
  assert.match(native,/--lu-e1-version-one-shot/);
  assert.match(native,/--lu-e1-artifact-one-shot/);
  assert.match(native,/NOFOLLOW/);
  assert.match(native,/\/usr\/libexec\/lu\/lu-sec06c-launch-native/);
  assert.match(launcher,/str\(MANAGER\)/);
  assert.match(launcher,/Delegate=yes/);
  assert.match(launcher,/Kernel|kernel/i);
  assert.match(launcher,/PrivateNetwork=yes/);
  assert.match(launcher,/RuntimeMaxSec=900/);
  assert.match(worker,/root_go/);
  assert.match(worker,/os\.getpid\(\)/);
  assert.match(worker,/original Lu host cancelled or died/);
  assert.match(worker,/SEC06C_NATIVE_MEDIATOR_DENIED/);
  assert.match(manager,/NATIVE_WORKER/);
  assert.match(manager,/finite_native/);
  assert.match(manager,/def negative_original_ancestor_probe/);
  assert.match(manager,/wait_kernel_zero/);
  assert.match(manager,/LU_SEC06C_OWNED_NATIVE_V1/);
  assert.match(tasks,/Immutable Linux OS-owned native admission/);
  assert.match(tasks,/bind_linux_os_owned_native_admission/);
  assert.match(host,/LinuxOsOwnedReviewedNative::spawn_if_installed/);
  assert.match(host,/os_owned_native/);
  assert.match(oracle,/linux_os_owned_native_original_receipt/);
  assert.doesNotMatch(launcher,/shell=True|os\.system\(/);
  assert.doesNotMatch(worker,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-reviewed-native-syntax/);
  assert.match(gate,/linux-ancestor-migration/);
  assert.match(gate,/linux-guardian-death/);
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8')
    .replace(/\r\n/g,'\n');
  assert.match(workflow,/core01e-sec06c-os-owned-native/);
  assert.match(workflow,/SEC06_REVIEWED_NATIVE_JOB/);
  assert.match(workflow,/sec06c_root_owned_scanner_utility_original_journal/);
});

test('SEC06C native one-shot admission cannot substitute for jointly owned headed Chromium and Node',()=>{
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_native.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-native-launcher.py'),'utf8');
  const manager=readFileSync(join(root,'scripts/lu-sec06c-owned-kernel-manager.py'),'utf8');
  const browser=readFileSync(join(root,'src-tauri/lu-core/src/browser_worker_supervisor.rs'),'utf8');
  const chromium=readFileSync(join(root,'src-tauri/lu-core/src/browser_chromium_launcher.rs'),'utf8');
  assert.doesNotMatch(native,/browser-chromium|browser-node|browser-launch/);
  assert.doesNotMatch(launcher,/browser-chromium|browser-node|browser-launch/);
  assert.doesNotMatch(manager,/"browser":\s*\{/);
  assert.match(browser,/Command::new\(&config\.trusted_node\)/);
  assert.match(chromium,/Command::new\(browser_executable\)/);
  assert.match(browser,/launch_attached/);
  assert.match(browser,/OwnedBrowserWindow/);
  // Production browser acceptance requires BOTH independent processes in the
  // same trusted OS custody boundary, not a Node-only generic privileged
  // launch, and cannot borrow a scanner/utility kernel receipt.
  assert.match(gate,/linux-guardian-death/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C root-owned boundary prototype must deny external same-UID migration and use OS group cleanup',()=>{
  const prototype=readFileSync(join(root,'benchmarks/e1/sec06c_root_owned_systemd_boundary.py'),'utf8');
  assert.match(prototype,/--property=Delegate=no/);
  assert.match(prototype,/--property=KillMode=control-group/);
  assert.match(prototype,/os\.get(e)?uid\(\) == 0/);
  assert.match(prototype,/start_new_session=True/);
  assert.match(prototype,/parent \/ "cgroup\.procs"/);
  assert.match(prototype,/signal\.pidfd_send_signal\(mainfd, signal\.SIGKILL\)/);
  assert.match(prototype,/pidfd_terminated\(childfd, 15\)/);
  assert.match(prototype,/SEC06C_ANCESTOR_ESCAPE_SUCCEEDED/);
  assert.match(prototype,/noReleaseOrInstallAuthority/);
  assert.match(gate,/linux-root-owned-unit-prototype/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C original ancestor escape forbids a false positive receipt even with real delegation',()=>{
  const helper=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/bin/lu-linux-cgroup-guard.rs'),'utf8');
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/lib.rs'),'utf8');
  assert.match(helper,/UNSAFE_ANCESTOR/);
  assert.match(helper,/SEC06C_BLOCKED_UNSAFE_WRITABLE_ANCESTOR/);
  assert.match(native,/SEC06C_BLOCKED_UNSAFE_WRITABLE_ANCESTOR/);
  assert.match(negative,/MIGRATION_ESCAPE_UNVERIFIED/);
  assert.match(negative,/SEC05 MIGRATION SECURITY FAILURE/);
  assert.match(negative,/migrated survivor MUST NEVER/);
  assert.match(negative,/SEC06_BLOCKED_ANCESTOR_CGROUP_ESCAPE=1/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C provisions real systemd delegation and never runs kernel cases as root',()=>{
  const python=readFileSync(join(root,'scripts/sec06-linux-delegated-runner.py'),'utf8');
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8').replace(/\r\n/g,'\n');
  const linux=workflow.split('  core01e-sec06-native:\n')[1]
    ?.split('  p0-gate-installed-windows:')[0];
  assert.ok(linux);
  assert.match(linux,/sec06-linux-delegated-runner\.py --outer/);
  assert.match(linux,/kernel-delegation\.json/);
  assert.match(python,/--property=Delegate=yes/);
  assert.match(python,/--property=User=/);
  assert.match(python,/os\.geteuid\(\) == 0/);
  assert.match(python,/os\.getxattr\(path, "user\.delegate"\)/);
  assert.match(python,/child\.mkdir\(\)/);
  assert.match(python,/cgroup\.kill/);
  assert.match(python,/ancestorCgroupProcsUserWritable/);
  assert.match(python,/SEC06C_BLOCKED: native case or ancestor containment failed/);
  assert.match(python,/return 77/);
  assert.doesNotMatch(python,/chmod\s+777|mount\s+-t\s+cgroup2/);
  assert.match(gate,/delegationWitnessSha256/);
  assert.match(gate,/missing or altered genuine Linux OS delegation witness/);
});

test('SEC06 actual kernel oracle treats escaped descendant as security blocker',()=>{
  assert.match(negative,/SEC06_BLOCKED_ANCESTOR_CGROUP_ESCAPE=1/);
  assert.match(negative,/SEC06_SECURITY_BLOCKER/);
  assert.match(negative,/raise AssertionError\(/);
  assert.match(gate,/mustReject:\['SEC06_BLOCKED_ANCESTOR_CGROUP_ESCAPE=1'\]/);
});
test('SEC06 positive custodian receipt requires exact original admission identity',()=>{
  assert.match(guardian,/fn validate_linux_cgroup_receipt\(/);
  assert.match(guardian,/expected_receipt:String/);
  assert.match(guardian,/Kernel cgroup witness identity or original nonce mismatch/);
  assert.match(guardian,/sec06_kernel_receipt_rejects_cross_chat_nonce_truncation_symlinks_and_mode/);
  assert.match(guardian,/O_NOFOLLOW/);
  assert.match(guardian,/bytes!=expected\.as_bytes\(\)/);
});
test('SEC06F independent receipt reconciliation preserves same SHA, all native OS gates and owner separation',()=>{
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8');
  // Windows checkout may use CRLF. Parse only the section marker; keep all
  // exact original native receipt and independent-owner assertions unchanged.
  const section=workflow.split(/  core01e-sec06f-original-receipts:\r?\n/)[1]
    ?.split('  p0-gate-installed-windows:')[0];
  assert.ok(section,'SEC-06F reconciliation job missing');
  assert.match(section,/core01e-sec06-native/);
  assert.match(section,/core01e-sec06c-os-owned-git/);
  assert.match(section,/core01e-sec06c-os-owned-probes/);
  assert.match(section,/actions\/download-artifact@v4/);
  assert.match(section,/lu-core01e-sec06-\*/);
  assert.match(section,/--source/);
  assert.match(section,/--linux/);
  assert.match(section,/--windows/);
  assert.match(section,/SEC06F_EXACT_HEAD_CHANGED/);
  assert.match(section,/SEC06F_ORIGINAL_LOG_OR_KERNEL_PROOF_BLOCKED/);
  assert.match(section,/SEC06F_BROKER_OR_NATIVE_GATE_BLOCKED/);
  assert.match(section,/NATIVE_CASES_PRESENT_OWNER_INSTALLED_AND_INDEPENDENT_REVIEW_PENDING/);
  assert.match(section,/SEC06F_OWNER_INSTALLED_AND_INDEPENDENT_REVIEW_REQUIRED/);
  assert.doesNotMatch(section,/LU_CORE01E_INSTALLED_SHA=|SEC06_RELEASE_ACCEPTED=1/);
});

test('SEC06 never auto-promotes source and native proofs to app release',()=>{
  assert.match(gate,/securityDecision=valid\?/);
  assert.match(gate,/NATIVE_CASES_PRESENT_OWNER_INSTALLED_AND_INDEPENDENT_REVIEW_PENDING/);
  assert.match(gate,/noReleaseAuthority:true/);
  assert.doesNotMatch(gate,/SEC06_RELEASE_ACCEPTED\s*=\s*true/);
});


test('SEC06 verifier rejects changed original native log with correct head and labels',()=>{
  const temp=mkdtempSync(join(tmpdir(),'lu-sec06-edited-log-'));
  try{
    const digest=(v)=>createHash('sha256').update(v).digest('hex');
    const current=spawnSync('git',['rev-parse','HEAD'],{
      cwd:root,encoding:'utf8',timeout:10000
    });
    assert.equal(current.status,0);
    const head=current.stdout.trim();
    const lockHash=canonicalSourceLockSha256(root);
    const required=[
      'exact-head-inventory','security-contracts','sec06-js-syntax',
      'linux-negative-python-syntax','linux-host-crash-python-syntax','linux-delegated-python-syntax',
      'linux-root-owned-prototype-syntax','linux-root-owned-unit-prototype',
      'linux-root-owned-unit-cancel','linux-root-owned-unit-host-crash',
      'linux-root-owned-unit-guardian-death',
      'linux-root-owned-unit-guardian-stall',
      'linux-root-owned-unit-worker-stall',
      'linux-root-owned-unit-ancestor-migration',
      'linux-retired-guardian-admission-denial',
      'linux-os-owned-git-syntax','linux-os-owned-probe-syntax',
      'linux-os-owned-kernel-manager-syntax',
      'linux-os-owned-reviewed-native-syntax',
      'core-native-full','desktop-ansf-native','worker-job-native',
      'core-sec-regressions','desktop-sec-regressions',
      'worker-sec-regressions',
      'native-file-scan-build','native-file-scan-e2e',
      'guardian-test-binaries','linux-original-host-sigkill',
      'linux-reviewed-git-original-journal',
      'linux-guardian-death','linux-guardian-stall',
      'linux-worker-stall','linux-ancestor-migration'
    ];
    const cases=required.map(id=>{
      const log=id+'.log';
      const bytes=Buffer.from('fixed original proof placeholder '+id+'\n');
      writeFileSync(join(temp,log),bytes);
      return {id,status:'PASS',log,sha256:digest(bytes)};
    });
    const delegation=Buffer.from(JSON.stringify({
      schema:'lu-sec06c-native-delegation-v1',
      status:'DELEGATED_KERNEL_PREFLIGHT_ONLY',
      head,euid:1000,realDelegation:true,systemdDelegateXattr:true,
      kernelCgroupKillPresent:true,probePopulatedZero:true,
      kernelTestAccepted:false,releaseOrInstallAuthorized:false
    }));
    writeFileSync(join(temp,'kernel-delegation.json'),delegation);
    const receipt=join(temp,'linux.receipt.json');
    writeFileSync(receipt,JSON.stringify({
      schema:'lu-sec06-native-evidence-v1',target:'linux',head,
      sourceLockSha256:lockHash,machine:{platform:'linux'},
      status:'PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING',
      allNativeCasesPassed:true,localExecutionOnly:true,
      delegationWitnessSha256:digest(delegation),
      noReleaseAuthority:true,
      independentlyVerifiedInstalledHumanAcceptance:false,
      cases
    }));
    // Even before corruption, this verifier only says 'UNREVIEWED',
    // never SEC06 release approved. The logs are not a cryptographic
    // machine attestation and still require independent human review.
    const unreviewed=run(['--linux',receipt]);
    assert.equal(unreviewed.status,1);
    assert.equal(JSON.parse(unreviewed.stdout).proofs.linux.status,
      'NATIVE_EVIDENCE_PRESENT_UNREVIEWED');
    writeFileSync(join(temp,'linux-ancestor-migration.log'),
      'MALICIOUSLY REPLACED ORIGINAL LOG\n');
    const modified=run(['--linux',receipt]);
    assert.equal(modified.status,1);
    assert.equal(JSON.parse(modified.stdout).proofs.linux.status,
      'UNTRUSTED_OR_INCOMPLETE');
  }finally{rmSync(temp,{recursive:true,force:true})}
});


test('SEC06 lock witness is identical for original Git blob on LF and Windows CRLF and rejects tampering',()=>{
  const repo=mkdtempSync(join(tmpdir(),'lu-sec06-lock-'));
  const git=(...args)=>{
    const r=spawnSync('git',['-C',repo,...args],{encoding:'utf8',timeout:30000});
    assert.equal(r.status,0,r.stderr);
  };
  try{
    git('init','-q');
    const lock=join(repo,'source-tree.lock.json');
    writeFileSync(lock,'{"schema":1}\n');
    git('-c','core.autocrlf=false','add','--','source-tree.lock.json');
    git('-c','user.name=Lu Test','-c','user.email=lu@example.invalid',
      'commit','--no-gpg-sign','-q','-m','original canonical source lock');
    const immutableDigest=canonicalSourceLockSha256(repo);
    writeFileSync(lock,'{"schema":1}\r\n');
    assert.equal(canonicalSourceLockSha256(repo),immutableDigest);
    writeFileSync(lock,'{"schema":2}\r\n');
    assert.throws(()=>canonicalSourceLockSha256(repo),/SEC06_SOURCE_LOCK_WORKTREE_MISMATCH/);
    assert.match(gate,/canonicalSourceLockSha256\(root\)/);
  }finally{rmSync(repo,{recursive:true,force:true})}
});

// This source-only retirement assertion supplements, never replaces, the
// original unchanged Linux guardian-loss, migration, restart and cancel cases.
test('SEC06C reviewed Linux workers have no same-UID guardian or mutable-profile proof fallback',()=>{
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const oracle=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  for(const legacy of [/LinuxCgroupCustodian/,/LinuxOwnedGroupGuardian/,
    /bind_worker_to_cgroup_before_exec/,/spawn_if_delegated/,
    /worker\.cgroup/,/worker\.guardian/]){
    assert.doesNotMatch(host,legacy,'retired guardian still launches production workers');
  }
  assert.match(host,/if self\.profile\.is_some\(\)\|\|typed_capture\.is_some\(\)/);
  assert.match(host,/os_owned_git_spawn\.is_none\(\)/);
  assert.match(host,/os_owned_probe_spawn\.is_none\(\)/);
  assert.match(host,/os_owned_native_spawn\.is_none\(\)/);
  assert.match(host,/BackgroundProcessError::SpawnFailed/);
  assert.match(oracle,/linux_os_owned_git_original_receipt/);
  assert.match(oracle,/linux_os_owned_probe_original_receipt/);
  assert.match(oracle,/linux_os_owned_native_original_receipt/);
  assert.doesNotMatch(oracle,/lu-cgroup-os-\{turn\}-\{call\}\.receipt/);
});

// Regression: old guard binaries are CI-only and the Linux browser cannot
// execute from the former direct-spawn path until one OS-owned session covers
// both Node and Chromium. This is a fail-closed gate, NOT browser acceptance.
test('SEC06C retires production guardian binaries and fences uncovered Linux browser',()=>{
  const manifest=readFileSync(join(root,'src-tauri/lu-win-worker-job/Cargo.toml'),'utf8');
  const lib=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/lib.rs'),'utf8');
  const browser=readFileSync(join(root,'lu-desktop/src-tauri/src/native_browser_host.rs'),'utf8');
  for(const kind of ['lu-linux-cgroup-guard','lu-linux-group-guard']){
    const start=manifest.indexOf('name = "'+kind+'"');
    assert.ok(start>=0,'missing original CI-only bin '+kind);
    assert.match(manifest.slice(start,start+180),/required-features = \["e1-p08-crash-fixture"\]/);
  }
  assert.match(lib,/cfg\(all\(target_os="linux",any\(test,feature="e1-p08-crash-fixture"\)\)\)/);
  assert.match(browser,/SEC06C_BROWSER_JOINT_KERNEL_CUSTODY_UNAVAILABLE/);
  assert.match(browser,/Self::require_browser_kernel_custody\(\)\?/);
  assert.match(browser,/self\.package_verified\(\)[\s\S]{0,100}Self::require_browser_kernel_custody\(\)\.is_ok\(\)/);
  assert.doesNotMatch(browser,/systemd-run|sudo|Command::new/);
});

// Every public Linux direct-spawn browser path must independently reject
// uncontained starts, even if an internal caller bypasses the desktop gate.
test('SEC06C public Linux Chromium and Node launch paths both fail closed until joint unit exists',()=>{
  const chrome=readFileSync(join(root,'src-tauri/lu-core/src/browser_chromium_launcher.rs'),'utf8');
  const node=readFileSync(join(root,'src-tauri/lu-core/src/browser_worker_supervisor.rs'),'utf8');
  assert.match(chrome,/KernelCustodyUnavailable/);
  assert.match(chrome,/require_joint_os_owned_browser_scope\(\)\?/);
  assert.match(chrome,/#\[cfg\(target_os="linux"\)\]/);
  assert.match(node,/KernelCustodyUnavailable/);
  assert.match(node,/require_joint_os_owned_node_scope\(\)\?/);
  assert.match(node,/#\[cfg\(target_os="linux"\)\]/);
});

// SEC-06C browser process admission is a NEW original root-owned service.
// This source test does not replace kernel-native browser and hostile tests.
test('SEC06C browser puts Chrome, Node and setsid descendants under one root-owned leaf',()=>{
  const path=(p)=>readFileSync(join(root,p),'utf8');
  const launcher=path('scripts/lu-sec06c-owned-browser-launcher.py');
  const manager=path('scripts/lu-sec06c-owned-browser-manager.py');
  const mediator=path('scripts/lu-sec06c-owned-browser-worker.py');
  const proof=path('src-tauri/lu-win-worker-job/src/os_owned_browser.rs');
  const host=path('src-tauri/lu-core/src/browser_worker_supervisor.rs');
  const desktop=path('lu-desktop/src-tauri/src/native_browser_host.rs');
  const rust=path('src-tauri/lu-win-worker-job/src/lib.rs');
  const x11=path('src-tauri/lu-core/src/browser_process_owner.rs');
  const grant=path('lu-desktop/src-tauri/src/lib.rs');
  for(const kind of ['chromium','node','script','policy_sha']){
    assert.ok(launcher.includes(kind),'missing root broker SHA contract '+kind);
    assert.ok(manager.includes(kind),'missing original kernel identity '+kind);
  }
  for(const value of ['--property=User=root','--property=Delegate=yes',
      '--property=KillMode=control-group','--property=NoNewPrivileges=yes',
      '--property=AmbientCapabilities=CAP_SETUID','--property=TasksMax=128',
      '--property=MemoryMax=4294967296']){
    assert.ok(launcher.includes(value),'missing fixed browser systemd property '+value);
  }
  assert.match(launcher,/p\.mkdir\(mode=0o755,parents=True,exist_ok=True\)/);
  assert.match(launcher,/for checked in \(p,p\.parent,p\.parent\.parent\)/);
  assert.match(manager,/runpy\.run_path\(str\(KERNEL\)/);
  assert.match(manager,/k\["first_party_unit"\]\(a\.nonce,"browser"\)/);
  assert.match(manager,/k\["preexec_leaf"\]\(leaf,a\.uid,user\.pw_gid/);
  assert.match(manager,/--attack-leaf/);
  assert.match(manager,/SEC06C_BROWSER_ORIGINAL_ANCESTOR_MIGRATION_NOT_DENIED/);
  assert.match(manager,/parse_pair\(frame,a\.nonce,leaf\)/);
  assert.match(manager,/for pid in \(cpid,npid\)/);
  assert.match(manager,/for label,fd in \(\("chromium",cfd\),\("node",nfd\)\)/);
  assert.match(manager,/watcher\.poll\(0\)/);
  assert.match(manager,/original "\+label\+" PIDFD terminated during session/);
  assert.match(manager,/mediator_exit="\+str\(child\.poll\(\)\)/);
  assert.match(manager,/leaf\/"cgroup\.kill"/);
  assert.match(manager,/k\["wait_kernel_zero"\]\(events,15\)/);
  assert.match(manager,/k\["original_root_file"\]\(receipt/);
  assert.match(manager,/EFFECT=.*STOPPED/);
  assert.match(mediator,/chrome=subprocess\.Popen/);
  assert.match(mediator,/node=subprocess\.Popen/);
  assert.match(mediator,/require_node_startup_ack\(node,node_log\)/);
  assert.match(mediator,/LU_SEC06C_NODE_READY_V1/);
  assert.match(mediator,/original Node protocol initialization timed out/);
  const nodeWorker=path('src-tauri/lu-core/workers/lu_browser_worker.mjs');
  assert.match(nodeWorker,/LU_SEC06C_STARTUP_ACK/);
  assert.match(nodeWorker,/LU_SEC06C_NODE_READY_V1/);
  assert.match(mediator,/require_go\(a\.nonce\)/);
  assert.match(mediator,/connectOverCDP|cdpEndpoint/);
  assert.match(proof,/BrowserKernelAdmission/);
  assert.match(proof,/verified_member\(chrome_pid/);
  assert.match(proof,/verified_member\(node_pid/);
  assert.match(proof,/SEC06C browser original kernel receipt\/one-use identity mismatch/);
  assert.match(proof,/EFFECT=STOPPED/);
  assert.match(proof,/LU_SEC06C_BROWSER_JOURNAL_V1/);
  assert.match(proof,/File::open\(profile\)\?\.sync_all\(\)/);
  assert.match(proof,/pub fn reconcile_previous\(profile:&Path\)/);
  assert.match(proof,/record\.original_root_receipt\(\)\?/);
  assert.match(proof,/JOURNAL_EVENT_RECONCILED/);
  assert.match(proof,/original_browser_journal_requires_independent_root_receipt_on_restart/);
  assert.match(host,/LinuxOsOwnedBrowserSession::reconcile_previous\(&config\.trusted_profile\)/);
  assert.match(host,/pub fn launch_sec06c/);
  assert.match(host,/pub fn stop_verified/);
  assert.match(host,/scope\.stop_and_verify\(&mut self\.child\)/);
  assert.match(desktop,/OwnedBrowserWorker::launch_sec06c/);
  assert.match(desktop,/worker\.stop_verified\(\)/);
  const stop=desktop.indexOf('pub fn stop(&self)');
  const stopSection=desktop.slice(stop,stop+800);
  assert.match(stopSection,/session\.as_mut\(\)/);
  assert.match(stopSection,/worker\.stop_verified\(\)/);
  assert.ok(stopSection.indexOf('session.take()')>stopSection.indexOf('worker.stop_verified()'),
    'never lose original root service custody on failed kernel cleanup');
  assert.match(desktop,/SEC06C_BROWSER_PRIOR_KERNEL_CUSTODY_UNRECONCILED/);
  assert.match(desktop,/OwnedBrowserWorker::sec06c_reconcile_previous/);
  assert.match(x11,/original_leaf_member\(pid,&leaf\)/);
  assert.match(x11,/set_verified_browser_pid_visible/);
  assert.match(grant,/&chat_id,&turn_id,&launch_task_id/);
  assert.match(rust,/pub mod os_owned_browser/);
  assert.doesNotMatch(launcher,/shell=True|os\.system\(/);
  assert.doesNotMatch(manager,/shell=True|os\.system\(/);
  assert.doesNotMatch(mediator,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-browser-syntax/);
  // No direct Linux browser launch, no borrowed scanner/native receipt.
  assert.match(host,/require_joint_os_owned_node_scope\(\)\?/);
  assert.match(path('src-tauri/lu-core/src/browser_chromium_launcher.rs'),
    /require_joint_os_owned_browser_scope\(\)\?/);
});

// Native SEC-06C browser acceptance must use ORIGINAL headed Chromium and
// Node, a real X11 human takeover, a scoped root systemd unit and root receipt.
// Source assertions alone cannot certify kernel custody or installation.
test('SEC06C real browser native job is a mandatory independent SEC06F gate',()=>{
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8');
  const fixture=readFileSync(join(root,'benchmarks/e1/sec06c_browser_real_native.py'),'utf8');
  assert.match(workflow,/core01e-sec06c-os-owned-browser:/);
  assert.match(workflow,/SEC-06C real jointly owned headed Chromium and Node/);
  assert.match(workflow,/playwright@1\.64\.1/);
  assert.match(workflow,/assemble-browser-ci-candidate\.mjs/);
  assert.match(workflow,/xvfb-run -a/);
  assert.match(workflow,/sec06c_browser_real_native\.py/);
  assert.match(workflow,/sec06c_browser_host_crash\.py/);
  assert.match(workflow,/sec06c_browser_guardian_loss\.py/);
  assert.match(workflow,/original-browser-guardian-loss\.log/);
  assert.match(workflow,/SEC06C_BROWSER_ORIGINAL_GUARDIAN_LOSS_FAIL_CLOSED_PASS=1/);
  const rootLoss=readFileSync(join(root,'benchmarks/e1/sec06c_browser_guardian_loss.py'),'utf8');
  assert.match(rootLoss,/find_exact_root_manager\(unit,nonce\)/);
  assert.match(rootLoss,/systemctl","kill"/);
  assert.match(rootLoss,/--kill-whom=main/);
  assert.match(rootLoss,/fds=\[os\.pidfd_open\(p\) for p in \(manager,chrome,node\)\]/);
  assert.match(rootLoss,/if receipt\.exists\(\):/);
  assert.match(rootLoss,/DEAD ROOT SUPERVISOR MINTED FALSE KERNEL_EMPTY=1 RECEIPT/);
  assert.match(rootLoss,/stale_authority_replay_denied=1/);
  assert.doesNotMatch(rootLoss,/shell=True|os\.system\(/);
  assert.match(workflow,/sec06c_real_linux_browser_original_grant_handoff_and_durable_recovery/);
  assert.match(workflow,/original-browser-host-sigkill\.log/);
  assert.match(workflow,/browser-original-log-sha256\.txt/);
  assert.match(workflow,/SEC06F_ORIGINAL_BROWSER_LOG_MISSING/);
  assert.match(workflow,/SEC06C_ORIGINAL_RUST_BROWSER_KERNEL_PASS=1/);
  assert.match(workflow,/SEC06C_BROWSER_ORIGINAL_HOST_SIGKILL_PASS=1/);
  assert.match(workflow,/SEC06C_GENUINE_BROWSER_KERNEL_PASS=1/);
  assert.match(workflow,/SEC06_BROWSER_JOB:/);
  assert.match(workflow,/for name in \('SEC06_GIT_JOB'[\s\S]{0,180}'SEC06_BROWSER_JOB'/);
  assert.match(fixture,/os\.pidfd_open\(pid\)/);
  assert.match(fixture,/membership\(chrome_pid\)/);
  assert.match(fixture,/membership\(node_pid\)/);
  assert.match(fixture,/xdotool/);
  assert.match(fixture,/browser_call\(proc,chrome_pid,"show_browser"\)/);
  assert.match(fixture,/browser_call\(proc,chrome_pid,"hide_browser"\)/);
  assert.match(fixture,/proc\.stdin\.close\(\)/);
  assert.match(fixture,/root_receipt\(before,nonce/);
  assert.match(fixture,/nonce_replay_denied=1/);
  assert.match(fixture,/unrelated_survived=1/);
  assert.doesNotMatch(fixture,/MOCK_|FAKE_|SIMULATED_KERNEL_PASS/);
});

test('SEC06C Chrome startup failure logs bounded redacted stderr without bypassing OS custody',()=>{
  const mediator=readFileSync(join(root,'scripts/lu-sec06c-owned-browser-worker.py'),'utf8');
  assert.match(mediator,/def chrome_pre_admission_failure\(/);
  assert.match(mediator,/log\.read\(4096\)/);
  assert.match(mediator,/text\.replace\(private,"<original-user-private-path>"\)/);
  assert.match(mediator,/SEC06C_BROWSER_CHROMIUM_PRE_ADMISSION_FAILED/);
  assert.match(mediator,/stderr=startup_log/);
  assert.match(mediator,/reject\("original Chromium died before admission"\)/);
  assert.doesNotMatch(mediator,/--no-sandbox|--disable-setuid-sandbox/);
});

test('SEC06C Chromium userns is granted to a single root-owned installed path, not disabled globally',()=>{
  const workflow=readFileSync(join(root,'.github/workflows/lu-repo01-full-source.yml'),'utf8');
  const profile=readFileSync(join(root,'scripts/lu-sec06c-browser-userns.apparmor'),'utf8');
  const mediator=readFileSync(join(root,'scripts/lu-sec06c-owned-browser-worker.py'),'utf8');
  assert.match(profile,/profile lu-sec06c-chromium \/usr\/lib\/lu\/browser\/chrome flags=\(unconfined\)/);
  assert.match(profile,/\buserns,/);
  assert.doesNotMatch(profile,/\*|\/home\/|\/tmp\//);
  assert.match(workflow,/apparmor_parser --replace \/etc\/apparmor.d\/lu-sec06c-browser-ci/);
  assert.match(workflow,/SEC06C_BROWSER_EXACT_CHROMIUM_USERNS_APPARMOR=1/);
  assert.match(workflow,/apparmor_restrict_unprivileged_userns\)" = 1/);
  assert.doesNotMatch(workflow,/echo 0 \| sudo tee \/proc\/sys\/kernel\/apparmor_restrict_unprivileged_userns/);
  assert.doesNotMatch(mediator,/--no-sandbox|--disable-setuid-sandbox/);
});

test('SEC06C genuine headed Chrome checks a live renderer user namespace AND seccomp',()=>{
  const native=readFileSync(join(root,'benchmarks/e1/sec06c_browser_real_native.py'),'utf8');
  assert.match(native,/def genuine_renderer_userns_sandbox\(chrome_pid,group,leaf\)/);
  assert.match(native,/\(leaf\/"cgroup\.procs"\)\.read_text\(\)/);
  assert.match(native,/b"--type=renderer"/);
  assert.match(native,/status_lines\.get\("Seccomp",""\)/);
  assert.match(native,/chrome_userns==new_userns/);
  assert.match(native,/membership\(pid\)!=group/);
  assert.match(native,/SEC06C_CHROMIUM_REAL_USERNS_SECCOMP_PASS=1/);
  assert.match(native,/genuine_renderer_userns_sandbox\(chrome_pid,group,leaf\)/);
  assert.match(native,/if b"--no-sandbox" in cmd or b"--disable-setuid-sandbox" in cmd/);
});

test('SEC06C OS-owned original PIDFD stall scenarios require genuine systemd subtree cleanup',()=>{
  const gate=readFileSync(join(root,'scripts/p0-sec06-consolidated.mjs'),'utf8');
  const native=readFileSync(join(root,'benchmarks/e1/sec06c_root_owned_systemd_boundary.py'),'utf8');
  for(const mode of ['guardian-stall','worker-stall']){
    assert.match(gate,new RegExp('linux-root-owned-unit-'+mode));
    assert.match(native,new RegExp('"'+mode+'"'));
  }
  assert.match(native,/signal\.pidfd_send_signal\(victim_fd,signal\.SIGSTOP\)/);
  assert.match(native,/if state in \("T","t"\):break/);
  assert.match(native,/"nativeMainSigkill": scenario in \("worker-main-sigkill","guardian-death"\)/);
  assert.match(native,/if scenario in \("cancel","ancestor-migration"\)/);
  assert.match(native,/SEC06C_ANCESTOR_ESCAPE_SUCCEEDED/);
  assert.match(sec06,/linux-retired-guardian-admission-denial/);
  const retired=path('benchmarks/e1/sec06c_retired_guardian_denial.py');
  assert.match(retired,/SEC06C_BLOCKED_UNSAFE_WRITABLE_ANCESTOR/);
  assert.match(retired,/historical_cases_preserved=4/);
  assert.match(native,/"nativeSupervisorActuallyStalled": scenario=="guardian-stall"/);
  assert.match(native,/"nativeWorkerActuallyStalled": scenario=="worker-stall"/);
  assert.match(native,/"setsidDescendantReaped": True/);
  assert.match(native,/SEC06C_ANCESTOR_ESCAPE_SUCCEEDED/);
});
