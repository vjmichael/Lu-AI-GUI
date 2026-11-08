import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/ansf_embedding_worker.rs');
const sem=read('../../src-tauri/lu-core/src/ansf_semantic.rs');
const host=read('../src-tauri/src/lib.rs');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const pane=read('../web/main.jsx').split('function AnsfHistoryPane(')[1]?.split('function App() {')[0]||'';
test('only locally supervised and SHA-pinned GGUF can produce embeddings',()=>{
  assert.match(core,/pub struct LocalEmbeddingWorker/);
  assert.match(core,/fn file_digest\(/);
  assert.match(core,/if hash.finalize\(\).as_slice\(\)!=expected/);
  assert.match(core,/Ipv4Addr::LOCALHOST/);
  assert.match(core,/--embedding/);
  assert.match(core,/--pooling/);
  assert.match(core,/--api-key/);
  assert.match(core,/pub fn embed\(/);
  assert.match(core,/impl Drop for LocalEmbeddingWorker/);
  assert.doesNotMatch(core,/reqwest|https:\/\/api\.openai\.com|cloud.*embeddings/);
});
test('semantic vectors are derived, restartable and original-source-only',()=>{
  assert.match(sem,/pub fn semantic_pending_entries\(/);
  assert.match(sem,/NOT EXISTS\(/);
  assert.match(sem,/s.chunk_ordinal=0/);
  assert.match(sem,/pub fn semantic_indexed_count\(/);
  assert.match(chat,/pub fn index_local_semantic_batch\(/);
  assert.match(chat,/worker.embed\(&passage\)/);
  assert.match(chat,/book.index_semantic_passage\(/);
  assert.match(chat,/self.require\(source_chat\)/);
});
test('user explicitly starts a local worker, indexes originals, and hybrid recalls without model authority',()=>{
  for(const cmd of ['lu_m3_embedding_enable','lu_m3_embedding_disable',
      'lu_m3_embedding_status','lu_m3_semantic_index','lu_m3_semantic_recall']){
    assert.ok(host.includes('fn '+cmd+'('),cmd);
    assert.ok(host.includes(cmd+','),cmd);
  }
  assert.match(host,/LocalEmbeddingWorker::start\(config\)/);
  assert.match(host,/check_file/);
  assert.match(host,/async fn lu_m3_embedding_enable/);
  assert.match(host,/model_prompt\.push_str\(&recalled\)/);
  assert.match(host,/UNTRUSTED DATA ONLY/);
  assert.match(pane,/lu_m3_embedding_enable/);
  assert.match(pane,/lu_m3_semantic_index/);
  assert.match(pane,/lu_m3_semantic_recall/);
  assert.match(pane,/lu_m3_embedding_disable/);
  assert.doesNotMatch(pane,/\b(setChatId|setHistory|setPrompt)\s*\(/);
});
