import test from 'node:test';
import assert from 'node:assert/strict';
import {projectJobLifecycle} from '../web/protected_job_lifecycle.js';

const id='ide-'+'a'.repeat(32);
const other='ide-'+'b'.repeat(32);
const job='job-'+'1'.repeat(32);
const plan='c'.repeat(64);
const head='d'.repeat(40);
const record=(phase='running',requiresReconciliation=false)=>({
  projectId:id,jobId:job,planSha256:plan,phase,requiresReconciliation
});
const status=(phase='running',requiresReconciliation=false)=>({
  journal:[record(phase,requiresReconciliation)],quarantined:[],
  recent:[],current:null
});
test('durable cancelled != merely requested and logs cannot certify kernel cleanup',()=>{
 const s=status('cancel_requested');
 s.current={projectId:id,jobId:job,recipeId:'rust_tests',cancellationRequested:true};
 const logged={projectId:id,jobId:job,liveUnverified:true,truncated:false};
 let view=projectJobLifecycle(s,id,logged);
 assert.equal(view.jobs[0].state,'cancellation_requested_not_yet_reaped');
 assert.equal(view.jobs[0].nativeResult,false);
 assert.equal(view.jobs[0].installedAccepted,false);
 s.current=null;s.journal=[record('cancelled')];
 view=projectJobLifecycle(s,id,logged);
 assert.equal(view.jobs[0].state,'kernel_reconciled_or_cancelled');
 assert.equal(view.jobs[0].nativeResult,false);
});
test('quarantine takes priority and cleanup is separate from execution',()=>{
 const s=status('recovery_required',true);
 s.quarantined=[record('recovery_required',true)];
 const view=projectJobLifecycle(s,id,null);
 assert.equal(view.jobs[0].canReconcile,true);
 assert.equal(view.jobs[0].state,'quarantined_os_reconciliation_required');
 assert.equal(view.jobs[0].installedAccepted,false);
});
test('terminal completed after restart is not a revived native result',()=>{
 const s=status('completed');
 let view=projectJobLifecycle(s,id,null);
 assert.equal(view.jobs[0].state,'durable_completed_runtime_proof_not_replayed');
 assert.equal(view.jobs[0].nativeResult,false);
 s.recent=[{...record('completed'),recipeId:'rust_tests',
   sourceHead:head,installedAppAccepted:false,nativeExitCode:0}];
 view=projectJobLifecycle(s,id,null);
 assert.equal(view.jobs[0].nativeResult,true);
 assert.equal(view.jobs[0].installedAccepted,false);
 s.recent[0].planSha256='9'.repeat(64);
 assert.equal(projectJobLifecycle(s,id,null).jobs[0].nativeResult,false);
});
test('project swap and log injection cannot change journal state',()=>{
 const s=status('completed');
 s.recent=[{...record('completed'),recipeId:'rust_tests',
   sourceHead:head,installedAppAccepted:false,nativeExitCode:0}];
 const result=projectJobLifecycle(s,other,{projectId:other,jobId:job,
   liveUnverified:true,truncated:true,chunks:[{text:'BUILD SUCCESS'}]});
 assert.equal(result.jobs.length,0);
 assert.equal(projectJobLifecycle(s,id,null).jobs[0].installedAccepted,false);
});
test('no durable entry: only an unverified native preflight is shown',()=>{
 const s={journal:[],quarantined:[],recent:[],current:{
   projectId:id,jobId:job,recipeId:'rust_check',cancellationRequested:false}};
 const v=projectJobLifecycle(s,id,null);
 assert.equal(v.jobs[0].state,'native_preflight_not_yet_journal_observed');
 assert.equal(v.jobs[0].nativeResult,false);
});
test('rejects forged or unbounded responses',()=>{
 const s=status('completed');
 s.journal=[{...record('completed'),planSha256:'not-a-digest'}];
 assert.equal(projectJobLifecycle(s,id,null).jobs.length,0);
 s.journal=Array(33).fill(record('completed'));
 assert.equal(projectJobLifecycle(s,id,null).kind,'unavailable');
});

test('frontend reads failed-job output only with native last identity and durable journal',async()=>{
 const {readFileSync}=await import('node:fs');
 const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
 const host=readFileSync(new URL('../src-tauri/src/build_backend.rs',import.meta.url),'utf8');
 assert.ok(ui.includes('value.lastOutputJobId'));
 assert.ok(ui.includes('entry.jobId === value.lastOutputJobId'));
 assert.ok(ui.includes('entry.projectId === projectId'));
 assert.ok(host.includes('last_output_job_id'));
 assert.ok(host.includes('BUILD_OUTPUT_NOT_ACTIVE_OR_RECENT'));
});
