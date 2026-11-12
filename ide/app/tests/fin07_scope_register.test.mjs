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
