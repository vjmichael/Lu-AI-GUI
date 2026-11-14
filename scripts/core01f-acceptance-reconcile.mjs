#!/usr/bin/env node
// CORE-01F-04 metadata reconciliation against ORIGINAL GitHub run identities.
// READ ONLY GitHub API. Metadata is not kernel, log or human installed acceptance.
const SHA=/^[a-f0-9]{40}$/,DIGEST=/^sha256:[a-f0-9]{64}$/;
const BRANCH='lu/core01-source-convergence-no-ide-no-kobold-20261112';
const WORKFLOW='.github/workflows/lu-repo01-full-source.yml';
const INSTALLED='.github/workflows/lu-plan07d-installed.yml';
export const NATIVE=[
 ['verify-source',null],
 ['P0-GATE exact-head compiler diagnostics (windows-2025)','lu-p0-compiler-windows-2025'],
 ['P0-GATE exact-head compiler diagnostics (ubuntu-24.04)','lu-p0-compiler-ubuntu-24.04'],
 ['P0-WIN-CHAT native model/tool, image and UI (windows-2025)','lu-p0-win-chat-native-windows-2025'],
 ['P0-WIN-CHAT native model/tool, image and UI (ubuntu-24.04)','lu-p0-win-chat-native-ubuntu-24.04'],
 ['P0-WIN-CHAT real Qwen native tool choice and SmolVLM MMProj pixel inference','lu-p0-win-chat-genuine-model'],
 ['CORE-01E independent SEC-06 (source)','lu-core01e-sec06-source'],
 ['CORE-01E independent SEC-06 (linux)','lu-core01e-sec06-linux'],
 ['CORE-01E independent SEC-06 (windows)','lu-core01e-sec06-windows'],
 ['SEC-06C original root-owned reviewed Git (Linux)','lu-sec06c-root-owned-git'],
 ['SEC-06C original root-owned finite media/metadata (Linux)','lu-sec06c-finite-probes'],
 ['SEC-06C original root-owned scanner archive and finite utility (Linux)','lu-sec06c-native-scanner-utility'],
 ['SEC-06C real jointly owned headed Chromium and Node (Linux)','lu-sec06c-genuine-browser'],
 ['SEC-06F original Windows/Linux/source receipt reconciliation only','lu-sec06f-original-reconciliation']
];
export const FULL_NATIVE=[
 ['P0-GATE original native regression and custody (windows-2025)','lu-p0-native-windows-2025'],
 ['P0-GATE original native regression and custody (ubuntu-24.04)','lu-p0-native-ubuntu-24.04']
];
export const INSTALLED_JOBS=[
 ['hosted-windows-nsis','lu-plan07d-windows-nsis-original'],
 ['hosted-debian-installed-human','lu-plan07d-debian-installed-human'],
 ['hosted-genuine-gguf-original','lu-plan07d-genuine-gguf-original-qwen']
];
function check(group,data,required,sha,path){
 const problems=[],rows=[],run=data?.run,runId=data?.runId;
 const runValid=Number.isSafeInteger(runId)&&runId>0&&run?.id===runId&&
   run.head_sha===sha&&run.path===path&&run.head_branch===BRANCH&&
   run.event==='pull_request';
 if(!runValid)problems.push(group+':WRONG_SOURCE_RUN_OR_WORKFLOW');
 if(run?.status!=='completed'||run?.conclusion!=='success')
   problems.push(group+':RUN_NOT_COMPLETED_SUCCESS');
 const jobs=Array.isArray(data?.jobs)?data.jobs:[];
 const artifacts=Array.isArray(data?.artifacts)?data.artifacts:[];
 const unique=new Set();
 for(const [name,artifactPrefix] of required){
   const found=jobs.filter(x=>x?.name===name);
   if(found.length!==1){
     problems.push(group+':'+(found.length?'DUPLICATE_JOB:':'MISSING_JOB:')+name);
     continue;
   }
   const job=found[0];
   if(job.status!=='completed'||job.conclusion!=='success'||job.run_id!==runId||
      !Number.isSafeInteger(job.id)||job.id<=0||unique.has(job.id))
     problems.push(group+':JOB_FAILED_SKIPPED_OR_WRONG_RUN:'+name);
   unique.add(job.id);
   let artifactId=null;
   if(artifactPrefix){
     const expected=artifactPrefix+'-'+sha;
     const matches=artifacts.filter(x=>x?.name===expected);
     if(matches.length!==1)
       problems.push(group+':'+(matches.length?'DUPLICATE_ARTIFACT:':'MISSING_ARTIFACT:')+expected);
     else {
       const a=matches[0];artifactId=a.id;
       if(!Number.isSafeInteger(a.id)||a.id<=0||!Number.isSafeInteger(a.size_in_bytes)||
          a.size_in_bytes<=0||a.expired!==false||!DIGEST.test(a.digest||'')||
          a.workflow_run?.id!==runId||a.workflow_run?.head_sha!==sha||
          a.workflow_run?.head_branch!==BRANCH)
         problems.push(group+':ARTIFACT_ORIGIN_DIGEST_OR_EXPIRY_INVALID:'+expected);
     }
   }
   rows.push({name,jobId:job.id,conclusion:job.conclusion,artifactId});
 }
 return {runId,sourceSha:run?.head_sha??null,problems,observed:rows};
}
export function reconcile({sha,native,fullNative,installed}){
 if(!SHA.test(sha||''))throw Error('CORE01F_EXPECTED_SHA_INVALID');
 const n=check('native',native,NATIVE,sha,WORKFLOW);
 const f=check('full-native',fullNative??native,FULL_NATIVE,sha,WORKFLOW);
 const i=check('installed',installed,INSTALLED_JOBS,sha,INSTALLED);
 const blockers=[...n.problems,...f.problems,...i.problems];
 return {
   schema:'lu-core01f-04-original-hosted-metadata-v1',sourceSha:sha,
   status:blockers.length?'ORIGINAL_EVIDENCE_BLOCKED':'METADATA_REVIEWABLE_LOGS_REQUIRED',
   blockers,native:n,fullNative:f,installed:i,
   originalArtifactContentsReopened:false,kernelReceiptsReconciled:false,
   installedHumanInteractionVerified:false,sec06fAccepted:false,
   nativeWindowsLinuxAccepted:false,hostedInstalledAccepted:false,
   mergeAuthorized:false,releaseAuthorized:false,
   note:'Artifact metadata alone never establishes original kernel or installed receipt acceptance.'
 };
}
if(process.argv[1]&&new URL(import.meta.url).pathname===new URL('file://'+process.argv[1]).pathname){
 const args=process.argv.slice(2);
 const arg=(flag)=>{const n=args.indexOf(flag);return n<0?null:args[n+1]};
 const sha=arg('--sha'),nativeId=Number(arg('--native-run-id')),
   fullId=Number(arg('--full-native-run-id')||nativeId),
   installedId=Number(arg('--installed-run-id'));
 const token=process.env.GITHUB_TOKEN;
 if(!SHA.test(sha||'')||!token||![nativeId,fullId,installedId].every(x=>Number.isSafeInteger(x)&&x>0)){
   process.stderr.write('CORE01F_READ_ONLY_AUDIT_UNAVAILABLE: exact --sha, --native-run-id, --installed-run-id and GITHUB_TOKEN required; --full-native-run-id optional.\n');
   process.exitCode=2;
 }else{
   const request=async(path)=>{
     const res=await fetch('https://api.github.com/repos/vjmichael/Lu-AI-GUI'+path,{
       headers:{Accept:'application/vnd.github+json',Authorization:'Bearer '+token,
         'X-GitHub-Api-Version':'2022-11-28'}});
     if(!res.ok)throw Error('GitHub READ API HTTP '+res.status);
     return res.json();
   };
   const load=async(id)=>{
     const getAll=async(kind)=>{
       const output=[];
       for(let page=1;page<=20;page++){
         const body=await request('/actions/runs/'+id+'/'+kind+'?per_page=100&page='+page);
         const batch=body[kind];
         if(!Array.isArray(batch))throw Error('Missing original '+kind);
         output.push(...batch);if(batch.length<100)return output;
       }
       throw Error('Excessive GitHub evidence pagination');
     };
     return {runId:id,run:await request('/actions/runs/'+id),
       jobs:await getAll('jobs'),artifacts:await getAll('artifacts')};
   };
   try{
     const ids=[...new Set([nativeId,fullId,installedId])];
     const values=await Promise.all(ids.map(load));
     const byId=new Map(values.map(x=>[x.runId,x]));
     const result=reconcile({sha,native:byId.get(nativeId),
       fullNative:byId.get(fullId),installed:byId.get(installedId)});
     process.stdout.write(JSON.stringify(result,null,2)+'\n');
     if(result.blockers.length)process.exitCode=1;
   }catch(e){
     process.stderr.write(JSON.stringify({status:'GITHUB_EVIDENCE_UNAVAILABLE',
       reason:String(e.message||e),mergeAuthorized:false,releaseAuthorized:false})+'\n');
     process.exitCode=2;
   }
 }
}
