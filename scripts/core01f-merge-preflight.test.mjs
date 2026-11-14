import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {CHAIN,PROTECTED_MAIN_FILES,excludedCorePath,verifyCandidateSourceLock,reconcileProtectedMain,integrationPlan,decision} from './core01f-merge-preflight.mjs';

const ready={chainValid:true,candidateDescendsFromReviewed:true,
  currentDescendsFromReviewed:true,candidateSourceLockVerified:true,
  currentSourceLockVerified:true,scopeViolations:[],
  mainOnlyPaths:['docs/LU_DEVELOPMENT_LEDGER.md'],expectedCandidateMatches:true,
  expectedMainMatches:true,protectedMainVerified:true};
const hex='a'.repeat(40),parent='b'.repeat(40);
const locked=()=>({
 manifest:{schema:1,sourceCommitBeforeLock:parent,trackedFileCount:2,
   files:[{path:'docs/readme.md',blob:hex},
          {path:'src-tauri/lu-core/src/lib.rs',blob:'c'.repeat(40)}]},
 entries:[{path:'docs/readme.md',type:'blob',blob:hex},
          {path:'source-tree.lock.json',type:'blob',blob:'d'.repeat(40)},
          {path:'src-tauri/lu-core/src/lib.rs',type:'blob',blob:'c'.repeat(40)}]
});

test('original reviewed chain remains immutable ancestry anchor',()=>{
 assert.deepEqual(CHAIN.map(x=>x.id),[
  'R00_PR200','P0_RECONCILE_BRIDGE','P0_GATE_PR223',
  'P0_WIN_CHAT_PR224','CORE01E_PR226_REVIEWED_ANCHOR']);
 assert.equal(CHAIN.at(-1).sha,'97dd1e897c4cf73a1fd8d8bf36f1233b081322c0');
});
test('Core source only; IDE, symlink-like and P7L/KoboldCpp paths remain excluded',()=>{
 for(const p of ['src-tauri/lu-ide/src/lib.rs','lu-desktop/src/ide/editor.jsx',
   '.github/workflows/ide-native.yml','src-tauri/lu-core/src/p7l-evaluation.rs',
   'src-tauri/lu-core/src/koboldcpp_adapter.rs','../escape','/absolute/file'])
   assert.ok(excludedCorePath(p),p);
 for(const p of ['src-tauri/lu-core/src/inference_host.rs',
   'lu-desktop/src-tauri/src/lib.rs'])assert.ok(!excludedCorePath(p),p);
});
test('newer legitimate CORE01E candidate becomes REVIEWABLE, never accepted for merge',()=>{
 const x=decision(ready);
 assert.equal(x.status,'SOURCE_INTEGRATION_REVIEWABLE_ONLY');
 assert.deepEqual(x.blockers,[]);
 for(const k of ['mergeAuthorized','releaseAuthorized','core01eProductionAccepted',
   'sec06fAccepted','hostedInstalledAccepted','independentOsCustodyAccepted',
   'nativeWindowsLinuxAccepted','browserPackagedAccepted'])
   assert.equal(x[k],false,k);
});
test('candidate branch must DESCEND from reviewed anchor and honor explicit exact SHA',()=>{
 const x=decision({...ready,candidateDescendsFromReviewed:false,
   expectedCandidateMatches:false});
 assert.deepEqual(x.blockers,['CORE01E_NOT_DESCENDANT_OF_REVIEWED','CANDIDATE_HEAD_DRIFT']);
});
test('nonancestral CORE01F branch, altered main, and excluded new scope stay blockers',()=>{
 const x=decision({...ready,currentDescendsFromReviewed:false,
   scopeViolations:[{stage:'CURRENT_CORE01E_CANDIDATE',path:'src-tauri/lu-ide/src/lib.rs'}],
   mainOnlyPaths:['docs/LU_DEVELOPMENT_LEDGER.md','src-tauri/lu-core/src/lib.rs']});
 assert.deepEqual(x.blockers,['NON_ANCESTRAL_STACK','EXCLUDED_SOURCE_PATH','UNREVIEWED_MAIN_DELTA']);
});
test('missing OR forged candidate/current source locks block independently',()=>{
 const x=decision({...ready,candidateSourceLockVerified:false,
   currentSourceLockVerified:false});
 assert.deepEqual(x.blockers,['CANDIDATE_SOURCE_LOCK_NOT_VERIFIED',
   'CURRENT_SOURCE_LOCK_NOT_VERIFIED']);
});
test('candidate must contain genuine matching independently frozen Git blobs',()=>{
 const {manifest,entries}=locked();
 assert.deepEqual(verifyCandidateSourceLock(manifest,entries,parent),
   {valid:true,reason:null,trackedFileCount:2});
 assert.equal(verifyCandidateSourceLock(manifest,entries,hex).reason,
   'UNTRUSTED_LOCK_OR_CODE_PARENT');
 const forged=structuredClone(manifest);forged.files[1].blob='e'.repeat(40);
 assert.equal(verifyCandidateSourceLock(forged,entries,parent).reason,
   'SOURCE_LOCK_BLOB_MISMATCH');
 const missing=entries.filter(e=>e.path!=='docs/readme.md');
 assert.equal(verifyCandidateSourceLock(manifest,missing,parent).reason,
   'SOURCE_TREE_FILE_COUNT_CHANGED');
});
test('candidate source lock rejects duplicate, traversal and unsorted Git rows',()=>{
 const {manifest,entries}=locked();
 const duplicate=[entries[0],entries[0],...entries.slice(1)];
 assert.equal(verifyCandidateSourceLock(manifest,duplicate,parent).reason,
   'SOURCE_TREE_FILE_COUNT_CHANGED');
 const escaped=structuredClone(entries);escaped[0].path='../escape';
 assert.equal(verifyCandidateSourceLock(manifest,escaped,parent).reason,
   'UNSAFE_OR_UNSORTED_TREE');
 const reversed=structuredClone(entries);
 [reversed[0],reversed[2]]=[reversed[2],reversed[0]];
 assert.equal(verifyCandidateSourceLock(manifest,reversed,parent).reason,
   'UNSAFE_OR_UNSORTED_TREE');
});
test('restack collision review never performs merge and preserves main ledger',()=>{
 const x=integrationPlan({coreSha:hex,currentSha:parent,
   coreDelta:['source-tree.lock.json','src-tauri/lu-core/src/lib.rs','docs/another.md'],
   bridgeDelta:['source-tree.lock.json','docs/another.md','scripts/core01f-merge-preflight.mjs'],
   mainOnlyPaths:['docs/LU_DEVELOPMENT_LEDGER.md'],currentContainsCandidate:false});
 assert.deepEqual(x.overlapPaths,['docs/another.md','source-tree.lock.json']);
 assert.equal(x.restackRequired,true);
 assert.equal(x.preserveLiveMainLedger,true);
 assert.equal(x.noAutomaticMergeOrRebase,true);
 assert.equal(x.installedOrMergeAuthority,false);
});
test('even a restacked candidate provides no merge or release authorization',()=>{
 const x=integrationPlan({coreSha:hex,currentSha:parent,
   currentContainsCandidate:true});
 assert.equal(x.restackRequired,false);
 assert.equal(x.installedOrMergeAuthority,false);
 assert.equal(x.noAutomaticMergeOrRebase,true);
});

