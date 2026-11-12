#!/usr/bin/env node
// PERF-01 P7: original-host receipt comparison for alternate local inference.
// This command never starts a model, accepts an arbitrary network URL, runs
// executable argv, reads prompts, or attests physical acceptance.
// Required: two exact same-GGUF runners, exact source head, matched corpus,
// >=5 paired trials per case. Partial modality coverage is reported honestly.
import {readFileSync,lstatSync} from 'node:fs';
import {argv,exit} from 'node:process';
const arg=(name)=>{const i=argv.indexOf(name);return i<0?null:argv[i+1]};
const file=arg('--input'),head=arg('--head');
const sha64=(v)=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v)&&!/^0+$/.test(v);
const alphabet=(v)=>typeof v==='string'&&/^[a-zA-Z0-9_.-]{3,80}$/.test(v);
const candidates=new Set(['mistral_rs','koboldcpp','vllm_gguf']);
const cases=['text_exact','text_code','vision_mmproj','structured_tools',
             'cancel_stream','restart_recovery'];
const essential=['text_exact','text_code'];
const number=(v)=>Number.isSafeInteger(v)&&v>=0;
const percentile=(values,p)=>{
  const a=[...values].sort((x,y)=>x-y);
  return a[Math.ceil(a.length*p)-1];
};
const summary=(values)=>values.length?{
  samples:values.length,
  p50:percentile(values,0.5),
  p95:percentile(values,0.95),
  minimum:Math.min(...values),
  maximum:Math.max(...values),
}:null;
if(!file||!head||!/^[a-f0-9]{40}$/.test(head)){
  process.stderr.write('Usage: perf01-p7-backend-comparison.mjs --input receipts.json --head <exact 40-hex source SHA>\n');
  exit(2);
}
try{
  const stat=lstatSync(file);
  if(!stat.isFile()||stat.isSymbolicLink()||stat.size>1024*1024){
    throw Error('Missing, symlinked or oversized original-host comparison file');
  }
  const data=JSON.parse(readFileSync(file,'utf8'));
  if(!data||data.schema!=='lu-perf01-p7-receipts-v1'||
      data.sourceHead!==head||!alphabet(data.hardwareId)||
      !sha64(data.modelSha256)||!sha64(data.suiteSha256)||
      (data.projectorSha256!==null&&!sha64(data.projectorSha256))||
      !number(data.contextTokens)||data.contextTokens<256||
      data.contextTokens>1048576||
      data.baseline?.backend!=='llama_cpp'||
      !sha64(data.baseline.engineSha256)||
      !candidates.has(data.candidate?.backend)||
      !sha64(data.candidate.engineSha256)||
      !Array.isArray(data.samples)||data.samples.length<20||
      data.samples.length>480){
    throw Error('Missing exact-head/model/engine/suite identity or bounded samples');
  }
  const rows=new Map();
  const covered=new Set();
  for(const item of data.samples){
    if(!item||!['baseline','candidate'].includes(item.runner)||
       !cases.includes(item.caseId)||
       !Number.isInteger(item.trial)||item.trial<1||item.trial>20||
       !number(item.totalMs)||!number(item.firstForwardMs)||
       item.firstForwardMs>item.totalMs||
       (item.generatedTokens!==null&&
          (!number(item.generatedTokens)||item.generatedTokens>32768))||
       typeof item.qualityPass!=='boolean'||
       typeof item.cancelReaped!=='boolean'||
       typeof item.restartedSafely!=='boolean'||
       (item.decodeMs!==null&&
           (!number(item.decodeMs)||item.decodeMs>item.totalMs))||
       (item.decodeTokens!==null&&
           (item.generatedTokens===null||
            !number(item.decodeTokens)||item.decodeTokens>item.generatedTokens))){
      throw Error('Invalid timing, quality or custody measurement');
    }
    if((item.decodeMs===null)!==(item.decodeTokens===null)){
      throw Error('Decode timing and token count must be measured together');
    }
    const key=`${item.runner}/${item.caseId}/${item.trial}`;
    if(rows.has(key))throw Error('Duplicate original-host sample identity');
    rows.set(key,item);
    covered.add(item.caseId);
  }
  if(essential.some(id=>!covered.has(id))){
    throw Error('Both mandatory text and code cases must be measured');
  }
  if(covered.has('vision_mmproj')&&data.projectorSha256===null){
    throw Error('Vision comparison requires exact original projector SHA-256');
  }
  const sampleCounts={};
  const comparisons={};
  for(const id of [...covered].sort()){
    const base=[...rows.values()].filter(s=>s.runner==='baseline'&&s.caseId===id);
    const candidate=[...rows.values()].filter(s=>s.runner==='candidate'&&s.caseId===id);
    const baseTrials=new Set(base.map(s=>s.trial));
    const candidateTrials=new Set(candidate.map(s=>s.trial));
    if(base.length<5||base.length>20||candidate.length!==base.length||
       candidateTrials.size!==baseTrials.size||
       [...baseTrials].some(t=>!candidateTrials.has(t))){
      throw Error('Each included case requires 5-20 exactly paired host trials');
    }
    sampleCounts[id]=base.length;
    const modeSummary=(list)=>{
      const decode=list.filter(s=>s.decodeMs!==null&&s.decodeMs>0);
      return {
        totalMs:summary(list.map(s=>s.totalMs)),
        firstForwardMs:summary(list.map(s=>s.firstForwardMs)),
        estimatedDecodeTokensPerSecond:decode.length===list.length
          ?summary(decode.map(s=>Math.round(s.decodeTokens*1000/s.decodeMs)))
          :null,
        qualityPasses:list.filter(s=>s.qualityPass).length,
        sampleCount:list.length,
        nativeCancelReapedPasses:list.filter(s=>s.cancelReaped).length,
        restartRecoveryPasses:list.filter(s=>s.restartedSafely).length,
      };
    };
    comparisons[id]={baseline:modeSummary(base),candidate:modeSummary(candidate)};
  }
  // An omitted test is NOT an unsupported capability and certainly not a PASS.
  const missing=cases.filter(id=>!covered.has(id));
  const fullSuite=missing.length===0 && data.projectorSha256!==null;
  const parityObserved=fullSuite&&cases.every(id=>{
    const v=comparisons[id];
    return v.candidate.qualityPasses===v.candidate.sampleCount&&
      v.candidate.nativeCancelReapedPasses===v.candidate.sampleCount&&
      v.candidate.restartRecoveryPasses===v.candidate.sampleCount;
  });
  const result={
    schema:'lu-perf01-p7-comparison-v1',
    evidenceScope:'OWNER_SUPPLIED_RECEIPTS_NOT_INDEPENDENTLY_ATTESTED',
    status:'REVIEW_ONLY_NOT_PRODUCTION_ACCEPTANCE',
    sourceHead:head,hardwareId:data.hardwareId,
    modelSha256:data.modelSha256,
    suiteSha256:data.suiteSha256,
    contextTokens:data.contextTokens,
    baseline:data.baseline,
    candidate:data.candidate,
    testedCases:[...covered].sort(),
    missingCases:missing,
    trialCounts:sampleCounts,comparisons,
    allModalityCasesPresent:fullSuite,
    candidateObservedBasicParity:parityObserved,
    exactModelRuntimeIdentityIndependentlyProven:false,
    backendBinaryCustodyIndependentlyProven:false,
    independentMMProjAccuracyProven:false,
    modelOutputOrToolAuthorizationGranted:false,
    trustedDevicePowerOrVramMeasured:false,
    performanceImprovementAccepted:false,
    installedBackendPromotionAllowed:false,
    physicalOwnerAcceptance:false,
    note:'Only a host and owner native replay against the exact installed engine/model may establish real backend parity. Do not infer from these data fields or cherry-picked timings.',
  };
  process.stdout.write(JSON.stringify(result,null,2)+'\n');
}catch(e){
  process.stderr.write('PERF01_P7_COMPARISON_REFUSED '+String(e)+'\n');
  process.exitCode=1;
}
