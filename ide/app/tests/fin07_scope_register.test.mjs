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
