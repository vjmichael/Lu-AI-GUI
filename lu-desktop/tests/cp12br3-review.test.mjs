import {readFileSync} from 'node:fs';import test from 'node:test';import assert from 'node:assert/strict';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'),ui=read('../web/native_windows.jsx'),review=read('../src-tauri/src/dx_review.rs'),ipc=read('../src-tauri/src/dx_ipc.rs');
test('collaborative review has separate native approval and no renderer-supplied evidence',()=>{
 const a=host.slice(host.indexOf('async fn lu_dx_review_share('),host.indexOf('async fn lu_dx_review_receive('));
 const b=host.slice(host.indexOf('async fn lu_dx_review_receive('),host.indexOf('fn lu_dx_stop_review('));
 assert.ok(a.indexOf('.blocking_show()')<a.indexOf('archive.report()'));assert.ok(a.indexOf('dx_review::run')<a.indexOf('start_review(finding)'));
 assert.ok(b.indexOf('.blocking_show()')<b.indexOf('dx_ipc::receive_review'));assert.match(b,/peer\.summary\.clone\(\)/);assert.match(b,/local\.finding!=peer\.finding/);assert.match(a,/notify\.emit\("lu-model-catalog-changed"/);assert.match(b,/notify\.emit\("lu-model-catalog-changed"/);
 assert.match(review,/reserve_generation_with_history\(sid,&gid,&rid,&prompt,TOKENS,&\[\]\)/);assert.match(review,/ReviewedSummary::checked\(summary,&text\)/);assert.doesNotMatch(review,/ToolCall|load_model|TcpStream/);
});
test('review uses separate invitation protocol and opt-in UI',()=>{
 assert.match(ipc,/const REVIEW_VERSION: u32 = 2/);assert.match(ipc,/auth_request_for\(&key/);assert.match(ipc,/Finding::expected/);assert.match(ui,/onClick=.*invoke\('lu_dx_review_share'\)/);assert.match(ui,/onClick=.*invoke\('lu_dx_review_receive'/);assert.match(ui,/onClick=.*invoke\('lu_dx_stop_review'\)/);assert.match(ui,/No root cause or answer accuracy/);
});
