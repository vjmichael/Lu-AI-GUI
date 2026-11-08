import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const source=read('../src-tauri/src/ansf_chat_store.rs');
const core=read('../../src-tauri/lu-core/src/ansf.rs');
const host=read('../src-tauri/src/lib.rs');
const jsx=read('../web/main.jsx');
const css=read('../web/styles.css');
test('Read original ANSF entries through bounded stable backward cursor',()=>{
  assert.match(core,/pub fn page_before\(/);
  assert.match(core,/id<\?2 AND role IN \('user','assistant'\)/);
  assert.match(source,/pub fn original_history_page\(/);
  assert.match(source,/self\.require\(active_chat\)/);
  assert.match(source,/self\.require\(source_chat\)/);
  assert.match(source,/pub fn original_source_text_page\(/);
  assert.match(host,/fn lu_m3_history_page\(/);
  assert.match(host,/fn lu_m3_source_text_page\(/);
});
test('History browser is a genuinely independent overlay, not active transcript navigation',()=>{
  const pane=jsx.slice(jsx.indexOf('function AnsfHistoryPane('),jsx.indexOf('function App() {'));
  assert.match(pane,/Independent ANSF history browser/);
  assert.match(pane,/lu_m3_history_page/);
  assert.match(pane,/lu_m3_source_text_page/);
  assert.doesNotMatch(pane,/\b(setChatId|setHistory|setPrompt|setImages|switchChat)\s*\(/);
  assert.match(jsx,/\{historyPaneOpen&&chatId&&<AnsfHistoryPane/);
  assert.doesNotMatch(jsx,/aria-label="ANSF memory recall"/);
  assert.match(css,/position:fixed;top:0;right:0;bottom:0/);
});
test('Human can load old pages and continue full original source in bounded slices',()=>{
  assert.match(jsx,/nextBeforeId/);
  assert.match(jsx,/pendingPaneAnchor\.current/);
  assert.match(jsx,/lu_m3_history_page_after/);
  assert.match(jsx,/PANE_VISIBLE_ROWS=192/);
  assert.match(jsx,/offsetChars:previous\.nextOffsetChars/);
  assert.match(jsx,/Read next source page/);
});
