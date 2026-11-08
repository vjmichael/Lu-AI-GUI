import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/operational_asset_ledger.rs');
const host=read('../src-tauri/src/lib.rs');
const jsx=read('../web/main.jsx');
test('Operational memory uses separate immutable reference records, not ANSF blobs',()=>{
  assert.match(core,/CREATE TABLE IF NOT EXISTS lu_operational_asset_receipts/);
  assert.match(core,/source_id TEXT NOT NULL/);
  assert.match(core,/fn digest_file\(path:&Path\)/);
  assert.match(core,/AssetPresence::PresentVerified/);
  assert.match(core,/pub fn revalidate\(/);
  assert.match(core,/AssetError::SourceDenied/);
  assert.match(core,/asset observations are immutable/);
});
test('Real host picker and source book gate; no model-chosen package path or execution',()=>{
  assert.match(host,/async fn lu_m3_remember_asset\(/);
  assert.match(host,/blocking_pick_file\(\)/);
  assert.match(host,/original_source_text_page\(&chat_id,&source_id,0,1\)/);
  assert.match(host,/record_verified\(&proposal/);
  assert.match(host,/async fn lu_m3_operational_inventory\(/);
  assert.match(host,/spawn_blocking\(move\|\|/);
  const pane=jsx.slice(jsx.indexOf('function AnsfHistoryPane('),jsx.indexOf('function App() {'));
  assert.match(pane,/lu_m3_operational_inventory/);
  assert.match(pane,/lu_m3_remember_asset/);
  assert.doesNotMatch(pane,/setChatId|setHistory|setPrompt|switchChat/);
});
