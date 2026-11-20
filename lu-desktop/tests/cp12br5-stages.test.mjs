import {readFileSync} from 'node:fs';import test from 'node:test';import assert from 'node:assert/strict';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'),ui=read('../web/native_windows.jsx'),ipc=read('../src-tauri/src/dx_ipc.rs'),stage=read('../src-tauri/src/dx_stage.rs');
test('stage sharing consents before projection/inference, receiver gets no peer verdict in prompt',()=>{
 const a=host.slice(host.indexOf('async fn lu_dx_stage_share('),host.indexOf('async fn lu_dx_stage_receive('));
 const b=host.slice(host.indexOf('async fn lu_dx_stage_receive('),host.indexOf('/// CP-11A:',host.indexOf('async fn lu_dx_stage_receive(')));
 assert.ok(a.indexOf('.blocking_show()')<a.indexOf('archive.report()'));assert.ok(a.indexOf('run_stages')<a.indexOf('review.publish'));assert.ok(b.indexOf('.blocking_show()')<b.indexOf('receive_stage_review_cancellable'));
 assert.match(b,/run_stages\(&owner,&review,&snapshot,peer.summary.clone\(\),peer.evidence.clone\(\)\)/);assert.match(b,/local.verdict!=peer.verdict/);assert.match(b,/review.publish/);assert.match(ipc,/const STAGE_VERSION: u32 = 3/);
});
test('stage review is a separate optional UI action, with cautious findings and unchanged count-only actions',()=>{
 assert.match(ui,/invoke\('lu_dx_stage_share'\)/);assert.match(ui,/invoke\('lu_dx_stage_receive'/);assert.match(ui,/incoming.startsWith\('lu3-'\)/);assert.match(ui,/invoke\('lu_dx_review_share'\)/);assert.match(ui,/Both findings match the checked stage counts/);assert.match(ui,/No root cause or answer accuracy/);assert.match(ui,/A unique affected step cannot be established/);assert.doesNotMatch(stage,/pub .*:String/);
});
