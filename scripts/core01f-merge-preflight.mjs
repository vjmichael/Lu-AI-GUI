#!/usr/bin/env node
// CORE-01F read-only Git/source topology audit. Never fetch, merge, rebase,
// change a ref, grant tool authority, or attest native/installed acceptance.
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';

export const CHAIN=Object.freeze([
 {id:'R00_PR200',sha:'e61d4bf998bccf51074b01c864df208c233eb2f8'},
 {id:'P0_RECONCILE_BRIDGE',sha:'cc312e281b5989a0a9fb972eb04b9e34215a77da'},
 {id:'P0_GATE_PR223',sha:'e422d18cef10fdb1092578631b5fec692b3d7a4f'},
 {id:'P0_WIN_CHAT_PR224',sha:'9b14dde7a01ad36208642be7536159ccb1f42e93'},
 {id:'CORE01E_PR226',sha:'97dd1e897c4cf73a1fd8d8bf36f1233b081322c0'}
]);

export function excludedCorePath(path){
 if(typeof path!=='string')return true;
 return /^(?:ide\/|src-tauri\/lu-ide\/|lu-desktop\/src\/ide\/|lu-desktop\/src-tauri\/src\/ide_|docs\/(?:IDE[-_]|LU_IDE)|\.github\/workflows\/[^/]*ide[^/]*\.yml)/i.test(path)
   || /(?:^|\/)(?:[^/]*koboldcpp[^/]*|p7l[-_][^/]*|[^/]*p7l1[^/]*)/i.test(path);
}
export function decision({chainValid,scopeViolations,mainOnlyPaths,lockVerified,pinnedCoreRef}){
 const blockers=[];
 if(!chainValid)blockers.push('NON_ANCESTRAL_STACK');
 if(!pinnedCoreRef)blockers.push('CORE01E_HEAD_DRIFT');
 if(!lockVerified)blockers.push('SOURCE_LOCK_NOT_VERIFIED');
 if(scopeViolations.length)blockers.push('EXCLUDED_SOURCE_PATH');
 if(mainOnlyPaths.some(p=>p!=='docs/LU_DEVELOPMENT_LEDGER.md'))
   blockers.push('UNREVIEWED_MAIN_DELTA');
 return {schema:'lu-core01f-readonly-preflight-v1',
   status:blockers.length?'SOURCE_REVIEW_BLOCKED':'SOURCE_TOPOLOGY_PASS_ONLY',
   blockers,
   nativeWindowsLinuxAccepted:false,
   independentOsCustodyAccepted:false,
   hostedInstalledAccepted:false,
   physicalRadeonAccepted:false,
   browserPackagedAccepted:false,
   mergeAuthorized:false,releaseAuthorized:false};
}
const here=fileURLToPath(import.meta.url);
if(process.argv[1]&&resolve(process.argv[1])===here){
 const root=resolve(dirname(here),'..');
 const argv=process.argv.slice(2);
 const arg=(flag,fallback)=>{
   const i=argv.indexOf(flag);return i<0?fallback:argv[i+1];
 };
 if(argv.includes('--help')){
   process.stdout.write('Read-only CORE01F: [--main-ref origin/main] [--core-ref origin/lu/core01-source-convergence-no-ide-no-kobold-20261112]\n');
   process.exit(0);
 }
 const mainRef=arg('--main-ref','origin/main');
 const coreRef=arg('--core-ref','origin/lu/core01-source-convergence-no-ide-no-kobold-20261112');
 const cmd=(exe,args)=>spawnSync(exe,args,{cwd:root,encoding:'utf8',
   timeout:120000,maxBuffer:4*1024*1024,
   env:{...process.env,GIT_OPTIONAL_LOCKS:'0'}});
 const git=(...args)=>{
   const r=cmd('git',args);
   if(r.error||r.status!==0)
     throw Error('Git inspection unavailable: '+args[0]+': '+String(r.stderr||r.error||'').slice(0,320));
   return (r.stdout||'').trim();
 };
 const diff=(a,b)=>git('diff','--no-renames','--name-only',a+'..'+b).split(/\r?\n/).filter(Boolean);
 try{
   for(const name of [mainRef,coreRef]){
     if(!name||name.startsWith('-')||name.includes('\n'))throw Error('Invalid ref name');
   }
   const mainSha=git('rev-parse','--verify',mainRef+'^{commit}');
   const coreSha=git('rev-parse','--verify',coreRef+'^{commit}');
   const currentSha=git('rev-parse','HEAD');
   const ancestry=[];
   for(let i=1;i<CHAIN.length;i++){
     const parent=CHAIN[i-1],child=CHAIN[i];
     ancestry.push({parent:parent.id,child:child.id,
       ancestor:cmd('git',['merge-base','--is-ancestor',parent.sha,child.sha]).status===0});
   }
   const pinned=CHAIN.at(-1).sha;
   const currentDescendsFromPinned=cmd('git',['merge-base','--is-ancestor',pinned,currentSha]).status===0;
   const common=git('merge-base',mainSha,coreSha);
   const mainOnlyPaths=diff(common,mainSha);
   const scope=[];
   for(let i=1;i<CHAIN.length;i++){
     const parent=CHAIN[i-1],child=CHAIN[i];
     const paths=diff(parent.sha,child.sha);
     scope.push({from:parent.id,to:child.id,changed:paths.length,
       forbiddenPaths:paths.filter(excludedCorePath)});
   }
   const check=cmd(process.execPath,['scripts/verify-source-tree.mjs']);
   const lockVerified=check.status===0&&/REPO01_SOURCE_PASS tracked=\d+/.test(check.stdout||'');
   const violations=scope.flatMap(x=>x.forbiddenPaths.map(path=>({stage:x.to,path})));
   const verdict=decision({chainValid:ancestry.every(x=>x.ancestor)&&currentDescendsFromPinned,
     scopeViolations:violations,mainOnlyPaths,lockVerified,pinnedCoreRef:coreSha===pinned});
   process.stdout.write(JSON.stringify({...verdict,currentSha,mainSha,coreSha,
     mainCommonAncestorSha:common,chain:CHAIN,ancestry,scope,mainOnlyPaths,
     currentDescendsFromPinned,lockVerified},null,2)+'\n');
   if(verdict.blockers.length)process.exitCode=1;
 }catch(e){
   process.stderr.write(JSON.stringify({status:'PREFLIGHT_UNAVAILABLE',
     error:String(e.message||e),mergeAuthorized:false,releaseAuthorized:false})+'\n');
   process.exitCode=2;
 }
}
