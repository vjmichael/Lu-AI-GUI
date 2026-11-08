import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/operational_asset_ledger.rs');
const host=read('../src-tauri/src/lib.rs');
const pane=read('../web/main.jsx').split('function AnsfHistoryPane(')[1]?.split('function App() {')[0]||'';
test('automatic reuse requires original-source profile check AND an unchanged SHA-256',()=>{
 assert.match(core,/pub fn resolve_for_reuse\(/);
 assert.match(core,/AssetReuseState::VerifiedPresent/);
 assert.match(core,/AssetReuseState::RecordedMissing/);
 assert.match(core,/AssetReuseState::RecordedChanged/);
 assert.match(core,/mut source_allowed:impl FnMut/);
 assert.match(core,/if !source_allowed\(&record.source_id\)\{continue\}/);
 assert.match(core,/self\.revalidate\(&record\)/);
 assert.match(core,/fn automatic_reuse_checks_source_and_hash_then_falls_back_to_valid_cached_location/);
});
test('host automatically reconciles existing package availability on task resumption, without executing',()=>{
 assert.match(host,/async fn lu_m3_resume_resources\(/);
 assert.match(host,/async fn lu_m3_resolve_asset\(/);
 assert.match(host,/state\.operational_assets\.resolve_for_reuse\(/);
 assert.match(host,/state\.chats\.original_source_text_page\(&chat_id,source,0,1\)/);
 assert.match(pane,/lu_m3_resume_resources/);
 assert.match(pane,/Automatically revalidated project resources/);
 assert.match(pane,/Only verified_present files may be considered for reuse/);
 assert.doesNotMatch(pane,/setChatId|switchChat/);
});
