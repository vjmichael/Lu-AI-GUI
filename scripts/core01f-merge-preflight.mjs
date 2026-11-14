#!/usr/bin/env node
// CORE-01F read-only candidate source/integration preflight.
// The historical reviewed chain is an IMMUTABLE ancestry anchor, NOT a
// permanent live-branch-equals-old-SHA requirement. This tool never fetches,
// merges, rebases, switches branches, changes refs or certifies native receipts.
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';

export const CHAIN=Object.freeze([
 {id:'R00_PR200',sha:'e61d4bf998bccf51074b01c864df208c233eb2f8'},
 {id:'P0_RECONCILE_BRIDGE',sha:'cc312e281b5989a0a9fb972eb04b9e34215a77da'},
 {id:'P0_GATE_PR223',sha:'e422d18cef10fdb1092578631b5fec692b3d7a4f'},
 {id:'P0_WIN_CHAT_PR224',sha:'9b14dde7a01ad36208642be7536159ccb1f42e93'},
 {id:'CORE01E_PR226_REVIEWED_ANCHOR',sha:'97dd1e897c4cf73a1fd8d8bf36f1233b081322c0'}
]);
const HEX=/^[a-f0-9]{40}$/;
const LEDGER='docs/LU_DEVELOPMENT_LEDGER.md';

export function excludedCorePath(path){
 if(typeof path!=='string'||!path||path.startsWith('/')||
    path.split('/').includes('..')||path.includes('\\'))return true;
 return /^(?:ide\/|src-tauri\/lu-ide\/|lu-desktop\/src\/ide\/|lu-desktop\/src-tauri\/src\/ide_|docs\/(?:IDE[-_]|LU_IDE)|\.github\/workflows\/[^/]*ide[^/]*\.yml)/i.test(path)
   || /(?:^|\/)(?:[^/]*koboldcpp[^/]*|p7l[-_][^/]*|[^/]*p7l1[^/]*)/i.test(path);
}

// Compare ALL immutable Git tree blobs with the independently supplied
// candidate's source-tree.lock.json, not the current CORE-01F checkout lock.
// The published lock commit must be the immediate child of its code commit.
export function verifyCandidateSourceLock(manifest,entries,codeParent){
 const bad=(reason)=>({valid:false,reason,trackedFileCount:0});
 if(!manifest||manifest.schema!==1||!Array.isArray(manifest.files)||
    !HEX.test(manifest.sourceCommitBeforeLock||'')||
    manifest.sourceCommitBeforeLock!==codeParent)
   return bad('UNTRUSTED_LOCK_OR_CODE_PARENT');
 if(!Array.isArray(entries)||!entries.length||!Number.isSafeInteger(manifest.trackedFileCount)||
    manifest.trackedFileCount!==manifest.files.length)
   return bad('MISSING_OR_INCOMPLETE_TREE');
 const lock=entries.filter(e=>e.path==='source-tree.lock.json');
 if(lock.length!==1||lock[0].type!=='blob')return bad('MISSING_SOURCE_LOCK_BLOB');
 const actual=entries.filter(e=>e.path!=='source-tree.lock.json');
 if(actual.length!==manifest.files.length)return bad('SOURCE_TREE_FILE_COUNT_CHANGED');
 let prior='';
 for(let i=0;i<actual.length;i++){
   const node=actual[i],expected=manifest.files[i];
   if(!node||node.type!=='blob'||!HEX.test(node.blob||'')||
      typeof node.path!=='string'||!node.path||
      node.path.startsWith('/')||node.path.includes('..')||
      node.path.includes('\\')||node.path.includes('\n')||
      (prior!==''&&prior>=node.path))
     return bad('UNSAFE_OR_UNSORTED_TREE');
   if(!expected||expected.path!==node.path||expected.blob!==node.blob)
     return bad('SOURCE_LOCK_BLOB_MISMATCH');
   prior=node.path;
 }
 return {valid:true,reason:null,trackedFileCount:actual.length};
}

export function integrationPlan({coreSha,currentSha,coreDelta=[],bridgeDelta=[],
    mainOnlyPaths=[],currentContainsCandidate=false}){
 const changedByBridge=new Set(bridgeDelta);
 const overlapPaths=[...new Set(coreDelta.filter(path=>changedByBridge.has(path)))].sort();
 return {
   candidateCoreSha:coreSha,core01fReviewSha:currentSha,
   currentContainsCandidate,restackRequired:!currentContainsCandidate,
   overlapPaths,overlapRequiresHumanReview:overlapPaths.length>0,
   sourceLockMustBeRegeneratedAfterRestack:true,
   preserveLiveMainLedger:mainOnlyPaths.includes(LEDGER),
   recommendedNextAction:'REVIEW_DIFFERENCES_ONLY_NO_MUTATION',
   noAutomaticMergeOrRebase:true,
   installedOrMergeAuthority:false
 };
}

