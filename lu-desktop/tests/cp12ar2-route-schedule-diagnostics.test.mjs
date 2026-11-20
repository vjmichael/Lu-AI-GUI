import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'), negotiation=read('../src-tauri/src/capability_negotiation.rs'), trace=read('../src-tauri/src/causal_diagnostic.rs');
test('CP12AR2 negotiated and explicit native routes consume the exact original diagnostic prefix',()=>{
 assert.match(host,/try_read_turn\([^\n]*&mut perf_trace\)/);
 for(const route of ['reader','utility','read','browser','reviewed']) assert.match(negotiation,new RegExp('execute_negotiated_'+route+'\\([^\\n]*perf_trace.take_causal_progress\\(\\)\\)'));
 assert.match(trace,/Self::resume_parent\(trace,host.perf01.archive\(\)\)/);
 assert.match(host,/generate_browser_for_chat\([^\n]*cp01_trace\)/);
 assert.match(host,/ask_reviewed_for_chat\([\s\S]*?cp01_trace/);
});
test('CP12AR2 traces real browser and supervised dispatch boundaries, not synthetic search proposals',()=>{
 const browser=read('../src-tauri/src/browser_model_desktop.rs'),reviewed=read('../src-tauri/src/reviewed_app_desktop.rs'),web=read('../src-tauri/src/model_web_research_desktop.rs');
 assert.match(browser,/if search.is_some\(\)\{trace.lost\(\);\}/);
 assert.match(browser,/ToolDispatchCheckpoint::Entered[\s\S]*?registry.dispatch\(invocation,policy.as_ref\(\)\)[\s\S]*?ToolDispatchCheckpoint::Returned/);
 assert.match(browser,/record_browser_read\([\s\S]*?ToolDispatchCheckpoint::FeedbackAudited/);
 for(const route of [reviewed,web]){
  assert.match(route,/trace.record\(\|t\|t.tool_checkpoint\(point\)\)/);
  assert.match(route,/trace.saved_response\(/);
 }
});
test('CP12AR2 scheduled receipt is checked independently and shown only as plain language',()=>{
 const scheduler=read('../src-tauri/src/scheduled_ai.rs'),ui=read('../web/native_windows.jsx');
 assert.match(scheduler,/execute_ai_reader\([\s\S]*?record_ai_reader_diagnostic\(/);
 assert.match(scheduler,/answer.trim\(\) != receipt.verified_text[\s\S]*?AiDiagnosticStep::AnswerMismatch/);
 assert.match(ui,/scheduledRuns.recent\?\.slice\(0,8\)/);
 assert.match(ui,/run.checkpointDescription/);
 assert.doesNotMatch(ui,/run.sourceSha256|run.verifiedText/);
});
