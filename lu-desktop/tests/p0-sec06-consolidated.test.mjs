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
    const lockHash=digest(readFileSync(join(root,'source-tree.lock.json')));
    const required=[
      'exact-head-inventory','security-contracts','sec06-js-syntax',
      'linux-negative-python-syntax','linux-host-crash-python-syntax',
      'core-native-full','desktop-ansf-native','worker-job-native',
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
    const receipt=join(temp,'linux.receipt.json');
    writeFileSync(receipt,JSON.stringify({
      schema:'lu-sec06-native-evidence-v1',target:'linux',head,
      sourceLockSha256:lockHash,machine:{platform:'linux'},
      status:'PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING',
      allNativeCasesPassed:true,localExecutionOnly:true,
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
