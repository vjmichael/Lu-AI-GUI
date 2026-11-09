import assert from 'node:assert/strict';
import test from 'node:test';
import {ingestJobProgress, visibleJobProgress, MAX_JOB_PROGRESS_EVENTS} from '../web/job_progress.js';
const projectId = 'ide-' + 'a'.repeat(32);
const job = (id, phase='running') => ({projectId, jobId:id, recipeId:'rust_tests', phase});
const status = (current=null, recent=[], quarantined=[]) => ({
  ready:true,current,recent,quarantined,journal:[]});
test('native snapshots produce one event for each phase, not per poll', () => {
  const running=status(job('job-1'));
  const a=ingestJobProgress([],projectId,running);
  assert.equal(a.length,1);
  assert.deepEqual(ingestJobProgress(a,projectId,running),a);
  const b=ingestJobProgress(a,projectId,status(job('job-1','cancel_requested')));
  assert.deepEqual(b.map(x=>x.phase),['running','cancel_requested']);
  assert.equal(b[1].ordinal,2);
});
test('different project or malicious text never appears', () => {
  const wrong={...job('wrong'),projectId:'ide-'+'b'.repeat(32)};
  const hostile={...job('bad'),'phase':'<script>alert(1)</script>'};
  const unreviewed={...job('unknown'),recipeId:'shell'};
  const events=ingestJobProgress([],projectId,status(wrong,[hostile,unreviewed,job('safe')]));
  assert.deepEqual(events.map(x=>x.jobId),['safe']);
  assert.deepEqual(visibleJobProgress(events,'ide-'+'b'.repeat(32)),[]);
});
test('quarantine remains visible, without declaring recovery complete', () => {
  const entries=ingestJobProgress([],projectId,
    status(null,[],[job('job-x','recovery_required')]));
  assert.equal(entries[0].phase,'recovery_required');
});
test('bounded adversarial status and history', () => {
  let history=[];
  for(let i=0;i<110;i++) history=ingestJobProgress(history,projectId,status(job('j-'+i)));
  assert.equal(history.length,MAX_JOB_PROGRESS_EVENTS);
  const many=Array.from({length:50},(_,i)=>job('q-'+i));
  const out=ingestJobProgress([],projectId,status(null,many));
  assert.equal(out.length,8);
});
test('missing native status cannot manufacture an event',()=>{
  assert.deepEqual(ingestJobProgress([],projectId,null),[]);
  assert.deepEqual(ingestJobProgress([],projectId,{}),[]);
});
