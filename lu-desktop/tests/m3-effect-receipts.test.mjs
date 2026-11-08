import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const continuity=read('../../src-tauri/lu-core/src/continuity.rs');
const desktop=read('../src-tauri/src/lib.rs');
const pane=read('../web/main.jsx').split('function AnsfHistoryPane(')[1]?.split('function App() {')[0]||'';
test('a checkpoint links only an immutable real APPLIED effect proof',()=>{
 assert.match(continuity,/CREATE TABLE IF NOT EXISTS continuity_verified_effects/);
 assert.match(continuity,/effect evidence is immutable/);
 assert.match(continuity,/pub fn link_verified_effect\(/);
 assert.match(continuity,/pub fn verified_effects_for_checkpoint\(/);
 assert.match(continuity,/record\.state!=EffectState::Applied/);
 assert.match(continuity,/Sha256::digest\(bytes\)/);
 assert.match(continuity,/actual\.state!=EffectState::Applied/);
});
test('native confirmation and exact checkpoint/source access guard effect proof reads',()=>{
 assert.match(desktop,/fn lu_m3_task_effects\(/);
 assert.match(desktop,/async fn lu_m3_link_applied_effect\(/);
 assert.match(desktop,/Lu — Link applied tool receipt/);
 assert.match(desktop,/state\.chats\.recall_project_continuity\(/);
 assert.match(desktop,/state\.effects\.get\(&call_id\)/);
 assert.match(pane,/lu_m3_task_effects/);
 assert.match(pane,/lu_m3_link_applied_effect/);
 assert.match(pane,/Previously completed native effects/);
 assert.doesNotMatch(pane,/setChatId|setHistory|setPrompt/);
});
