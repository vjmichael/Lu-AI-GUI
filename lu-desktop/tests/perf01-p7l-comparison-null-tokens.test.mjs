import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';

const head='a'.repeat(40);
const sha=x=>x.repeat(64);
function receipt(){
  const samples=[];
  for(let trial=1;trial<=5;trial++){
    for(const caseId of ['text_exact','text_code']){
      for(const runner of ['baseline','candidate']){
        samples.push({
          caseId,runner,trial,
          firstForwardMs:40+trial,totalMs:140+trial,
          // The native P7L generator does not own decoded token accounting.
          generatedTokens:null,decodeMs:null,decodeTokens:null,
          qualityPass:true,cancelReaped:false,restartedSafely:false,
          outputSha256:sha('f'),textChunkCount:2,
        });
      }
    }
  }
  return {
    schema:'lu-perf01-p7-receipts-v1',sourceHead:head,
    hardwareId:'test-fixture-unattested',
    modelSha256:sha('b'),projectorSha256:null,suiteSha256:sha('c'),
    contextTokens:4096,
    baseline:{backend:'llama_cpp',engineSha256:sha('d')},
    candidate:{backend:'koboldcpp',engineSha256:sha('e')},
    samples,
  };
}
function run(input,sourceHead=head){
  const dir=mkdtempSync(join(tmpdir(),'lu-p7l-analyzer-'));
  try{
    const file=join(dir,'receipt.json');
    writeFileSync(file,JSON.stringify(input));
    return spawnSync(process.execPath,[
      'scripts/perf01-p7-backend-comparison.mjs',
      '--input',file,'--head',sourceHead
    ],{cwd:fileURLToPath(new URL('../..',import.meta.url)),
      encoding:'utf8',timeout:10000});
  }finally{
    rmSync(dir,{recursive:true,force:true});
  }
}
test('P7L real harness-shaped paired receipts do not invent tokens or promote',()=>{
  const actual=run(receipt());
  assert.equal(actual.status,0,actual.stderr);
  const report=JSON.parse(actual.stdout);
  assert.equal(report.comparisons.text_exact.candidate.firstForwardMs.p50,43);
  assert.equal(report.comparisons.text_code.candidate.estimatedDecodeTokensPerSecond,null);
  assert.equal(report.missingCases.includes('vision_mmproj'),true);
  assert.equal(report.performanceImprovementAccepted,false);
  assert.equal(report.installedBackendPromotionAllowed,false);
});
test('P7L refuses fabricated token counts, stale SHA and missing pair',()=>{
  const wrong=receipt();
  wrong.samples[0].decodeTokens=15;
  assert.equal(run(wrong).status,1);
  assert.equal(run(receipt(),'f'.repeat(40)).status,1);
  const incomplete=receipt();
  incomplete.samples.pop();
  assert.equal(run(incomplete).status,1);
});