export function decision({chainValid,candidateDescendsFromReviewed,
 currentDescendsFromReviewed,candidateSourceLockVerified,
 currentSourceLockVerified,scopeViolations=[],mainOnlyPaths=[],
 expectedCandidateMatches=true}){
 const blockers=[];
 if(!chainValid||!currentDescendsFromReviewed)blockers.push('NON_ANCESTRAL_STACK');
 if(!candidateDescendsFromReviewed)blockers.push('CORE01E_NOT_DESCENDANT_OF_REVIEWED');
 if(!expectedCandidateMatches)blockers.push('CANDIDATE_HEAD_DRIFT');
 if(!candidateSourceLockVerified)blockers.push('CANDIDATE_SOURCE_LOCK_NOT_VERIFIED');
 if(!currentSourceLockVerified)blockers.push('CURRENT_SOURCE_LOCK_NOT_VERIFIED');
 if(scopeViolations.length)blockers.push('EXCLUDED_SOURCE_PATH');
 if(mainOnlyPaths.some(p=>p!==LEDGER))blockers.push('UNREVIEWED_MAIN_DELTA');
 return {
   schema:'lu-core01f-readonly-candidate-preflight-v2',
   status:blockers.length?'SOURCE_REVIEW_BLOCKED':'SOURCE_INTEGRATION_REVIEWABLE_ONLY',
   blockers,
   nativeWindowsLinuxAccepted:false,
   independentOsCustodyAccepted:false,
   hostedInstalledAccepted:false,
   physicalRadeonAccepted:false,
   browserPackagedAccepted:false,
   core01eProductionAccepted:false,
   sec06fAccepted:false,
   mergeAuthorized:false,releaseAuthorized:false
 };
}

