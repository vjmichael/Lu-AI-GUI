import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'),trace=read('../src-tauri/src/causal_diagnostic.rs'), archive=read('../src-tauri/src/diagnostic_archive.rs'), workspace=read('../src-tauri/src/workspace_desktop.rs');
test('CP12AR2R1 CPU routes use native diagnostic checkpoints and exact saved readback, not model checkpoints',()=>{
 const names=['CpuChatCounts','CpuModelStatus','CpuChatLiteral(needle)','CpuClock'];
 for(const name of names){
  const start=host.indexOf('TurnRoute::'+name+'=>');assert.ok(start>=0,name);const end=host.indexOf('TurnRoute::',start+11);const route=host.slice(start,end);
  for(const mark of ['NativeDiagnosticTrace::cpu','CausalCheckpoint::CpuEntered','CausalCheckpoint::CpuReturned','CausalCheckpoint::UserSaved','trusted_user_turn_text','CausalCheckpoint::SavedAttested','trace.saved_response'])assert.ok(route.includes(mark),`${name} ${mark}`);
  assert.ok(route.indexOf('CpuReturned')<route.indexOf('append_user_with_turn_id'),`${name} pre-read order`);
  assert.doesNotMatch(route,/CausalCheckpoint::ModelEntered|CausalCheckpoint::ModelReturned/);
 }
 assert.match(workspace,/verify_user_turn\(&chat_id,&gid\)[\s\S]*?TaskAuthorized[\s\S]*?CpuEntered/);
 assert.match(trace,/CPU_APPROVED_ORDER/);
});
test('CP12AR2R1 bounded contention retry preserves the durable-prefix, terminal and recovery guards',()=>{
 assert.match(archive,/Duration::from_millis\(250\)/);
 assert.match(archive,/DatabaseBusy\|rusqlite::ErrorCode::DatabaseLocked/);
 assert.match(archive,/TryLockError::Poisoned\(_\)\)=>return false/);
 assert.match(archive,/transaction_with_behavior\(rusqlite::TransactionBehavior::Immediate\)/);
 assert.match(archive,/old_state!="live"/);assert.match(archive,/!trace.events.starts_with\(&old.events\)/);
 assert.match(archive,/tx.commit\(\)\?/);
});
test('CP12AR2R1 workspace error leaves a terminal finding, does not assert successful execution',()=>{
 assert.match(workspace,/if result.is_err\(\)\{trace.native_error\(ticket.cancelled\(\)\|\|cancel.is_cancelled\(\)\)/);
 assert.match(workspace,/if result.tool_calls==0 \{\s*trace.record\(\|t\|t.model_failed_with\(10\)\)/);
 assert.match(trace,/if !self.terminal\(\)&&!self.saved\(\)\{self.append\(21,3,1\)/);
});
