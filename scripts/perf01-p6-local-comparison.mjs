#!/usr/bin/env node
// PERF-01 P6: summarize actual owner-collected local inference receipts.
// Never launches engines, invents GPUs, reads private prompts, or accepts
// benchmark hardware solely because a label says "Radeon".
// Usage: node scripts/perf01-p6-local-comparison.mjs --input <json> --head <40 hex>
// Input: { sourceHead,engineSha256,modelSha256,hardwareId,observedGpuCount,
//          runs:[{mode,trial,ttftMs,totalMs,qualityPass}] }
// All three modes need >=5 IDENTICAL trial IDs with one successful observation
// per trial/mode. Physical performance is never independently accepted here.
import {readFileSync,lstatSync} from 'node:fs';
import {argv,exit} from 'node:process';
const arg=key=>{const i=argv.indexOf(key);return i<0?null:argv[i+1]};
const path=arg('--input'),head=arg('--head');
if(!path||!head||!/^[a-f0-9]{40}$/.test(head)){
  process.stderr.write('Usage: perf01-p6-local-comparison.mjs --input receipts.json --head <40 hex>\n');
  exit(2);
}
const sha=x=>typeof x==='string'&&/^[a-f0-9]{64}$/.test(x);
const modes=['single','layer_split','independent'];
const p=(arr,f)=>{const sorted=[...arr].sort((a,b)=>a-b);return sorted[Math.ceil(f*sorted.length)-1]};
function stats(values){
  if(!values.length)return null;
  return {samples:values.length,p50Ms:p(values,0.5),p95Ms:p(values,0.95),
    minMs:Math.min(...values),maxMs:Math.max(...values)};
}
try{
  const info=lstatSync(path);
  if(!info.isFile()||info.isSymbolicLink()||info.size>512*1024){
    throw Error('Unsafe benchmark evidence file');
  }
  const data=JSON.parse(readFileSync(path,'utf8'));
  if(!data||data.sourceHead!==head||
     !sha(data.engineSha256)||!sha(data.modelSha256)||
     typeof data.hardwareId!=='string'||
     !/^[a-zA-Z0-9_.-]{3,80}$/.test(data.hardwareId)||
     data.observedGpuCount!==2||
     !Array.isArray(data.runs)||data.runs.length<15||data.runs.length>300){
    throw Error('Missing or stale exact-head two-device identity');
  }
  const grouped=new Map(modes.map(mode=>[mode,new Map()]));
  for(const r of data.runs){
    if(!r||!modes.includes(r.mode)||
       !Number.isSafeInteger(r.trial)||r.trial<1||r.trial>20||
       !Number.isSafeInteger(r.ttftMs)||r.ttftMs<0||
       !Number.isSafeInteger(r.totalMs)||r.totalMs<r.ttftMs||
       typeof r.qualityPass!=='boolean'){
      throw Error('Invalid or unsupported run result');
    }
    const map=grouped.get(r.mode);
    if(map.has(r.trial))throw Error('Duplicate mode/trial');
    map.set(r.trial,r);
  }
  const trialIds=[...grouped.get('single').keys()].sort((a,b)=>a-b);
  if(trialIds.length<5||trialIds.length>20 ||
     modes.some(mode=>grouped.get(mode).size!==trialIds.length||
       trialIds.some(t=>!grouped.get(mode).has(t)))){
    throw Error('Require at least five complete paired trial IDs per GPU mode');
  }
  const output={schema:'lu-perf01-p6-local-comparison-v1',
    expectedHead:head,hardwareId:data.hardwareId,
    engineSha256:data.engineSha256,modelSha256:data.modelSha256,
    comparedModes:{},pairs:trialIds.length,
    independentDeviceConcurrencyVerified:false,
    gpuVramAvailabilityIndependentlyVerified:false,
    sourceHardwareAttestationIndependentlyVerified:false,
    qualityEquivalenceIndependentlyVerified:false,
    physicalAcceptance:false,comparativeSpeedupAccepted:false,
    status:'PAIRED_OWNER_RECEIPTS_SUMMARIZED_NOT_INDEPENDENTLY_ACCEPTED'};
  for(const mode of modes){
    const rows=trialIds.map(t=>grouped.get(mode).get(t));
    output.comparedModes[mode]={
      ttft:stats(rows.map(r=>r.ttftMs)),
      total:stats(rows.map(r=>r.totalMs)),
      exactQualityPasses:rows.filter(r=>r.qualityPass).length,
      exactQualityTotal:rows.length,
    };
  }
  process.stdout.write(JSON.stringify(output,null,2)+'\n');
}catch(error){
  process.stderr.write('PERF01_P6_COMPARISON_REFUSED '+String(error)+'\n');
  process.exitCode=1;
}
