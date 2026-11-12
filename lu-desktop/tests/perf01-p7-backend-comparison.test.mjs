import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const head='a'.repeat(40);
function fixture(full=false){
  const included=full?[
    'text_exact','text_code','vision_mmproj','structured_tools',
    'cancel_stream','restart_recovery'
  ]:['text_exact','text_code'];
  const samples=[];
  for(const caseId of included){
    for(let trial=1;trial<=5;trial++){
      for(const runner of ['baseline','candidate']){
        samples.push({
          runner,caseId,trial,totalMs:160+trial,
          firstForwardMs:50+trial,generatedTokens:12,
          qualityPass:true,cancelReaped:true,restartedSafely:true,
          decodeTokens:10,decodeMs:90,
        });
      }
    }
  }
  return {schema:'lu-perf01-p7-receipts-v1',sourceHead:head,
    hardwareId:'test-only-host-not-attested',modelSha256:'b'.repeat(64),
    projectorSha256:full?'c'.repeat(64):null,
    suiteSha256:'d'.repeat(64),contextTokens:4096,
    baseline:{backend:'llama_cpp',engineSha256:'e'.repeat(64)},
    candidate:{backend:'mistral_rs',engineSha256:'f'.repeat(64)},
    samples};
}
function run(f,providedHead=head){
  const root=mkdtempSync(join(tmpdir(),'lu-p7-eval-'));
  try{
    const path=join(root,'test.json');
    writeFileSync(path,JSON.stringify(f));
    return spawnSync(process.execPath,[
      'scripts/perf01-p7-backend-comparison.mjs',
      '--input',path,'--head',providedHead
    ],{cwd:fileURLToPath(new URL('../..',import.meta.url)),
      encoding:'utf8',timeout:10000});
  }finally{rmSync(root,{recursive:true,force:true})}
}
test('P7 paired text-only candidate reports missing feature parity, never promotion',()=>{
  const result=run(fixture());
  assert.equal(result.status,0,result.stderr);
  const x=JSON.parse(result.stdout);
  assert.equal(x.comparisons.text_exact.candidate.totalMs.p50,163);
  assert.equal(x.comparisons.text_code.baseline.estimatedDecodeTokensPerSecond.p95,111);
  assert.equal(x.candidateObservedBasicParity,false);
  assert.ok(x.missingCases.includes('vision_mmproj'));
  assert.equal(x.installedBackendPromotionAllowed,false);
  assert.equal(x.physicalOwnerAcceptance,false);
});
test('P7 full synthetic corpus still never asserts native real-model acceptance',()=>{
  const result=run(fixture(true));
  assert.equal(result.status,0,result.stderr);
  const x=JSON.parse(result.stdout);
  assert.equal(x.candidateObservedBasicParity,true);
  assert.equal(x.allModalityCasesPresent,true);
  assert.equal(x.exactModelRuntimeIdentityIndependentlyProven,false);
  assert.equal(x.installedBackendPromotionAllowed,false);
});
test('P7 refuses stale source, changed model and inappropriate backend identity',()=>{
  const input=fixture();
  assert.equal(run(input,'f'.repeat(40)).status,1);
  input.modelSha256='0'.repeat(64);
  assert.equal(run(input).status,1);
  input.modelSha256='b'.repeat(64);
  input.candidate.backend='some-remote-api';
  assert.equal(run(input).status,1);
});
test('P7 refuses missing samples, duplicate mode/trial and invalid timing',()=>{
  let x=fixture();x.samples.pop();
  assert.equal(run(x).status,1);
  x=fixture();x.samples.push({...x.samples[0]});
  assert.equal(run(x).status,1);
  x=fixture();x.samples[0].totalMs=30;
  assert.equal(run(x).status,1);
  x=fixture();x.samples[0].decodeMs=-1;
  assert.equal(run(x).status,1);
  x=fixture();x.samples[0].qualityPass='true';
  assert.equal(run(x).status,1);
});
