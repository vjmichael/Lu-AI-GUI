import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const contents=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const ipc=contents('../src-tauri/src/dx_ipc.rs');
const pipe=contents('../src-tauri/src/dx_windows_pipe.rs');
const manifest=contents('../src-tauri/Cargo.toml');
const host=contents('../src-tauri/src/lib.rs');
test('CP-12B-R7 Windows stage reviews remain explicitly opt-in, not shipped in default features',()=>{
 assert.match(manifest,/windows_dx_eval\s*=\s*\[\]/);
 assert.doesNotMatch(manifest,/default\s*=\s*\[[^\]]*windows_dx_eval/);
 assert.match(ipc,/all\(target_os\s*=\s*"windows",\s*feature\s*=\s*"windows_dx_eval"\)/);
 assert.match(ipc,/review_supported\(\)->bool\{cfg!\(any\(target_os="linux",all\(target_os="windows",feature="windows_dx_eval"/);
 assert.match(ipc,/fn cp12br1_windows_transport_is_unavailable_without_explicit_qualification\(/);
});
test('stage review uses same Windows protected endpoint and native per-message peer verification',()=>{
 assert.match(pipe,/FILE_FLAG_FIRST_PIPE_INSTANCE/);assert.match(pipe,/PIPE_REJECT_REMOTE_CLIENTS/);
 assert.match(pipe,/SE_DACL_PROTECTED/);assert.match(pipe,/size\.AceCount != 1/);
 assert.match(pipe,/ImpersonateNamedPipeClient\(pipe\.as_raw_handle\(\)\)/);
 assert.match(pipe,/OpenThreadToken\(GetCurrentThread\(\), TOKEN_QUERY, 1/);
 assert.match(pipe,/RevertToSelf\(\)/);
 assert.match(pipe,/auth_request_for\(&key, &b, version\)/);
 assert.match(pipe,/response_full\(&key, r\.nonce, summary\.clone\(\), version, finding, stages\.clone\(\)\)/);
 assert.match(pipe,/auth_response_full\(&key, &nonce, &read\(&h, stop, end\)\?, version\)/);
});
test('stage invitation and cancellation are versioned, bounded, single use',()=>{
 assert.match(pipe,/start_inner_version\(review\.summary, STAGE_VERSION, review\.verdict, Some\(review\.evidence\), TTL\)/);
 assert.match(pipe,/e\.expected\(&summary\)==Some\(finding\)/);
 assert.match(pipe,/if code\.len\(\) != 101/);
 assert.match(pipe,/let end = Instant::now\(\) \+ FRAME/);
 assert.match(pipe,/expired\(stop, end\)\?/);
 assert.match(pipe,/let authenticated = read\(&h, &w, end\)/);
 assert.match(pipe,/break;\s*}\s*if unsafe \{ DisconnectNamedPipe/);
 assert.match(pipe,/fn cp12br7_stage_cancel_mid_frame_keeps_invitation_unconsumed\(/);
 assert.match(pipe,/fn cp12br7_stage_sender_crash_invalidates_invitation\(/);
 assert.match(pipe,/fn cp12br7_stage_different_account_kernel_denial\(/);
});
test('two review processes must independently classify native stage aggregates; never accept peer labels as evidence',()=>{
 const share=host.slice(host.indexOf('async fn lu_dx_stage_share('),host.indexOf('async fn lu_dx_stage_receive('));
 const receive=host.slice(host.indexOf('async fn lu_dx_stage_receive('),host.indexOf('async fn lu_run_local_checks('));
 assert.ok(share.indexOf('.blocking_show()')<share.indexOf('dx_review::run_stages('));
 assert.ok(receive.indexOf('.blocking_show()')<receive.indexOf('dx_ipc::receive_stage_review_cancellable'));
 assert.match(receive,/dx_review::run_stages\(&owner,&review,&snapshot,peer\.summary\.clone\(\),peer\.evidence\.clone\(\)\)/);
 assert.match(receive,/if local\.verdict!=peer\.verdict\{return Err/);
 assert.doesNotMatch(receive,/TCP|Shell|Command::new|auto_repair/);
});