test('manual preflight does not grant merge or release, even on current HEAD',()=>{
 const x=decision({...ready,currentDescendsFromReviewed:true});
 assert.equal(x.mergeAuthorized,false);
 assert.equal(x.releaseAuthorized,false);
 assert.equal(x.nativeWindowsLinuxAccepted,false);
});

test('CORE01F-03 main ledger and scheduled workflow protected by immutable main blobs',()=>{
 assert.deepEqual(PROTECTED_MAIN_FILES,[
   'docs/LU_DEVELOPMENT_LEDGER.md',
   '.github/workflows/lu-development-ledger.yml'
 ]);
 const a='a'.repeat(40),b='b'.repeat(40),c='c'.repeat(40);
 const main=Object.fromEntries(PROTECTED_MAIN_FILES.map(p=>[p,a]));
 const core=Object.fromEntries(PROTECTED_MAIN_FILES.map(p=>[p,b]));
 const review=Object.fromEntries(PROTECTED_MAIN_FILES.map(p=>[p,b]));
 const legacy={...main,[PROTECTED_MAIN_FILES[0]]:c};
 const result=reconcileProtectedMain({mainSha:a,mainBlobs:main,
   coreBlobs:core,reviewBlobs:review,legacyBlobs:legacy});
 assert.equal(result.valid,true);
 assert.deepEqual(result.staleLegacyPaths,[PROTECTED_MAIN_FILES[0]]);
 assert.ok(result.files.every(x=>x.requiredIntegrationSource==='EXACT_CURRENT_MAIN_BLOB'));
 assert.ok(result.files.every(x=>x.automaticResolutionAllowed===false));
 assert.equal(result.mainIntegrationAuthorized,false);
 assert.equal(result.noAutomaticWriteOrMerge,true);
});
test('CORE01F-03 refuses missing protected blobs, wrong main identity and drift',()=>{
 const a='a'.repeat(40),blobs=Object.fromEntries(PROTECTED_MAIN_FILES.map(p=>[p,a]));
 assert.equal(reconcileProtectedMain({mainSha:'unsafe',mainBlobs:blobs,
   coreBlobs:blobs,reviewBlobs:blobs}).valid,false);
 const missing={...blobs};delete missing[PROTECTED_MAIN_FILES[0]];
 const denied=reconcileProtectedMain({mainSha:a,mainBlobs:blobs,
   coreBlobs:missing,reviewBlobs:blobs});
 assert.equal(denied.valid,false);
 assert.ok(denied.problems.includes('MISSING_PROTECTED_BLOB:'+PROTECTED_MAIN_FILES[0]));
 const verdict=decision({...ready,expectedMainMatches:false,protectedMainVerified:false});
 assert.deepEqual(verdict.blockers,['MAIN_HEAD_DRIFT','MAIN_PROTECTED_BLOBS_NOT_VERIFIED']);
});
