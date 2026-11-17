import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs');
const perf=read('../src-tauri/src/perf01_telemetry.rs');
const dx=read('../../src-tauri/lu-core/src/diagnostic_localization.rs');
const ui=read('../web/main.jsx');
test('CP01E native transaction uses current order and existing validated DX localization',()=>{
  const gen=host.slice(host.indexOf('async fn generate_internal('));
  const points=['SessionReady','UserSaved','WorkerEntered','SavedAttested','ModelEntered','ModelReturned','AssistantSaved'];
  let prev=-1;
  for(const name of points){
    const p=gen.indexOf('causal_checkpoint(CausalCheckpoint::'+name+')');
    assert.ok(p>prev,name);prev=p;
  }
  assert.match(gen,/saved\?;\s*perf_trace.causal_checkpoint\(CausalCheckpoint::AssistantSaved\)/);
  assert.match(perf,/diagnostic_localization::localize_transactional_turn\(&trace\)/);
  assert.match(dx,/\[4, 19, 2, 7, 5, 8, 9, 12, 10, 11, 13\]/);
  assert.match(dx,/localize_path\(trace, NORMAL_TURN\)/);
});
test('CP01E original native reply observation is exact-attempt scoped and passive',()=>{
  const ack=host.slice(host.indexOf('fn lu_cp01_reply_observed('),host.indexOf('/// Explicitly copy exact ordinary saved images'));
  assert.match(ack,/caller.label\(\)!="main"/);
  assert.match(ack,/image_submission_generation\(&chat_id,&submission_id\)\?!=generation_id/);
  assert.match(ack,/authenticated_chat_turn_visible_seq\(&chat_id,&generation_id\)/);
  assert.match(perf,/row.terminal_state=="COMPLETED"/);
  assert.match(perf,/t.trace_ref\(\)==trace_ref/);
  assert.match(perf,/#\[serde\(skip\)\]\s*cp01_trace/);
  assert.match(ui,/const result=await invoke\('lu_submit_image_turn'/);
  assert.match(ui,/result.receipt.generationId===attempt.generationId/);
  assert.match(ui,/void invoke\('lu_cp01_reply_observed'/);
  assert.match(host,/authenticated_completed_assistant_seq\(&chat_id,&generation_id\)/);
  assert.match(ui,/useLayoutEffect\(\(\)=>\{[\s\S]*?deliveryCandidate/);
});
