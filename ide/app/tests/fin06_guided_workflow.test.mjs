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

test('FIN06 recovery prevents navigation and native success promotion',()=>{
 const w=workbench();w.recoveryRequired=true;
 const journey=checkedFin06Journey(w,id,null,null,null);
 assert.equal(journey.workerRecoveryRequired,true);
 assert.equal(checkedFin06Destination(journey,'git',id),null);
 const sha='f'.repeat(64);
 const completed={projectId:id,phase:'completed',
  terminalReceiptSha256:sha,installedAppAccepted:false,
  requiresReconciliation:false};
 assert.equal(checkedFin06Journey(w,id,completed,null,null).steps
   .find(row=>row.id==='build').observation,'unverified');
});

test('FIN06B operational readiness distinguishes native observations from authority',async()=>{
 const {checkedFin06OperationalReadiness}=await import('../web/fin06_guided_workflow.js');
 const journey=checkedFin06Journey(workbench(),id,null,null,null);
 const signals={
  git:{projectId:id,state:'local_repository_inspected',truncated:false},
  build:{ready:true,globalRecoveryRequired:false,globalWorkerBusy:false,
    activeInAnotherProject:false},
  debug:{ready:true,projectDebugSupported:true,globalRecoveryRequired:false,
    activeInAnotherProject:false,orphanedDebugActionRequired:false},
  packages:[{reviewAvailable:true,installAuthorized:false}]
 };
 const v=checkedFin06OperationalReadiness(journey,id,signals);
 assert.equal(v.gitObserved,true);
 assert.equal(v.buildWorkerObserved,true);
 assert.equal(v.debuggerLaunchObserved,true);
 assert.equal(v.packageReviewObserved,true);
 assert.equal(v.productionDebuggerApproved,false);
 assert.equal(v.packageInstallApproved,false);
 assert.equal(v.executionAuthorized,false);
 const interrupted=checkedFin06OperationalReadiness(journey,id,{
  ...signals,build:{...signals.build,globalRecoveryRequired:true}});
 assert.equal(interrupted.blockedByRecovery,true);
 assert.equal(interrupted.buildWorkerObserved,false);
 assert.equal(interrupted.debuggerLaunchObserved,false);
 assert.equal(checkedFin06OperationalReadiness(journey,'ide-'+'c'.repeat(32),signals),null);
 assert.equal(checkedFin06OperationalReadiness(journey,id,{
  ...signals,git:{...signals.git,projectId:'ide-'+'c'.repeat(32)}}).gitObserved,false);
});
