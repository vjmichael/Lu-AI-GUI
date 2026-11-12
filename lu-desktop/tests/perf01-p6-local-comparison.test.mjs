import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const head='a'.repeat(40);
function fixture(){
  const runs=[];
  for(let i=1;i<=5;i++){
    for(const [mode,offset] of [
      ['single',0],['layer_split',10],['independent',20]
    ])runs.push({
      mode,trial:i,ttftMs:100+offset+i,
      totalMs:200+offset+i,qualityPass:true,
    });
  }
  return {sourceHead:head,engineSha256:'b'.repeat(64),
    modelSha256:'c'.repeat(64),hardwareId:'win-amd-fixture',
    observedGpuCount:2,runs};
}
function execute(input,sourceHead=head){
  const temp=mkdtempSync(join(tmpdir(),'lu-p6-bench-'));
  try{
    const path=join(temp,'bench.json');
    writeFileSync(path,JSON.stringify(input));
    return spawnSync(process.execPath,[
      'scripts/perf01-p6-local-comparison.mjs','--input',path,
      '--head',sourceHead,
    ],{cwd:fileURLToPath(new URL('../..',import.meta.url)),
      encoding:'utf8',timeout:10000});
  }finally{rmSync(temp,{recursive:true,force:true})}
}
test('P6 analyzer compares only exact-head complete paired modes and never accepts hardware',()=>{
  const r=execute(fixture());
  assert.equal(r.status,0,r.stderr);
  const parsed=JSON.parse(r.stdout);
  assert.equal(parsed.pairs,5);
  assert.equal(parsed.comparedModes.single.ttft.p50Ms,103);
  assert.equal(parsed.comparedModes.layer_split.total.p95Ms,215);
  assert.equal(parsed.comparedModes.independent.exactQualityPasses,5);
  assert.equal(parsed.physicalAcceptance,false);
  assert.equal(parsed.comparativeSpeedupAccepted,false);
  assert.equal(parsed.gpuVramAvailabilityIndependentlyVerified,false);
});
test('P6 rejects missing trials, stale source and claims of one GPU as two',()=>{
  const data=fixture();
  data.runs.pop();
  let r=execute(data);
  assert.equal(r.status,1);
  assert.match(r.stderr,/PERF01_P6_COMPARISON_REFUSED/);
  const valid=fixture();
  r=execute(valid,'f'.repeat(40));
  assert.equal(r.status,1);
  valid.observedGpuCount=1;
  r=execute(valid);
  assert.equal(r.status,1);
});
test('P6 rejects duplicated trial, false quality type and impossible negative time',()=>{
  const duplicated=fixture();
  duplicated.runs.push(duplicated.runs[0]);
  let r=execute(duplicated);assert.equal(r.status,1);
  const badQuality=fixture();badQuality.runs[0].qualityPass='true';
  r=execute(badQuality);assert.equal(r.status,1);
  const badTime=fixture();badTime.runs[0].totalMs=1;
  r=execute(badTime);assert.equal(r.status,1);
});
