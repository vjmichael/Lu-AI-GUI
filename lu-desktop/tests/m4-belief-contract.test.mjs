import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const source=read('../../src-tauri/lu-core/src/ansf_belief_revision.rs');
const host=read('../src-tauri/src/lib.rs');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const jsx=read('../web/main.jsx');
test('M4 only accepts source-backed human authored statements, not model grants',()=>{
  assert.match(source,/pub fn record_reported_claim\(/);
  assert.match(source,/pub fn record_verified_claim\(/);
  assert.match(source,/independent_verify/);
  assert.match(source,/pub fn retract_reported_or_verified_claim\(/);
  assert.match(source,/ClaimState::Contradicted/);
  assert.match(host,/fn lu_m4_note_reported_claim\(/);
  assert.match(host,/fn lu_m4_read_beliefs\(/);
  assert.match(host,/fn lu_m4_retract_claim\(/);
  assert.match(chat,/original\.role!="user"/);
  assert.match(chat,/state/); // interpreted only as local read data
});
test('Lu local GGUF Send receives bounded, explicit, inert belief evidence',()=>{
  assert.match(chat,/pub fn m4_explicit_belief_target/);
  assert.match(chat,/pub fn m4_belief_context/);
  assert.match(host,/chat_store::m4_explicit_belief_target\(&prompt\)/);
  assert.match(host,/if origin==GenerationOrigin::Human&&images\.is_empty\(\)/);
  assert.match(host,/let serialized=evidence\.to_string\(\)/);
  assert.match(host,/serialized\.len\(\)<=4096/);
  assert.match(host,/model_prompt\.push_str\(&envelope\)/);
  assert.match(host,/NO TOOL GRANTS/);
  assert.match(jsx,/M4 semantic claims · source and conflict review/);
  assert.match(jsx,/Record as reported, not verified/);
  assert.match(jsx,/lu_m4_retract_claim/);
  assert.match(jsx,/lu_m4_read_beliefs/);
  assert.match(jsx,/Recall belief project/);
});
