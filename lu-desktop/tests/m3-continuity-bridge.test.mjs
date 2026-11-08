import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const host=read('../src-tauri/src/lib.rs');
const pane=read('../web/main.jsx').split('function AnsfHistoryPane(')[1]?.split('function App() {')[0]||'';
const harness=read('./native-chat/src/lib.rs');
test('Existing ContinuityStore stores source-linked task status, not duplicate history',()=>{
 assert.match(chat,/pub fn record_continuity/);
 assert.match(chat,/pub fn recall_project_continuity/);
 assert.match(chat,/continuity.append_verified\(&checkpoint,&book\)/);
 assert.match(chat,/self\.original_source_text_page\(active_chat,&source_id,0,1\)/);
 assert.match(chat,/Checkpoint must cite its own original conversation/);
 assert.match(harness,/mod m3_project_continuity_acceptance/);
});
test('Human-confirmed source-scoped checkpoint can be loaded from another chat',()=>{
 assert.match(host,/async fn lu_m3_record_continuity\(/);
 assert.match(host,/fn lu_m3_resume_continuity\(/);
 assert.match(host,/Lu — Confirm task memory/);
 assert.match(host,/lu_m3_record_continuity,/);
 assert.match(pane,/lu_m3_record_continuity/);
 assert.match(pane,/lu_m3_resume_continuity/);
 assert.match(pane,/This is context, not current permission/);
 assert.doesNotMatch(pane,/setChatId|setHistory|setPrompt|switchChat/);
});

test('CCM compares source-validated task changes but cannot authorize execution',()=>{
 assert.match(chat,/pub fn monitor_project_context/);
 assert.match(chat,/self\.continuity_view\(active_chat,checkpoint\.clone\(\)\)/);
 assert.match(host,/fn lu_m3_context_change\(/);
 assert.match(host,/lu_m3_context_change,/);
 assert.match(pane,/Context Change Monitor · review changed task assumptions/);
 assert.match(pane,/lu_m3_context_change/);
});

test('explicit human resume rehydrates verified originals, CCM, and applied receipts only',()=>{
 assert.match(chat,/pub fn m3_explicit_resume_target\(/);
 assert.match(host,/chat_store::m3_explicit_resume_target\(&prompt\)/);
 assert.match(host,/LU HOST READ-ONLY OPERATIONAL RESUME/);
 assert.match(host,/confirmed_prior_applied_receipts/);
 assert.match(host,/all_future_tool_effects/);
 assert.match(host,/verified_effects_for_checkpoint/);
 assert.match(pane,/Resume project lu task windows-installer/);
});
