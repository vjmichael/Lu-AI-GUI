import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs');
const store=read('../src-tauri/src/attachment_store.rs');
const supervisor=read('../../src-tauri/lu-core/src/inference_supervisor.rs');
const bridge=read('../../src-tauri/lu-core/src/inference_bridge.rs');
const adapter=read('../../src-tauri/lu-core/src/persistent_llama_backend.rs');
const http=read('../../src-tauri/lu-core/src/multimodal_http.rs');
const perf=read('../src-tauri/src/perf01_telemetry.rs');
const ui=read('../web/main.jsx');
test('ordinary image Send reopens canonical manifest and uses the reported image lane',()=>{
  assert.match(host,/verify_saved_exact\(&state.chats,&chat_id,&gid,&manifest,&images\[\.\.manifest.len\(\)\]\)/);
  assert.match(host,/reserve_generation_images_with_history\(/);
  assert.match(host,/run_reserved_images_reported\(/);
  assert.match(host,/perf_trace.image_delivery\(image_report\)/);
  assert.match(store,/original\.as_slice\(\)!=expected/);
  assert.match(store,/&self.read\(chat,a\)\?!=p/);
});
test('native reservation binds the exact serialized frame and refuses text downgrade',()=>{
  assert.match(supervisor,/image_request_digest:Some\(digest\)/);
  assert.match(supervisor,/Self::image_request_digest\(&frame\)\?!=expected/);
  assert.match(supervisor,/active.image_request_digest.is_some\(\)\{return Err\(SupervisorError::InvalidRequest\)\}/);
  assert.match(bridge,/actual!=pinned.sha256/);
  assert.match(supervisor,/session.pid.is_some\(\)&&engine.process_id\(\).is_none\(\)/);
});
test('final backend payload is checked after sampling and cache choices and upload is actually observed',()=>{
  assert.match(adapter,/self.sampling.apply\(&mut json\);[\s\S]*?generate_verified_image_body\(/);
  assert.match(http,/parts\[1\.\.\]\.iter\(\).zip\(images\)/);
  assert.match(http,/bytes!=expected.bytes\|\|digest!=expected.sha256/);
  assert.match(http,/write_checked\(&mut socket, body.as_bytes\(\), &cancelled, deadline\)\?;\s*on_uploaded\(\)/);
  assert.match(adapter,/receipt.worker_idle_verified=verified.is_ok\(\)/);
});
test('backend faults do not count as completed semantic inference or expose engine bodies',()=>{
  assert.match(http,/Some\("length"\)=>clipped=true/);
  assert.match(http,/CP01_IMAGE_NO_VISIBLE_ANSWER/);
  assert.match(http,/CP01_IMAGE_BACKEND_EVENT/);
  assert.match(perf,/CP01_IMAGE_INSUFFICIENT_BACKEND_EVIDENCE/);
  assert.match(host,/exact_causal_turn\(&chat_id,&gid,&diagnostic_ref\)/);
  assert.match(ui,/result.error\|\|'Response failed\.'/);
});
test('genuine saved image oracle requires two singles and two differently ordered four-image turns',()=>{
  assert.match(store,/fn actual_cp01_saved_one_and_four_images_reach_supervised_mmproj/);
  assert.match(store,/order.shuffle\(&mut OsRng\)/);
  assert.match(store,/order.to_vec\(\),rotated.to_vec\(\)/);
  assert.match(store,/assert_eq!\(words,expected,/);
  assert.match(store,/report.worker_idle_verified/);
  assert.match(store,/LU_CP01_MMPROJ_SHA256/);
});
