import test from 'node:test';
import assert from 'node:assert/strict';
import {safeConventionalTestDiscovery} from '../web/conventional_tests.js';
const id='ide-'+'a'.repeat(32);
const base=()=>({
  schemaVersion:1,projectId:id,source:'fixed_native_conventional_file_metadata',
  directoriesChecked:7,files:[],truncated:false,incomplete:true,scanNotes:[],
  processStarted:false,workerGrantIssued:false,sourceContentsExported:false,
  discoveryIsExecutionAuthority:false
});
test('read only fixed candidate files',()=>{
 const d=base();d.files=[
  {relativePath:'tests/engine_test.rs',language:'rust',recipeId:'rust_tests',fileBytes:6,runnable:false},
  {relativePath:'tests/test_calc.py',language:'python',recipeId:null,fileBytes:5,runnable:false}
 ];
 const r=safeConventionalTestDiscovery(d,id);
 assert.equal(r.files.length,2);
 assert.ok(!r.files[1].runnable);
});
test('rejects project swap and forbidden execution fields',()=>{
 const d=base();assert.throws(()=>safeConventionalTestDiscovery(d,'ide-'+'b'.repeat(32)));
 d.workerGrantIssued=true;assert.throws(()=>safeConventionalTestDiscovery(d,id));
});
test('rejects traversal, duplicates, and purported runnable files',()=>{
 for(const path of ['tests/../secret.py','tests/a/b.test.js','/etc/passwd','tests/evil\\path.py']){
   const d=base();d.files=[{relativePath:path,language:'python',recipeId:null,
     fileBytes:1,runnable:false}];
   assert.throws(()=>safeConventionalTestDiscovery(d,id));
 }
 const d=base();d.files=[{relativePath:'tests/test_x.py',language:'python',
   recipeId:null,fileBytes:1,runnable:true}];
 assert.throws(()=>safeConventionalTestDiscovery(d,id));
});
test('rejects any claim to exhaustive test discovery',()=>{
 const d=base();d.incomplete=false;assert.throws(()=>safeConventionalTestDiscovery(d,id));
});
