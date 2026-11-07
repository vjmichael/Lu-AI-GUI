import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/ansf_semantic.rs');
const ansf=read('../../src-tauri/lu-core/src/ansf.rs');
const host=read('../src-tauri/src/ansf_chat_store.rs');
const harness=read('./native-chat/src/lib.rs');
test('Derived semantic index lives inside original ANSF book',()=>{
  assert.match(ansf,/initialize_semantic_schema\(&conn\)/);
  assert.match(core,/CREATE TABLE IF NOT EXISTS ansf_semantic_projection/);
  assert.match(core,/source_sha256/);
  assert.match(core,/PRIMARY KEY\(entry_id,chunk_ordinal,embedding_model\)/);
  assert.match(core,/pub fn clear_semantic_model_index/);
  assert.doesNotMatch(core,/CREATE TABLE.*original_conversation_archive/i);
});
test('M3 vector and hybrid index are bounded and original-source-only',()=>{
  assert.match(core,/const MAX_SCAN:usize=8192/);
  assert.match(core,/pub fn semantic_candidates/);
  assert.match(core,/pub fn fuse_source_ranks/);
  assert.match(core,/AnsfSourceId::parse\(id\)/);
  assert.match(core,/self\.read_entry\(session_id,entry_id\)/);
  assert.match(host,/pub fn hybrid_recall_with_embedding/);
  assert.match(host,/self\.known_source_for_turn\(active_chat,&source.source_id\)/);
  assert.match(host,/trusted LOCAL embedding engine/i);
  assert.match(harness,/pub mod ansf_semantic/);
});
test('S1 model version and no fake embedding provider',()=>{
  assert.match(core,/fn rebuild_and_model_versions_do_not_change_original_entries/);
  assert.match(core,/fn malformed_zero_nan_non_user_and_dimension_conflicts_fail_closed/);
  assert.doesNotMatch(host,/invoke\('remote_embedding_api'/);
});
