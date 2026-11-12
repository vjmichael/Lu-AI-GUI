import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkedToolchainInventory,checkedPackageToolchainReadiness,
  checkedProtectedTerminal} from '../web/toolchain_terminal.js';

const project='ide-'+'a'.repeat(32),foreign='ide-'+'b'.repeat(32);
const job='job-'+'c'.repeat(32),sha='d'.repeat(64);
const status=()=>({
  ready:true,cargoReady:true,code:'BUILD_READY',
  globalRecoveryRequired:false,activeInAnotherProject:false,
  globalWorkerBusy:false,
  cargoProvenance:{
    profileId:'rust_cargo',observedSha256:sha,releasePinSha256:sha,
    provenanceState:'installed_bytes_match_compile_time_release_pin',
    versionEvidence:'not_independently_verified',
    publisherEvidence:'not_independently_verified',
    licenseEvidence:'not_independently_verified',
    executionAuthorized:false,installedAppAccepted:false,
  },
  multilangReady:['python_pytest','js_tests','cpp_cmake_build',
    'uv_sync_offline'].map(profileId=>({
      profileId,ready:true,code:'DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB',
      provenance:{
        profileId,observedSha256:sha,releasePinSha256:sha,
        provenanceState:'installed_bytes_match_compile_time_release_pin',
        versionEvidence:'not_independently_verified',
        publisherEvidence:'not_independently_verified',
        licenseEvidence:'not_independently_verified',
        executionAuthorized:false,installedAppAccepted:false,
      },
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
  assert.ok(rows.rows.every(row=>row.ready && row.verified &&
    row.observedSha256===sha && !row.canInstall &&
    !row.canStartWithoutConsent && !row.versionVerified &&
    !row.publisherVerified && !row.licenseVerified));
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
  a.multilangReady[3].provenance={
    ...a.multilangReady[3].provenance,observedSha256:null,
    provenanceState:'unavailable_or_not_independently_verified'};
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

test('INT05-E release pin and installed byte mismatch never claims availability',()=>{
  const s=status();
  s.multilangReady[0].provenance.observedSha256='e'.repeat(64);
  assert.equal(checkedToolchainInventory(s,project).kind,'unavailable');
  const s2=status();
  s2.cargoProvenance.licenseEvidence='verified';
  assert.equal(checkedToolchainInventory(s2,project).kind,'unavailable');
  const s3=status();
  s3.multilangReady[1].provenance.releasePinSha256=null;
  assert.equal(checkedToolchainInventory(s3,project).kind,'unavailable');
});
test('INT05-E pure host provenance binds observed bytes and release pins',()=>{
  const native=readFileSync(new URL('../../src-tauri/src/build_backend.rs',import.meta.url),'utf8');
  assert.match(native,/installed_bytes_match_compile_time_release_pin/);
  assert.match(native,/runtime_provenance\("rust_cargo"/);
  assert.match(native,/provenance: RuntimeProvenance/);
  assert.match(native,/license_evidence: "not_independently_verified"/);
});

test('FIN01 global native custody blocks even when foreign jobs are hidden',()=>{
  const s=status();s.globalRecoveryRequired=true;
  const i=checkedToolchainInventory(s,project);
  assert.equal(i.blockedByRecovery,true);
  assert.equal(i.rows[0].disposition,'reconciliation_required');
  const busy=status();busy.activeInAnotherProject=true;
  busy.globalWorkerBusy=true;
  const j=checkedToolchainInventory(busy,project);
  assert.equal(j.workerBusy,true);
  assert.equal(j.rows[0].disposition,'native_worker_busy');
  const legacy=status();delete legacy.globalRecoveryRequired;
  assert.equal(checkedToolchainInventory(legacy,project).kind,'unavailable');
});

test('FIN05 global native worker busy blocks offline uv even when manifest exists',()=>{
  const native=status();native.globalWorkerBusy=true;
  const inspection={projectId:project,fingerprint:sha,
    executionAuthorized:false,applySupported:false,networkAccessed:false,
    candidates:[{managerId:'uv',state:'candidate_needs_review',
      executionAuthorized:false,applySupported:false,resolutionSupported:false}]};
  const found=checkedPackageToolchainReadiness(inspection,
    checkedToolchainInventory(native,project),project);
  assert.equal(found[0].nativeOfflineSyncAvailable,false);
});
test('FIN05 private artifacts cannot imply project promotion',async()=>{
  const {checkedPrivatePackageLifecycle}=await import('../web/toolchain_terminal.js');
  const s=status();s.packageAudits=[{
    projectId:project,jobId:job,planSha256:sha,
    terminalReceiptSha256:sha,outputTreeSha256:sha,auditSha256:sha,
    state:'verified_local_bytes_private_not_promoted',
    requiresExplicitPrivateCleanup:true,mayExecuteOrRetryWorker:false,
    installedIntoProject:false,
  }];
  const viewed=checkedPrivatePackageLifecycle(s,project);
  assert.equal(viewed.rows.length,1);
  assert.equal(viewed.projectPromotionVerified,false);
  assert.equal(checkedPrivatePackageLifecycle({
    ...s,packageAudits:[{...s.packageAudits[0],installedIntoProject:true}]
  },project),null);
  assert.equal(checkedPrivatePackageLifecycle({
    ...s,packageAudits:[{...s.packageAudits[0],
      projectId:'ide-'+'f'.repeat(32)}]
  },project),null);
});
