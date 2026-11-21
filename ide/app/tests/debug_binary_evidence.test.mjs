import test from 'node:test';
import assert from 'node:assert/strict';
import {safeDebugBinaryEvidence} from '../web/debug_binary_evidence.js';
const projectId='ide-'+'a'.repeat(32);
const sha=(c)=>c.repeat(64);
const fixed=()=>{
 const b={relativePath:'target/debug/fixture',bytes:100,
   binarySha256:sha('b'),outputTreeSha256:sha('c'),
   format:'elf_candidate_magic_only',
   role:'post_termination_unique_cargo_binary_readback_not_launch_authority',
   debugSymbolsAttested:false,debuggerExecutionAuthorized:false,
   installedAppAccepted:false};
 return {projectId,jobId:'job-'+'a'.repeat(32),recipeId:'rust_build',
   phase:'completed',nativeExitCode:0,requiresReconciliation:false,
   installedAppAccepted:false,planSha256:sha('d'),sourceHead:'e'.repeat(40),
   inputSnapshotSha256:sha('f'),toolchainSha256:sha('1'),
   manifestSha256:sha('2'),terminalReceiptSha256:sha('3'),
   artifactManifestSha256:sha('c'),
   artifactEvidenceState:'read_only_post_termination_tree_hashed',
   debugBinaryCandidate:b,artifactFiles:[{
     relativePath:b.relativePath,bytes:b.bytes,sha256:b.binarySha256}]};
};
test('bind exact terminal receipt to one independently hashed build output',()=>{
 const r=safeDebugBinaryEvidence(fixed(),projectId);
 assert.equal(r.binarySha,sha('b'));assert.equal(r.debuggerAuthorized,false);
 assert.equal(r.terminalReceiptSha,sha('3'));
});
test('never promote cancellation, failure, exit status or unsigned installed claim',()=>{
 for(const [key,value] of [['phase','cancelled'],['nativeExitCode',1],
 ['requiresReconciliation',true],['installedAppAccepted',true],
 ['recipeId','rust_tests'],['terminalReceiptSha256','bad']]){
  let r=fixed();r[key]=value;
  assert.equal(safeDebugBinaryEvidence(r,projectId),null,key);
 }
});
test('reject mismatch, arbitrary path, binary grant and cross-project',()=>{
 let r=fixed();r.debugBinaryCandidate.relativePath='../../private.exe';
 assert.equal(safeDebugBinaryEvidence(r,projectId),null);
 r=fixed();r.debugBinaryCandidate.debuggerExecutionAuthorized=true;
 assert.equal(safeDebugBinaryEvidence(r,projectId),null);
 r=fixed();r.debugBinaryCandidate.binarySha256=sha('9');
 assert.equal(safeDebugBinaryEvidence(r,projectId),null);
 r=fixed();r.artifactManifestSha256=sha('9');
 assert.equal(safeDebugBinaryEvidence(r,projectId),null);
 r=fixed();assert.equal(safeDebugBinaryEvidence(r,'ide-'+'b'.repeat(32)),null);
});
