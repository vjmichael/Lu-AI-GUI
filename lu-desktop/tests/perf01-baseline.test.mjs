import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
const fixtureHead='a'.repeat(40);
function run(rows,head=fixtureHead){
  const root=mkdtempSync(join(tmpdir(),'lu-perf01-negative-'));
  try{
    const file=join(root,'host.json');
    writeFileSync(file,JSON.stringify(rows));
    return spawnSync(process.execPath,[
      'scripts/perf01-host-baseline.mjs',
      '--input',file,'--head',head
    ],{cwd:new URL('../..',import.meta.url),encoding:'utf8',timeout:10000});
  }finally{
    rmSync(root,{recursive:true,force:true});
  }
}
function row(i,route){
  return {
    chatId:'chat-private',generationId:'g-'+i,
    route,reasonCode:route==='CPU_DETERMINISTIC'?
      'EXACT_CURRENT_CHAT_LITERAL':'MODEL_REASONING_REQUIRED',
    terminalState:'COMPLETED',
    evidenceScope:'HOST_OBSERVED_ONLY',sourceHead:fixtureHead,
    operatingSystem:'fixture-not-real-host',architecture:'fixture',
    modelLoadedAtEntry:route==='MODEL_INFERENCE',
    modelStartedByThisRoute:false,
    queueWaitMs:route==='MODEL_INFERENCE'?5:null,
    contextAssemblyMs:route==='MODEL_INFERENCE'?10:null,
    modelDispatchToFirstChunkMs:route==='MODEL_INFERENCE'?15:null,
    originalSourceBytesScanned:route==='CPU_DETERMINISTIC'?200:null,
    originalEntriesScanned:route==='CPU_DETERMINISTIC'?2:null,
    sourceResultComplete:route==='CPU_DETERMINISTIC'?true:null,
    streamedUtf8Bytes:route==='MODEL_INFERENCE'?100:0,
    overallLatencyMs:i*10,
  };
}
test('PERF01 single samples cannot be advertised as a reliable baseline',()=>{
  const r=run([row(1,'CPU_DETERMINISTIC'),row(2,'MODEL_INFERENCE')]);
  assert.equal(r.status,1,r.stderr);
  const v=JSON.parse(r.stdout);
  assert.equal(v.status,'INSUFFICIENT_REPEATS_FOR_BASELINE');
  assert.equal(v.comparativePerformanceClaim,false);
});
test('PERF01 synthetic fixtures test percentile math but never count as device acceptance',()=>{
  const rows=[];
  for(let i=1;i<=5;i++){
    rows.push(row(i,'CPU_DETERMINISTIC'),row(i+5,'MODEL_INFERENCE'));
  }
  const r=run(rows);
  assert.equal(r.status,0,r.stderr);
  const v=JSON.parse(r.stdout);
  assert.equal(v.status,'LOCAL_HOST_BASELINE_SUMMARIZED_NOT_INDEPENDENTLY_ACCEPTED');
  assert.equal(v.routeGroups.CPU_DETERMINISTIC.total.p50Ms,30);
  assert.equal(v.routeGroups.CPU_DETERMINISTIC.total.p95Ms,50);
  assert.equal(v.firstPartyPhysicalDeviceAccepted,false);
  assert.equal(v.comparativePerformanceClaim,false);
  assert.equal(v.tokenThroughputUnmeasured,true);
});
test('PERF01 changed source identity or missing host-observed evidence fails closed',()=>{
  const rows=Array.from({length:5},(_,i)=>row(i+1,'CPU_DETERMINISTIC'));
  const stale=run(rows,'b'.repeat(40));
  assert.equal(stale.status,1);
  assert.match(stale.stderr,/PERF01_BASELINE_REFUSED/);
  rows[0].sourceHead=null;
  const absent=run(rows);
  assert.equal(absent.status,1);
  assert.match(absent.stderr,/PERF01_BASELINE_REFUSED/);
  rows[0].sourceHead=fixtureHead;
  rows[0].modelStartedByThisRoute=true;
  const startup=run(rows);
  assert.equal(startup.status,1);
  assert.match(startup.stderr,/PERF01_BASELINE_REFUSED/);
});
