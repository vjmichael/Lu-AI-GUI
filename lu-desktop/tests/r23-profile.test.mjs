import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {DEFAULT_PROFILE, normalizeProfile, profileField} from '../web/inference_profile.mjs';
test('legacy installed GGUF behavior is preserved until explicit host save',()=>{
 const p=normalizeProfile(DEFAULT_PROFILE);
 assert.equal(p.contextTokens,4096);assert.equal(p.maxNewTokens,512);
 assert.equal(p.temperature,0.0);assert.equal(p.threads,4);
});
test('per-model context cannot exceed allowed range',()=>{
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:524288}));
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:4001}));
});
test('model output tokens must fit within context',()=>{
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,contextTokens:8192,maxNewTokens:8192}));
});
test('sampling rejects nonfinite and invalid values',()=>{
 for(const v of [NaN,Infinity,-1,3])assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,temperature:v}));
});
test('field editing retains other controls',()=>{
 const result=profileField(DEFAULT_PROFILE,'contextTokens','16384');
 assert.equal(normalizeProfile(result).contextTokens,16384);
 assert.equal(result.topP,DEFAULT_PROFILE.topP);
});

test('saved R23 larger settings remain bounded within current Lu native ceiling',()=>{
 const p=normalizeProfile({...DEFAULT_PROFILE,contextTokens:262144,
   maxNewTokens:4096,temperature:0.6,threads:8});
 assert.equal(p.contextTokens,262144);
 assert.equal(p.maxNewTokens,4096);
 for(const budget of [511,4097,8192])
   assert.throws(()=>normalizeProfile({...p,maxNewTokens:budget}));
});
test('profile cannot be set with malformed sampling or output limits',()=>{
 for(const value of [0,-0.1,NaN,Infinity])
   assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,topP:value}));
 assert.throws(()=>normalizeProfile({...DEFAULT_PROFILE,threads:0}));
});


test('source-witness: model-specific settings reach protected llama.cpp runtime',()=>{
 const code=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
 const engine=readFileSync(new URL('../../src-tauri/lu-core/src/persistent_llama_backend.rs',import.meta.url),'utf8');
 const sup=readFileSync(new URL('../../src-tauri/lu-core/src/inference_supervisor.rs',import.meta.url),'utf8');
 const settings=readFileSync(new URL('../web/native_windows.jsx',import.meta.url),'utf8');
 for(const name of ['lu_model_inference_profile','lu_model_save_inference_profile'])
   assert.match(code,new RegExp('fn '+name+'\\('));
 assert.match(code,/new_with_model_profile/);
 assert.match(code,/threads: NonZeroUsize::new\(profile\.threads as usize\)/);
 assert.match(code,/context_tokens: profile\.context_tokens/);
 assert.match(code,/with_sampling_profile/);
 assert.match(code,/Ok\(\(backend,profile\.context_tokens,4096\)\)/);
 assert.match(sup,/max_output_tokens/);
 assert.equal((engine.match(/self\.sampling\.apply\(&mut json\)/g)||[]).length,3);
 assert.match(settings,/Save model inference settings/);
 assert.match(settings,/normalizeProfile\(profileSettings\)/);
});
