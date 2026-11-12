#!/usr/bin/env node
// PERF-01 P0 original host-observed baseline analyzer. Never runs GGUF,
// creates an authorization, or invents absent GPU/token information.
// Input is the JSON returned by the read-only lu_perf01_recent command,
// gathered from an original installed host, NOT synthetic timings.
import {readFileSync,lstatSync} from 'node:fs';
import {argv,exit} from 'node:process';

const args=(key)=>{const i=argv.indexOf(key);return i>=0?argv[i+1]:null};
const file=args('--input'),expectedHead=args('--head');
const priorityMode=args('--priority');
if(priorityMode!==null&&priorityMode!=='foreground-background'){
  process.stderr.write('PERF01_BASELINE_REFUSED unknown scheduler comparison mode\n');
  exit(2);
}
if(!file||!expectedHead||!/^[a-f0-9]{40}$/.test(expectedHead)){
  process.stderr.write('Usage: node scripts/perf01-host-baseline.mjs --input host-receipts.json --head <exact 40-char SHA>\n');
  exit(2);
}
try{
  const stat=lstatSync(file);
  if(!stat.isFile()||stat.isSymbolicLink()||stat.size>2*1024*1024){
    throw Error('unsafe, missing or oversized original receipt file');
  }
  const parsed=JSON.parse(readFileSync(file,'utf8'));
  if(!Array.isArray(parsed)||parsed.length===0||parsed.length>1024){
    throw Error('no bounded original Lu host receipt array');
  }
  const acceptable=new Set(['CPU_DETERMINISTIC','MODEL_INFERENCE']);
  const all=[];
  for(const x of parsed){
    if(typeof x!=='object'||x===null||typeof x.chatId!=='string'
        ||typeof x.generationId!=='string'||x.generationId.length>128
        ||!acceptable.has(x.route)
        ||x.evidenceScope!=='HOST_OBSERVED_ONLY'
        ||x.terminalState!=='COMPLETED'
        ||x.sourceHead!==expectedHead
        ||(x.route==='CPU_DETERMINISTIC'&&x.modelStartedByThisRoute!==false)
        ||(x.route==='MODEL_INFERENCE'&&x.modelStartedByThisRoute!==null
           &&x.modelStartedByThisRoute!==false
           &&x.modelStartedByThisRoute!==true)
        ||typeof x.overallLatencyMs!=='number'
        ||!Number.isSafeInteger(x.overallLatencyMs)
        ||x.overallLatencyMs<0
        ||typeof x.streamedUtf8Bytes!=='number'
        ||!Number.isSafeInteger(x.streamedUtf8Bytes)
        ||x.streamedUtf8Bytes<0){
      throw Error('missing/forged/stale original host measurement fields');
    }
    if(x.route==='CPU_DETERMINISTIC'&&
      (x.originalSourceBytesScanned!==null&&
       (!Number.isSafeInteger(x.originalSourceBytesScanned)
        ||x.originalSourceBytesScanned<0))){
      throw Error('untrusted CPU source byte count');
    }
    all.push(x);
  }
  const calc=(values)=>{
    if(!values.length)return null;
    const a=[...values].sort((x,y)=>x-y);
    const percentile=(q)=>a[Math.ceil(q*a.length)-1];
    return {samples:a.length,minMs:a[0],p50Ms:percentile(0.5),
      p95Ms:percentile(0.95),maxMs:a.at(-1),
      meanMs:Math.round(a.reduce((n,v)=>n+v,0)/a.length)};
  };
  const byRoute={};
  for(const route of acceptable){
    const rows=all.filter(v=>v.route===route);
    byRoute[route]={
      total:calc(rows.map(v=>v.overallLatencyMs)),
      queueWait:calc(rows.flatMap(v=>
        v.queueWaitMs==null?[]:[v.queueWaitMs])),
      dispatchToFirstText:calc(rows.flatMap(v=>
        v.modelDispatchToFirstChunkMs==null?[]:
        [v.modelDispatchToFirstChunkMs])),
      sourceEntries:rows.map(v=>v.originalEntriesScanned)
        .filter(v=>Number.isSafeInteger(v)),
      completeSourceScans:rows.filter(v=>
        v.sourceResultComplete===true).length,
      partialSourceScans:rows.filter(v=>
        v.sourceResultComplete===false).length,
    };
  }
  const min=5;
  const enough=Object.values(byRoute).every(x=>x.total&&x.total.samples>=min);
  // P4: these remain actual HOST-observed latency distributions, not a
  // hardware attestation, renderer paint measurement or scheduling proof.
  // Require separate repeated foreground/background model samples before
  // stating even that a two-class comparison dataset exists.
  const schedulerGroups={};
  for(const label of ['FOREGROUND_HUMAN','BACKGROUND_TERMINAL']){
    const rows=all.filter(x=>x.route==='MODEL_INFERENCE'
      &&x.schedulerClass===label
      &&Number.isSafeInteger(x.turnToFirstWebviewForwardMs)
      &&x.turnToFirstWebviewForwardMs>=0
      &&Number.isSafeInteger(x.queueWaitMs)
      &&x.queueWaitMs>=0);
    schedulerGroups[label]={
      total:calc(rows.map(x=>x.overallLatencyMs)),
      queueWait:calc(rows.map(x=>x.queueWaitMs)),
      firstForward:calc(rows.map(x=>x.turnToFirstWebviewForwardMs)),
    };
  }
  const priorityEvidenceSufficient=Object.values(schedulerGroups)
    .every(group=>group.total&&group.total.samples>=min);
  const result={
    schema:'lu-perf01-p0-baseline-v1',expectedHead,
    status:priorityMode
      ?(priorityEvidenceSufficient
        ?'HOST_PRIORITY_SAMPLES_SUMMARIZED_NOT_INDEPENDENTLY_ACCEPTED'
        :'INSUFFICIENT_REPEATS_FOR_INTERACTIVE_PRIORITY')
      :(enough
        ?'LOCAL_HOST_BASELINE_SUMMARIZED_NOT_INDEPENDENTLY_ACCEPTED'
        :'INSUFFICIENT_REPEATS_FOR_BASELINE'),
    originalHostReceipts:all.length,
    minimumPerRoute:min,
    schedulerGroups:priorityMode?schedulerGroups:null,
    priorityEvidenceSufficient:priorityMode?priorityEvidenceSufficient:null,
    priorityComparisonIndependentlyAccepted:false,
    routeGroups:byRoute,
    comparativePerformanceClaim:false,
    firstPartyPhysicalDeviceAccepted:false,
    tokenThroughputUnmeasured:true,
    gpuCpuUtilizationUnmeasured:true,
    sourceHeadMustBeOriginalInstalledBinary:true,
    note:'Local file SHA/head is not a signed machine attestation. Independently verify installed source/head and repeat the fixed corpus on the same hardware.',
  };
  process.stdout.write(JSON.stringify(result,null,2)+'\n');
  if(priorityMode?!priorityEvidenceSufficient:!enough)process.exitCode=1;
}catch(e){
  process.stderr.write('PERF01_BASELINE_REFUSED '+String(e)+'\n');
  process.exitCode=1;
}
