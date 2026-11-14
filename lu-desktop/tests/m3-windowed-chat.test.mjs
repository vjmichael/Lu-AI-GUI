import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const source=read('../../src-tauri/lu-core/src/ansf_live_chat.rs');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const host=read('../src-tauri/src/lib.rs');
const jsx=read('../web/main.jsx');
test('ANSF stores one pointer-only display row per visible message/generation',()=>{
  assert.match(source,/CREATE TABLE IF NOT EXISTS ansf_visible_chat\(/);
  assert.match(source,/latest_event_seq INTEGER NOT NULL REFERENCES ansf_live_chat_events\(seq\)/);
  assert.match(source,/UNIQUE\(session_id,generation_id\)/);
  assert.match(source,/fn visible_chat_page\(/);
  assert.match(source,/fn durable_visible_cursor_pages_collapse_stream_progress_and_recover_cancelled/);
  assert.match(source,/GROUP BY p.session_id,p.generation_id/);
});
test('Production host exposes bounded bidirectional cursor windows without changing model history',()=>{
  assert.match(chat,/pub fn history_window\(/);
  assert.match(chat,/book.visible_chat_before/);
  assert.match(chat,/book.visible_chat_after/);
  assert.match(host,/fn lu_history_window\(/);
  assert.match(host,/lu_history_window,\s*\n/);
  assert.match(host,/fn lu_history\(/);
  assert.match(chat,/pub fn recent_model_history\(/);
});
test('Active UI stages finite old/new messages and preserves independent model-facing context',()=>{
  assert.match(jsx,/MAX_VISIBLE_CHAT_ROWS=MAX_HISTORY_ROWS/);
  assert.match(jsx,/loadLatestUiHistory\(id\)/);
  assert.match(jsx,/loadOlderUiHistory\(\)/);
  assert.match(jsx,/loadNewerUiHistory\(\)/);
  assert.match(jsx,/pendingPrepend\.current/);
  assert.match(jsx,/key=\{entry.seq\?\?i\}/);
  assert.doesNotMatch(jsx,/invoke\('lu_history'/);
});
