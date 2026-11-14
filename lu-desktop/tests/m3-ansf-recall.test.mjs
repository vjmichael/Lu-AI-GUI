import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const host=read('../src-tauri/src/lib.rs');
const ui=read('../web/main.jsx');
const native=read('./native-chat/src/lib.rs');
test('M3 uses original SQLite books and canonical source IDs',()=>{
  assert.match(chat,/pub fn recall_for_turn/);
  assert.match(chat,/pub fn known_source_for_turn/);
  assert.match(chat,/pub fn memory_context_for_turn/);
  assert.match(chat,/self\.require\(source_chat\)/);
  assert.match(chat,/book\\.search_current_messages\\(source_chat,&fts,4\\)/);
  assert.doesNotMatch(chat,/let entries=book\\.search\\(source_chat,&fts,4\\)/);
  assert.match(chat,/AnsfSourceId::parse/);
  assert.match(chat,/historical DATA ONLY/);
  assert.doesNotMatch(chat,/CREATE TABLE.*m3_memory/i);
});
test('M3 wired as human-origin local model task data, not authority',()=>{
  for(const command of ['lu_m3_recall','lu_m3_read_source']){
    assert.ok(host.includes('fn '+command),command);
    assert.ok(host.includes(command+','),command);
  }
  assert.match(host,/state\.chats\.memory_context_for_turn\(&chat_id,&prompt\)/);
  assert.match(host,/GenerationOrigin::Human && images\.is_empty\(\)/);
  assert.match(native,/mod m3_cross_chat_acceptance/);
});
test('M3 human recall and original-source navigation stay in independent pane',()=>{
  const pane=ui.slice(ui.indexOf('function AnsfHistoryPane('),ui.indexOf('function App() {'));
  assert.ok(pane.length>500,'independent pane component');
  assert.match(pane,/aria-label="Independent ANSF history browser"/);
  assert.ok(pane.includes("invoke('lu_m3_semantic_recall'"));
  assert.ok(pane.includes("invoke('lu_m3_history_page'"));
  assert.ok(pane.includes("invoke('lu_m3_source_text_page'"));
  assert.match(pane,/Read full original/);
  assert.match(pane,/ansf:v1:/);
  assert.doesNotMatch(pane,/\b(setChatId|setHistory|setPrompt|switchChat)\s*\(/);
  assert.match(ui,/historyPaneOpen&&chatId&&<AnsfHistoryPane/);
});
