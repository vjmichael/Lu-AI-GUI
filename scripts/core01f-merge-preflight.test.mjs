import test from 'node:test';
import assert from 'node:assert/strict';
import {CHAIN,excludedCorePath,decision} from './core01f-merge-preflight.mjs';
const ready={chainValid:true,scopeViolations:[],mainOnlyPaths:['docs/LU_DEVELOPMENT_LEDGER.md'],
  lockVerified:true,pinnedCoreRef:true};
test('explicit bridge is present in frozen ancestry chain',()=>{
 assert.deepEqual(CHAIN.map(x=>x.id),[
  'R00_PR200','P0_RECONCILE_BRIDGE','P0_GATE_PR223',
  'P0_WIN_CHAT_PR224','CORE01E_PR226']);
});
test('Core paths are admitted, IDE and P7L/KoboldCpp are not',()=>{
 for(const p of ['src-tauri/lu-ide/src/lib.rs','lu-desktop/src/ide/editor.jsx',
   '.github/workflows/ide-native.yml','src-tauri/lu-core/src/p7l-evaluation.rs',
   'src-tauri/lu-core/src/koboldcpp_adapter.rs'])assert.ok(excludedCorePath(p),p);
 for(const p of ['src-tauri/lu-core/src/inference_host.rs',
   'lu-desktop/src-tauri/src/lib.rs'])assert.ok(!excludedCorePath(p),p);
});
test('perfect source topology still has no merge or release authority',()=>{
 const x=decision(ready);
 assert.equal(x.status,'SOURCE_TOPOLOGY_PASS_ONLY');
 assert.equal(x.mergeAuthorized,false);
 assert.equal(x.nativeWindowsLinuxAccepted,false);
 assert.equal(x.independentOsCustodyAccepted,false);
 assert.equal(x.hostedInstalledAccepted,false);
});
test('unknown main changes, head drift and excluded source files block independently',()=>{
 const x=decision({...ready,pinnedCoreRef:false,
   scopeViolations:[{path:'src-tauri/lu-ide/src/lib.rs'}],
   mainOnlyPaths:['docs/LU_DEVELOPMENT_LEDGER.md','src-tauri/lu-core/src/lib.rs']});
 assert.deepEqual(x.blockers,['CORE01E_HEAD_DRIFT','EXCLUDED_SOURCE_PATH','UNREVIEWED_MAIN_DELTA']);
});
test('missing source lock and nonancestral branch refuse source preflight',()=>{
 const x=decision({...ready,lockVerified:false,chainValid:false});
 assert.deepEqual(x.blockers,['NON_ANCESTRAL_STACK','SOURCE_LOCK_NOT_VERIFIED']);
});