const here=fileURLToPath(import.meta.url);
if(process.argv[1]&&resolve(process.argv[1])===here){
 const root=resolve(dirname(here),'..');
 const argv=process.argv.slice(2);
 const arg=(flag,fallback)=>{
   const i=argv.indexOf(flag);
   return i<0?fallback:argv[i+1];
 };
 if(argv.includes('--help')){
   process.stdout.write('Read-only CORE01F: [--main-ref origin/main] [--core-ref origin/lu/core01-source-convergence-no-ide-no-kobold-20261112] [--expect-core-sha <40-char-SHA>]\n');
   process.exit(0);
 }
 const mainRef=arg('--main-ref','origin/main');
 const coreRef=arg('--core-ref','origin/lu/core01-source-convergence-no-ide-no-kobold-20261112');
 const expectedCoreSha=arg('--expect-core-sha',null);
 const cmd=(exe,args)=>spawnSync(exe,args,{cwd:root,encoding:'utf8',
   timeout:120000,maxBuffer:16*1024*1024,
   env:{...process.env,GIT_OPTIONAL_LOCKS:'0'}});
 const git=(...args)=>{
   const r=cmd('git',args);
   if(r.error||r.status!==0)
     throw Error('Git inspection unavailable: '+args[0]+': '+String(r.stderr||r.error||'').slice(0,200));
   return (r.stdout||'').trim();
 };
 const diff=(a,b)=>git('diff','--no-renames','--name-only',a+'..'+b)
   .split(/\r?\n/).filter(Boolean);
 const ancestor=(a,b)=>cmd('git',['merge-base','--is-ancestor',a,b]).status===0;
 try{
   for(const name of [mainRef,coreRef]){
     if(typeof name!=='string'||!name||
       !/^[A-Za-z0-9._\/-]+$/.test(name)||name.startsWith('-'))
       throw Error('Invalid immutable inspection ref');
   }
   if(expectedCoreSha!==null&&!HEX.test(expectedCoreSha))
     throw Error('Expected CORE-01E candidate must be exact 40-char SHA');
   const mainSha=git('rev-parse','--verify',mainRef+'^{commit}');
   const coreSha=git('rev-parse','--verify',coreRef+'^{commit}');
   const currentSha=git('rev-parse','HEAD');
   const reviewed=CHAIN.at(-1).sha;
   const ancestry=[];
   for(let i=1;i<CHAIN.length;i++){
     const parent=CHAIN[i-1],child=CHAIN[i];
     ancestry.push({parent:parent.id,child:child.id,
       ancestor:ancestor(parent.sha,child.sha)});
   }
   const currentDescendsFromReviewed=ancestor(reviewed,currentSha);
   const candidateDescendsFromReviewed=ancestor(reviewed,coreSha);
   const common=git('merge-base',mainSha,coreSha);
   const mainOnlyPaths=diff(common,mainSha);
   const scope=[];
   for(let i=1;i<CHAIN.length;i++){
     const parent=CHAIN[i-1],child=CHAIN[i];
     const paths=diff(parent.sha,child.sha);
     scope.push({from:parent.id,to:child.id,changed:paths.length,
       forbiddenPaths:paths.filter(excludedCorePath)});
   }
   // Unlike the old preflight, review BOTH NEW source deltas after the
   // frozen, historical CORE01E anchor. Neither needs to have been merged.
   const coreDelta=diff(reviewed,coreSha);
   const bridgeDelta=diff(reviewed,currentSha);
   scope.push({from:CHAIN.at(-1).id,to:'CURRENT_CORE01E_CANDIDATE',
     changed:coreDelta.length,forbiddenPaths:coreDelta.filter(excludedCorePath)});
   scope.push({from:CHAIN.at(-1).id,to:'CORE01F_UNRESTACKED_DELTA',
     changed:bridgeDelta.length,forbiddenPaths:bridgeDelta.filter(excludedCorePath)});
   const local=cmd(process.execPath,['scripts/verify-source-tree.mjs']);
   const currentSourceLockVerified=local.status===0&&
     /REPO01_SOURCE_PASS tracked=\d+/.test(local.stdout||'');
   // Never mistake verification of CURRENT checkout's older lock for
   // verification of the moving CORE-01E candidate's exact Git blob set.
   let candidateLock;
   try{
     const raw=git('show',coreSha+':source-tree.lock.json');
     const manifest=JSON.parse(raw);
     const tree=cmd('git',['ls-tree','-r','-z','--full-tree',coreSha]);
     if(tree.status!==0||tree.error)throw Error('Cannot inspect candidate tree');
     const entries=tree.stdout.split('\0').filter(Boolean).map(line=>{
       const m=/^([0-7]{6}) (blob|tree|commit) ([a-f0-9]{40})\t(.+)$/.exec(line);
       if(!m)throw Error('Malformed candidate tree row');
       return {mode:m[1],type:m[2],blob:m[3],path:m[4]};
     });
     const parent=git('rev-list','--parents','-n','1',coreSha).split(' ');
     candidateLock=verifyCandidateSourceLock(manifest,entries,parent.length===2?parent[1]:null);
   }catch(error){
     candidateLock={valid:false,reason:'CANDIDATE_LOCK_UNAVAILABLE: '+String(error.message||error).slice(0,120),trackedFileCount:0};
   }
   const violations=scope.flatMap(x=>x.forbiddenPaths.map(path=>({stage:x.to,path})));
   const verdict=decision({
     chainValid:ancestry.every(x=>x.ancestor),candidateDescendsFromReviewed,
     currentDescendsFromReviewed,candidateSourceLockVerified:candidateLock.valid,
     currentSourceLockVerified,scopeViolations:violations,mainOnlyPaths,
     expectedCandidateMatches:expectedCoreSha===null||expectedCoreSha===coreSha
   });
   const plan=integrationPlan({coreSha,currentSha,coreDelta,bridgeDelta,
     mainOnlyPaths,currentContainsCandidate:ancestor(coreSha,currentSha)});
   process.stdout.write(JSON.stringify({...verdict,
     historicalReviewedCore01eSha:reviewed,currentSha,mainSha,coreSha,
     expectedCoreSha,mainCommonAncestorSha:common,
     candidateDescendsFromReviewed,currentDescendsFromReviewed,
     chain:CHAIN,ancestry,scope,mainOnlyPaths,
     candidateSourceLock:candidateLock,currentSourceLockVerified,
     integration:plan},null,2)+'\n');
   if(verdict.blockers.length)process.exitCode=1;
 }catch(e){
   process.stderr.write(JSON.stringify({status:'PREFLIGHT_UNAVAILABLE',
     error:String(e.message||e),mergeAuthorized:false,releaseAuthorized:false})+'\n');
   process.exitCode=2;
 }
}
