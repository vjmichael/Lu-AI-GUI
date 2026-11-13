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

test('SEC06C trusted OS-owned Git launch binds original journal nonce and finite root-managed worker',()=>{
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_git.rs'),'utf8');
  const tasks=readFileSync(join(root,'src-tauri/lu-core/src/tasks.rs'),'utf8');
  const historical=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-git-launcher.py'),'utf8');
  const worker=readFileSync(join(root,'scripts/lu-sec06c-owned-git-worker.py'),'utf8');
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
  for(const text of ['--property=Delegate=no','--property=KillMode=control-group',
    '--property=ProtectControlGroups=strict','--property=NoNewPrivileges=yes',
    '--property=User=','root_owned_file(WORKER)','SUDO_UID',
    'SEC06C_OS_BROKER_DENIED','cgroup.events']){
    assert.ok(launcher.includes(text),'missing OS-owned manager contract '+text);
  }
  assert.match(worker,/core\.fsmonitor=false/);
  assert.match(worker,/start_new_session|Popen/);
  assert.match(worker,/authorized_go\(args\.nonce\)/);
  assert.match(worker,/original Lu host died before one-use admission/);
  assert.match(worker,/original Lu host cancelled before reviewed Git spawn/);
  assert.match(launcher,/original host lifetime ended before systemd GO/);
  assert.match(worker,/SEC06C_GIT_WORKER_BLOCKED/);
  assert.doesNotMatch(launcher,/shell=True|subprocess\.getoutput|os\.system\(/);
  assert.doesNotMatch(worker,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-git-syntax/);
  assert.match(gate,/linux-ancestor-migration/);
});

test('SEC06C kernel populated-zero is witnessed before root-owned service teardown, never inferred after removal',()=>{
  for(const path of [
    'scripts/lu-sec06c-owned-git-launcher.py',
    'scripts/lu-sec06c-owned-probe-launcher.py'
  ]){
    const source=readFileSync(join(root,path),'utf8');
    assert.match(source,/--remain-after-exit/);
    assert.match(source,/--wait/);
    assert.match(source,/--property=SuccessExitStatus=93/);
    assert.match(source,/--property=KillMode=control-group/);
    assert.match(source,/--property=ProtectControlGroups=yes/);
    assert.match(source,/def original_service_main\(/);
    assert.match(source,/ExecMainCode/);
    assert.match(source,/ExecMainStatus/);
    assert.match(source,/SubState/);
    assert.match(source,/def wait_populated_zero\(/);
    assert.match(source,/def kill_exact_root_owned_subtree\(/);
    assert.match(source,/cgroup\.kill/);
    assert.match(source,/if not wait_populated_zero\(events,15\)/);
    assert.match(source,/kernel_verified=True/);
    assert.match(source,/if not kernel_verified:/);
    const verified=source.indexOf('kernel_verified=True');
    const stop=source.indexOf('stopped=subprocess.run(');
    assert.ok(verified>=0&&stop>verified,
      'original kernel proof must precede systemd stop/unload in '+path);
    assert.doesNotMatch(source,/if events is None or not kernel_empty\(events\):/);
    assert.match(source,/EFFECT=/);
  }
  assert.match(gate,/linux-ancestor-migration/);
  assert.match(gate,/linux-guardian-death/);
});

test('SEC06C has separate finite root-owned FFprobe/PDFInfo/ExifTool service boundary',()=>{
  const native=readFileSync(join(root,'src-tauri/lu-win-worker-job/src/os_owned_probe.rs'),'utf8');
  const launcher=readFileSync(join(root,'scripts/lu-sec06c-owned-probe-launcher.py'),'utf8');
  const worker=readFileSync(join(root,'scripts/lu-sec06c-owned-probe-worker.py'),'utf8');
  const host=readFileSync(join(root,'src-tauri/lu-core/src/native_background_process.rs'),'utf8');
  const tasks=readFileSync(join(root,'src-tauri/lu-core/src/tasks.rs'),'utf8');
  const custody=readFileSync(join(root,'src-tauri/lu-core/src/agent_os_custody.rs'),'utf8');
  assert.match(native,/SEC06C forbids arbitrary media\/metadata argv/);
  assert.match(native,/ffprobe-wav/);
  assert.match(native,/ffprobe-mp4/);
  assert.match(native,/pdfinfo-pdf/);
  assert.match(native,/exiftool-image/);
  assert.match(native,/\/usr\/libexec\/lu\/lu-sec06c-launch-probe/);
  assert.match(host,/LinuxOsOwnedReviewedProbe::spawn_if_installed/);
  assert.match(host,/bind_linux_os_owned_probe_admission/);
  assert.match(host,/os_owned_probe/);
  assert.match(tasks,/Immutable Linux OS-owned probe admission/);
  assert.match(custody,/linux_os_owned_probe_original_receipt/);
  for(const attr of ['Delegate=no','KillMode=control-group',
    'ProtectControlGroups=yes','PrivateNetwork=yes','NoNewPrivileges=yes',
    'CapabilityBoundingSet=','TasksMax=32','MemoryMax=536870912',
    'secure_file','secure_directory','kernel_empty','cgroup.events']){
    assert.ok(launcher.includes(attr),'missing original kernel/probe contract '+attr);
  }
  assert.match(worker,/os\.O_NOFOLLOW/);
  assert.match(worker,/pass_fds=\(fd,\)/);
  assert.match(worker,/hashlib\.sha256/);
  assert.match(worker,/select\.select/);
  assert.match(worker,/ffprobe-wav/);
  assert.match(worker,/ffprobe-mp4/);
  assert.match(worker,/pdfinfo-pdf/);
  assert.match(worker,/exiftool-image/);
  assert.match(worker,/approved_first_launch/);
  assert.doesNotMatch(launcher,/shell=True|os\.system\(/);
  assert.doesNotMatch(worker,/shell=True|os\.system\(/);
  assert.match(gate,/linux-os-owned-probe-syntax/);
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
  const section=workflow.split('  core01e-sec06f-original-receipts:\n')[1]
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
      'linux-os-owned-git-syntax','linux-os-owned-probe-syntax',
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
