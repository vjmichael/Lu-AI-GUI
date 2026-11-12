import test from 'node:test';
import assert from 'node:assert/strict';
import {checkedFin06Journey,checkedFin06Destination}
  from '../web/fin06_guided_workflow.js';
const id='ide-'+'a'.repeat(32);
const ids=['editor','language','build','debug','source','acceptance'];
function workbench(){
 const labels={editor:'explorer',language:'languages',build:'tasks',
   debug:'tasks',source:'source',acceptance:'continuity',
   packages:'packages',terminal:'terminal',continuity:'continuity'};
 return{projectId:id,kind:'non_atomic_read_only_observations',
   executionAuthorized:false,recoveryRequired:false,
   rows:Object.entries(labels).map(([key,target])=>({
     id:key,target,state:'unverified',executionAuthorized:false,
     taskAccepted:false,
   }))};
}
test('FIN06 seven manual stages never imply execution or acceptance',()=>{
 const w=checkedFin06Journey(workbench(),id,null,null,null);
 assert.equal(w.steps.length,7);
 assert.equal(w.executionAuthorized,false);
 assert.equal(w.anyTaskAutomaticallyAccepted,false);
 assert.equal(checkedFin06Destination(w,'tests',id).section,'test-explorer');
});
test('FIN06 stale project and invented native success fail closed',()=>{
 const sha='b'.repeat(64);
 const build={projectId:id,phase:'completed',
   terminalReceiptSha256:sha,installedAppAccepted:false,
   requiresReconciliation:false};
 const report={projectId:id,terminalReceiptSha256:sha,
   independentlyVerified:false,taskAcceptanceAuthorized:false};
 const w=checkedFin06Journey(workbench(),id,build,report,null);
 assert.equal(w.steps.find(x=>x.id==='tests').observation,
   'original_native_report_visible');
 assert.equal(checkedFin06Destination(w,'git','ide-'+'c'.repeat(32)),null);
 const altered=checkedFin06Journey(workbench(),id,
   {...build,requiresReconciliation:true},report,null);
 assert.equal(altered.steps.find(x=>x.id==='tests').observation,'not_observed');
});
