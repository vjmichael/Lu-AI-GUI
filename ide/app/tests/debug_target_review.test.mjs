import test from 'node:test';
import assert from 'node:assert/strict';
import {safeDebugTargetReview} from '../web/debug_target_review.js';
const project='ide-'+'a'.repeat(32),sha='b'.repeat(64);
const fixture=()=>({
 schemaVersion:1,projectId:project,profileId:'rust_cargo_debug_candidate',
 manifestRelativePath:'Cargo.toml',manifestSha256:sha,cleanGitHead:'c'.repeat(40),
 reviewFingerprint:'d'.repeat(64),sourceSnapshotState:'not_sealed',
 buildReceiptState:'no_completed_native_build_bound',
 binaryIdentityState:'no_binary_sha_or_provenance',debugSymbolsState:'not_attested',
 debuggerAdapterState:'fixed_self_test_only_no_project_target',
 targetExecutable:null,sourceTextExported:false,binaryExecuted:false,
 workerGrantIssued:false,debuggerAuthorized:false,installedAppAccepted:false
});
test('fixed native review carries source evidence only',()=>{
 const value=safeDebugTargetReview(fixture(),project,sha);
 assert.equal(value.executable,null); assert.equal(value.authorized,false);
});
test('rejects fabricated binary, success, grant, wrong project or stale manifest',()=>{
 for(const [key,val] of [
  ['targetExecutable','target/debug/project.exe'],['binaryExecuted',true],
  ['debuggerAuthorized',true],['workerGrantIssued',true],
  ['installedAppAccepted',true],['sourceSnapshotState','sealed'],
  ['binaryIdentityState','attested']]) {
   const r=fixture();r[key]=val;
   assert.throws(()=>safeDebugTargetReview(r,project,sha),key);
 }
 assert.throws(()=>safeDebugTargetReview(fixture(),'ide-'+'f'.repeat(32),sha));
 assert.throws(()=>safeDebugTargetReview(fixture(),project,'e'.repeat(64)));
});
