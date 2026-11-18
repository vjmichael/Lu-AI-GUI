import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';
import {verifyBridgeManifest} from './core01f-bridge-review.mjs';
const file=resolve(dirname(fileURLToPath(import.meta.url)),
 '../docs/LU_CORE01F_P0_BRIDGE_BLOB_REVIEW_20261112.json');
const original=JSON.parse(readFileSync(file,'utf8'));
const copy=()=>structuredClone(original);

test('all 125 bridge changed source paths have original and latest Git identity',()=>{
 const x=verifyBridgeManifest(original);
 assert.deepEqual(x,{tracked:125,exact:105,changed:20,deletions:0,
   mergeAuthorized:false,releaseAuthorized:false});
});
test('revised host authorization and desktop integration paths remain explicitly visible',()=>{
 const later=original.files.filter(f=>f.bridgeIdentityAtCore01e==='LATER_CHANGED')
   .map(f=>f.path);
 assert.equal(later.length,20);
 for(const x of [
  'lu-desktop/src-tauri/src/lib.rs',
  'lu-desktop/src-tauri/src/ansf_chat_store.rs',
  'lu-desktop/src-tauri/src/p0_auth_custody.rs',
  'lu-desktop/src-tauri/src/p0_auth_desktop.rs',
  'src-tauri/lu-core/src/native_background_process.rs',
  '.github/workflows/lu-repo01-full-source.yml'
 ])assert.ok(later.includes(x),x);
});
test('new out-of-scope IDE and KoboldCpp files cannot be passed into reviewed bridge',()=>{
 for(const name of ['src-tauri/lu-ide/src/worker.rs','src-tauri/lu-core/src/koboldcpp.rs',
   'docs/p7l-native.md']){
   const m=copy();m.files[0].path=name;
   assert.throws(()=>verifyBridgeManifest(m),/CORE01F_BRIDGE_REFUSED/);
 }
});
test('missing, duplicate, permuted, altered or untracked bridge records fail closed',()=>{
 let m=copy();m.files.pop();
 assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
 m=copy();m.files[1].path=m.files[0].path;
 assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
 m=copy();[m.files[0],m.files[1]]=[m.files[1],m.files[0]];
 assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
 m=copy();m.files[0].bridgeBlob='0'.repeat(40);
 assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
 m=copy();m.files[0].core01eBlob='0'.repeat(40);
 assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
});
test('cannot forge acceptance or omit source bridge data',()=>{
 for(const [key,val] of [
  ['nativeAndInstalledAccepted',true],['mergeAuthorized',true],
  ['releaseAuthorized',true],['bridgeDistinctAncestryCommits',256],
  ['changedPaths',124],['deletedPaths',1]
 ]){
   const m=copy();m[key]=val;
   assert.throws(()=>verifyBridgeManifest(m),/REFUSED/);
 }
});


const source=path=>readFileSync(resolve(dirname(fileURLToPath(import.meta.url)),'..',path),'utf8');
test('P0 review keeps exact human authorization and protected scoped cancellation',()=>{
 const authority=source('src-tauri/lu-core/src/conversation_authority.rs');
 assert.match(authority,/pub fn with_authorized_worker_launch/);
 assert.match(authority,/pub fn revoke_conversation/);
 const desktop=source('lu-desktop/src-tauri/src/p0_auth_desktop.rs');
 assert.match(desktop,/background_scheduler\.list_owned\(chat,100\)/);
 assert.match(desktop,/background_processes\.cancel_workspace_owned\(chat\)/);
 const journal=source('lu-desktop/src-tauri/src/p0_auth_custody.rs');
 assert.match(journal,/automatic_retry_allowed:false/);
 assert.match(journal,/os_custody_independently_verified:false/);
});
test('P0 bridge never treats remembered sources or task proposals as execution grants',()=>{
 const sourceReceipt=source('lu-desktop/src-tauri/src/ansf_source_evidence.rs');
 assert.match(sourceReceipt,/execution_authorized:false/);
 const continuation=source('lu-desktop/src-tauri/src/p0_memory_continuity.rs');
 assert.match(continuation,/"taskCompleted":false/);
 const register=source('src-tauri/lu-core/src/project_task_register.rs');
 assert.match(register,/execution_authorized:false/);
 const transfer=source('lu-desktop/src-tauri/src/model_transfer_desktop.rs');
 assert.match(transfer,/INTERRUPTED_REVIEW_REQUIRED/);
});
test('packaging review cannot claim missing browser payload',()=>{
 const manifest=JSON.parse(source('lu-desktop/src-tauri/resources/lu-browser/manifest.json'));
 assert.equal(manifest.status,'not-packaged');
 assert.deepEqual(manifest.files,[]);
 assert.equal(manifest.platform,'none');
});
