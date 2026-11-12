import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkedToolchainInventory,checkedPackageToolchainReadiness,
  checkedProtectedTerminal} from '../web/toolchain_terminal.js';

const project='ide-'+'a'.repeat(32),foreign='ide-'+'b'.repeat(32);
const job='job-'+'c'.repeat(32),sha='d'.repeat(64);
const status=()=>({
  ready:true,cargoReady:true,code:'BUILD_READY',
  multilangReady:['python_pytest','js_tests','cpp_cmake_build',
    'uv_sync_offline'].map(profileId=>({
      profileId,ready:true,code:'DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB',
    })),
  journal:[{projectId:project,jobId:job,planSha256:sha,
    phase:'completed',requiresReconciliation:false}],
  quarantined:[],recent:[{projectId:project,jobId:job,
    planSha256:sha,recipeId:'rust_build',phase:'completed',
    terminalReceiptSha256:sha,requiresReconciliation:false,
    installedAppAccepted:false}],
  current:null,lastOutputJobId:job,
});
const output=()=>({projectId:project,jobId:job,lastSequence:2,
  truncated:false,liveUnverified:true,chunks:[
    {sequence:1,stream:'stdout',text:'built\n'},
    {sequence:2,stream:'stderr',text:'warning\n'},
  ]});

test('INT05 native fixed catalog does not infer installation or execution',()=>{
  const rows=checkedToolchainInventory(status(),project);
  assert.equal(rows.kind,'native_pin_observation');
  assert.equal(rows.rows.length,5);
  assert.equal(rows.executionAuthorized,false);
  assert.ok(rows.rows.every(row=>row.ready && !row.canInstall &&
    !row.canStartWithoutConsent && !row.versionVerified));
});
test('INT05 incomplete, duplicated, malformed readiness fails closed',()=>{
  const a=status();a.multilangReady.pop();
  assert.equal(checkedToolchainInventory(a,project).kind,'unavailable');
  const b=status();b.multilangReady[3]={...b.multilangReady[0]};
  assert.equal(checkedToolchainInventory(b,project).kind,'unavailable');
  const c=status();c.multilangReady[0].ready='yes';
  assert.equal(checkedToolchainInventory(c,project).kind,'unavailable');
});
test('INT05 missing runtime or global recovery never becomes runnable',()=>{
  const a=status();a.multilangReady[3].ready=false;
  const inv=checkedToolchainInventory(a,project);
  assert.equal(inv.rows.find(x=>x.profileId==='uv_sync_offline').ready,false);
  const b=status();b.quarantined=[{projectId:foreign,requiresReconciliation:true}];
  assert.equal(checkedToolchainInventory(b,project).blockedByRecovery,true);
});
test('INT05 package manager review is not uv execution entitlement',()=>{
  const inspection={projectId:project,fingerprint:sha,
    executionAuthorized:false,applySupported:false,networkAccessed:false,
    candidates:['uv','vcpkg','conan'].map(managerId=>({
      managerId,state:'candidate_needs_review',
      executionAuthorized:false,applySupported:false,resolutionSupported:false,
    }))};
  const rows=checkedPackageToolchainReadiness(inspection,
    checkedToolchainInventory(status(),project),project);
  assert.equal(rows.length,3);
  assert.equal(rows[0].nativeOfflineSyncAvailable,true);
  assert.equal(rows[1].nativeOfflineSyncAvailable,false);
  assert.ok(rows.every(x=>x.installAuthorized===false));
  inspection.candidates[0].executionAuthorized=true;
  assert.equal(checkedPackageToolchainReadiness(inspection,
    checkedToolchainInventory(status(),project),project).length,0);
});
test('INT05 transcript is project/job/journal bound, not replayable',()=>{
  const view=checkedProtectedTerminal(status(),output(),project);
  assert.equal(view.kind,'ephemeral_stream_with_native_receipt');
  assert.equal(view.terminalReceiptSha256,sha);
  assert.equal(view.outputIndependentlyVerified,false);
  assert.equal(view.interactive,false);
  assert.equal(view.chunks.length,2);
  assert.equal(checkedProtectedTerminal(status(),output(),foreign).kind,
    'unavailable');
  const s=status();s.lastOutputJobId=null;
  assert.equal(checkedProtectedTerminal(s,output(),project).kind,'unavailable');
});
test('INT05 untrusted live worker output cannot claim terminal success',()=>{
  const s=status();s.recent=[];s.current={
    projectId:project,jobId:job,recipeId:'rust_build'};
  const view=checkedProtectedTerminal(s,output(),project);
  assert.equal(view.kind,'unverified_active_stream');
  assert.equal(view.terminalReceiptSha256,null);
  const q=status();q.quarantined=[{projectId:project,jobId:job,
    requiresReconciliation:true}];
  assert.equal(checkedProtectedTerminal(q,output(),project).kind,'unavailable');
});
test('INT05 rejects malicious stream, sequence and oversized text',()=>{
  const a=output();a.chunks[0].stream='shell';
  assert.equal(checkedProtectedTerminal(status(),a,project).kind,'unavailable');
  const b=output();b.chunks[1].sequence=1;
  assert.equal(checkedProtectedTerminal(status(),b,project).kind,'unavailable');
  const c=output();c.chunks[0].text='x'.repeat(8193);
  assert.equal(checkedProtectedTerminal(status(),c,project).kind,'unavailable');
});
test('INT05 controls and terminal escape sequences never become markup',()=>{
  const o=output();o.chunks[0].text='Hello\x1b[31m\u202e';
  const v=checkedProtectedTerminal(status(),o,project);
  assert.match(v.chunks[0].text,/�/);
  assert.equal(v.chunks[0].text.includes('\x1b'),false);
});
test('INT05 desktop uses the existing native admission, not a shell plugin',()=>{
  const main=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(main,/checkedToolchainInventory/);
  assert.match(main,/checkedProtectedTerminal/);
  assert.match(main,/Protected terminal transcript/);
  assert.match(main,/No arbitrary commands, shells or package installs/);
  assert.doesNotMatch(main,/shell:allow-execute|Command\.create\(/);
});
