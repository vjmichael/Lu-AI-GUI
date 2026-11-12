import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkedTestExplorer} from '../web/test_explorer.js';
const id='ide-'+'a'.repeat(32);
const discovery={projectId:id,incomplete:true,files:[{
  path:'tests/test_math.py',language:'python',recipe:null,runnable:false},
  {path:'tests/helpers.rs',language:'rust',recipe:'rust_tests',runnable:false}]};
test('FIN04 test filename metadata does not create runnable cases',()=>{
  const v=checkedTestExplorer(discovery,null,id);
  assert.equal(v.files.length,2);
  assert.equal(v.singleCaseExecutionSupported,false);
  assert.equal(v.executionAuthorized,false);
  assert.equal(v.sourceMetadataOnly,true);
  assert.equal(checkedTestExplorer(discovery,null,id,'python').files.length,1);
});
test('FIN04 original reporter output never certifies assertions',()=>{
  const report={projectId:id,jobId:'job-'+'b'.repeat(32),
    taskAcceptanceAuthorized:false,independentlyVerified:false,
    cases:[{id:'case_'+'c'.repeat(64),name:'test_math',
      status:'reported_fail',source:{relativePath:'tests/test_math.py'},
      independentlyVerified:false}]};
  const v=checkedTestExplorer(discovery,report,id);
  assert.equal(v.cases[0].canReviewSource,true);
  assert.equal(v.cases[0].assertionVerified,false);
  assert.equal(v.installedAccepted,false);
});
test('FIN04 rejects foreign project and hostile paths',()=>{
  assert.equal(checkedTestExplorer(discovery,null,'ide-'+'d'.repeat(32)).files.length,0);
  assert.equal(checkedTestExplorer({...discovery,files:[{
    path:'../secret.py',language:'python',recipe:null,runnable:false}]},
    null,id),null);
});
test('FIN04 GUI never invokes ad-hoc test or process run',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(ui,/FIN-04 · Test Explorer/);
  assert.match(ui,/checkedTestExplorer/);
  assert.doesNotMatch(ui,/shell:allow-execute|Command\.create\(/);
});

test('FIN04 rejects forged job ids and hostile reporter navigation hints',()=>{
 const base={projectId:id,jobId:'job-'+'b'.repeat(32),
  taskAcceptanceAuthorized:false,independentlyVerified:false,
  cases:[{id:'case_'+'c'.repeat(64),name:'failing_case',
   status:'reported_fail',source:{relativePath:'tests/expected.py'},
   independentlyVerified:false}]};
 assert.equal(checkedTestExplorer(discovery,base,id).cases[0].canReviewSource,true);
 assert.equal(checkedTestExplorer(discovery,{...base,jobId:'foreign'},id).cases.length,0);
 for(const path of ['../secret.py','/etc/passwd','tests//bad.py','tests/./bad.py',
   'C:/Windows/secret','tests/\\\\evil.py']){
  assert.equal(checkedTestExplorer(discovery,{...base,cases:[{
   ...base.cases[0],source:{relativePath:path}}]},id),null);
 }
});

test('FIN04 rejects duplicate reported case identities',()=>{
 const one={id:'case_'+'d'.repeat(64),name:'one',status:'reported_pass',
  independentlyVerified:false};
 const report={projectId:id,jobId:'job-'+'e'.repeat(32),
  taskAcceptanceAuthorized:false,independentlyVerified:false,
  cases:[one,{...one,name:'two'}]};
 assert.equal(checkedTestExplorer(discovery,report,id),null);
});
