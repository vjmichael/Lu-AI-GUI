import test from 'node:test';
import assert from 'node:assert/strict';
import {fin07ScopeRegister} from '../web/fin07_scope_register.js';
const id='ide-'+'a'.repeat(32);
test('FIN07 feature-scope register cannot be a release approval',()=>{
 const s=fin07ScopeRegister(id);
 assert.equal(s.rows.length,7);
 assert.equal(s.frozen,false);
 assert.equal(s.releaseAuthorized,false);
 assert.equal(s.sourceCompilerAccepted,false);
 assert.equal(s.nativeWindowsAccepted,false);
 assert.equal(s.physicalHumanAccepted,false);
 assert.ok(s.unresolvedImplementation>=4);
});
test('FIN07 scope only belongs to a real project identifier',()=>{
 assert.equal(fin07ScopeRegister('../../fake'),null);
 assert.equal(fin07ScopeRegister('ide-'+'z'.repeat(32)),null);
});

test('FIN07 independent evidence never grants feature freeze',async()=>{
 const {fin07ReconcileEvidence}=await import('../web/fin07_scope_register.js');
 const result=fin07ReconcileEvidence(id,[{
  gate:'nativeWindows',projectId:id,status:'independently_verified',
  sourceHead:'a'.repeat(40),receiptSha256:'b'.repeat(64),
  independentReview:true,executionAuthorized:false,releaseAuthorized:false
 }]);
 assert.equal(result.allEvidenceVerified,false);
 assert.equal(result.frozen,false);
 assert.equal(result.releaseAuthorized,false);
 assert.equal(result.gates.length,8);
 assert.equal(fin07ReconcileEvidence(id,[{
  gate:'nativeWindows',projectId:id,status:'independently_verified',
  sourceHead:'a'.repeat(40),receiptSha256:'bad',
  independentReview:true,executionAuthorized:false,releaseAuthorized:false
 }]),null);
});

test('FIN07B reconciliation never derives implementation closure from observations',async()=>{
 const {fin07ImplementationReconciliation}=await import('../web/fin07_scope_register.js');
 const workflow={projectId:id,kind:'advisory_workflow_readiness',
  rows:Array.from({length:6},()=>({executionAuthorized:false})),
  blockedByRecovery:false,gitObserved:true,debuggerLaunchObserved:true,
  packageReviewObserved:true,executionAuthorized:false,
  installedAccepted:false,taskAccepted:false};
 const result=fin07ImplementationReconciliation(id,workflow);
 assert.equal(result.groups.length,7);
 assert.equal(result.unresolvedGroups,7);
 assert.equal(result.featureFrozen,false);
 assert.equal(result.productionReleaseEligible,false);
 assert.ok(result.groups.every(row=>row.implementationClosed===false));
 assert.equal(result.groups.find(row=>row.id==='FIN-02').boundary,
  'deferred_native_runtime_admission');
 assert.equal(fin07ImplementationReconciliation('ide-'+'c'.repeat(32),workflow),null);
 assert.equal(fin07ImplementationReconciliation(id,{
  ...workflow,installedAccepted:true}),null);
});
