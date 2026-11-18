#!/usr/bin/env node
// CORE-01F strict read-only review of the R00 -> P0 bridge -> CORE-01E
// original Git blobs. Does not fetch, modify refs, run workers or authorize merge.
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {excludedCorePath} from './core01f-merge-preflight.mjs';

const here=fileURLToPath(import.meta.url);
const root=resolve(dirname(here),'..');
const manifestFile=resolve(root,'docs/LU_CORE01F_P0_BRIDGE_BLOB_REVIEW_20261112.json');
const hex=/^[a-f0-9]{40}$/;
const VALID_DOMAINS=new Set([
 'build_provisioning_acceptance','regression_or_model_witness',
 'design_or_acceptance_ledger','core_desktop_integration',
 'ansf_provenance_and_memory','llama_multimodal_and_model_lifecycle',
 'authorization_custody_and_effects','project_plans_and_human_review',
 'desktop_presentation'
]);
export function verifyBridgeManifest(m){
 const refuse=reason=>{throw Error('CORE01F_BRIDGE_REFUSED: '+reason)};
 if(!m||m.schema!=='lu-core01f-bridge-review-v1'||
    !hex.test(m.r00Head||'')||!hex.test(m.bridgeHead||'')||
    !hex.test(m.core01eReviewedHead||''))refuse('schema or frozen SHA invalid');
 if(m.bridgeDistinctAncestryCommits!==257||m.bridgeFirstParentCommits!==253||
    m.nativeAndInstalledAccepted!==false||m.mergeAuthorized!==false||
    m.releaseAuthorized!==false)refuse('false acceptance or ancestry count');
 if(!Array.isArray(m.files)||m.files.length!==125||m.changedPaths!==125)
   refuse('expected complete 125-path delta');
 const seen=new Set(),counts={},status={EXACT:0,LATER_CHANGED:0};
 let prior='';
 for(const f of m.files){
   if(typeof f.path!=='string'||!f.path||seen.has(f.path)||
      (prior!==''&&prior>=f.path)||excludedCorePath(f.path))
      refuse('out-of-scope, duplicated or unsorted path');
   seen.add(f.path);prior=f.path;
   if(!['added','modified'].includes(f.change)||!hex.test(f.bridgeBlob||'')||
      !hex.test(f.core01eBlob||'')||
      (f.change==='added'&&f.baseBlob!==null)||
      (f.change==='modified'&&!hex.test(f.baseBlob||''))||
      f.baseBlob===f.bridgeBlob||!VALID_DOMAINS.has(f.domain))
       refuse('unexpected source provenance');
   const actual=f.bridgeBlob===f.core01eBlob?'EXACT':'LATER_CHANGED';
   if(f.bridgeIdentityAtCore01e!==actual)refuse('false downstream preservation');
   status[actual]++;counts[f.domain]=(counts[f.domain]||0)+1;
 }
 if(m.deletedPaths!==0||m.preservedExactBlobAtCore01e!==105||
    m.modifiedDownstreamAtCore01e!==20||
    status.EXACT!==105||status.LATER_CHANGED!==20||
    JSON.stringify(Object.entries(counts).sort())!==
      JSON.stringify(Object.entries(m.domains||{}).sort()))
      refuse('missing, mismatched or reclassified bridge paths');
 return {tracked:seen.size,exact:status.EXACT,changed:status.LATER_CHANGED,
    deletions:0,mergeAuthorized:false,releaseAuthorized:false};
}
export function checkGitSnapshot(m,git){
 const tree=sha=>{
   const raw=git(['ls-tree','-r','-z',sha]);
   const map=new Map();
   for(const item of raw.split('\0').filter(Boolean)){
     const match=/^(?:\d{6}) blob ([a-f0-9]{40})\t(.+)$/.exec(item);
     if(!match)throw Error('Unexpected Git source tree record');
     map.set(match[2],match[1]);
   }
   return map;
 };
 const maps=[m.r00Head,m.bridgeHead,m.core01eReviewedHead].map(tree);
 const raw=git(['diff','--no-renames','--name-status','-z',m.r00Head,m.bridgeHead]);
 const names=raw.split('\0').filter(Boolean);const actual=new Map();
 if(names.length%2)throw Error('Git diff status record incomplete');
 for(let i=0;i<names.length;i+=2){
   if(actual.has(names[i+1]))throw Error('Duplicate Git source delta');
   actual.set(names[i+1],names[i]);
 }
 if(actual.size!==m.files.length)throw Error('Unaccounted Git source delta');
 for(const f of m.files){
   const exp=f.change==='added'?'A':'M';
   if(actual.get(f.path)!==exp)throw Error('Git change class mismatch '+f.path);
   for(const [i,field] of ['baseBlob','bridgeBlob','core01eBlob'].entries()){
     if((maps[i].get(f.path)||null)!==f[field])
       throw Error('Original Git blob no longer matches '+f.path+' '+field);
   }
 }
 for(const [left,right] of [[m.r00Head,m.bridgeHead],[m.bridgeHead,m.core01eReviewedHead]]){
   if(git(['merge-base','--is-ancestor',left,right])!=='')
     throw Error('Expected ancestral source relation');
 }
 return {r00:m.r00Head,bridge:m.bridgeHead,core01e:m.core01eReviewedHead,
   sourceDelta:actual.size,originalGitBlobsVerified:true,
   nativeAndInstalledAccepted:false,mergeAuthorized:false,releaseAuthorized:false};
}
export function runReadOnlyGit(args,{cwd=root}={}){
 const res=spawnSync('git',args,{cwd,encoding:'utf8',timeout:120000,
   maxBuffer:8*1024*1024,env:{...process.env,GIT_OPTIONAL_LOCKS:'0'}});
 if(res.error||res.status!==0)throw Error('Original Git read failed: '+args[0]+
   ' '+String(res.stderr||res.error||'').slice(0,220));
 return res.stdout||'';
}
if(process.argv[1]&&resolve(process.argv[1])===here){
 try{
   const m=JSON.parse(readFileSync(manifestFile,'utf8'));
   const checked=verifyBridgeManifest(m);
   const original=checkGitSnapshot(m,runReadOnlyGit);
   process.stdout.write(JSON.stringify({schema:'lu-core01f-bridge-source-audit-v1',
     status:'SOURCE_BRIDGE_PASS_ONLY',...checked,...original})+'\n');
 }catch(e){
   process.stderr.write(JSON.stringify({schema:'lu-core01f-bridge-source-audit-v1',
     status:'BRIDGE_SOURCE_REFUSED_OR_UNAVAILABLE',
     error:String(e.message||e),mergeAuthorized:false,releaseAuthorized:false})+'\n');
   process.exitCode=1;
 }
}
