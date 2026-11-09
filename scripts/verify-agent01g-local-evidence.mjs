// AGENT-01G local diagnostic receipt integrity only, never Lu native acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';

const dir=fileURLToPath(new URL('../evidence/agent-01g/qwen2p5-local-cpu-20261108/',import.meta.url));
const m=JSON.parse(readFileSync(join(dir,'manifest.json'),'utf8'));
const read=(name)=>JSON.parse(readFileSync(join(dir,name),'utf8'));
test('three original receipts are byte-exact, SHA-256 pinned and diagnostics only',()=>{
  assert.equal(m.schema,'lu-agent01g-local-diagnostic-evidence-v1');
  assert.equal(m.evidence_class,'INDEPENDENT_LOCAL_DIAGNOSTIC_NOT_LU_HOST_ACCEPTANCE');
  assert.equal(m.receipts.length,3);
  for(const r of m.receipts){
    assert.match(r.file,/^lu_(?:engine_smoke_receipt|native_gguf_git_diagnostic_(?:receipt|first_attempt))\.json$/);
    const raw=readFileSync(join(dir,r.file));
    assert.equal(raw.length,r.bytes,'wrong byte length for '+r.file);
    assert.equal(createHash('sha256').update(raw).digest('hex'),r.sha256,'tampered receipt '+r.file);
  }
});
test('real local GGUF CPU smoke is not a Lu host or tool authorization pass',()=>{
  const x=read('lu_engine_smoke_receipt.json');
  assert.equal(x.model_basename,m.model.basename);
  assert.equal(x.model_sha256,m.model.sha256);
  assert.equal(x.result.choices[0].message.content,'4');
  assert.equal(x.healthy.status,'ok');
  assert.equal(x.child_exited,true);
  assert.equal(x.result.choices[0].message.tool_calls,undefined);
  assert.equal(m.engine.backend,'CPU');
});
test('one model-chosen Git diagnostic has an independent fixture witness',()=>{
  const x=read('lu_native_gguf_git_diagnostic_receipt.json');
  assert.equal(x.kind,'LOCAL_DIAGNOSTIC_GGUF_GIT_NATIVE_EFFECT_NOT_LU_HOST_ACCEPTANCE');
  assert.equal(x.status,'completed');
  assert.equal(x.tool_calls,1);
  assert.equal(x.model_first_message.tool_calls.length,1);
  assert.equal(x.model_first_message.tool_calls[0].function.name,'read_git_head');
  assert.deepEqual(JSON.parse(x.model_first_message.tool_calls[0].function.arguments),{});
  assert.equal(x.native_child_exit_code,0);
  assert.equal(x.native_child_exited,true);
  assert.equal(x.independent_source_verified,true);
  assert.equal(x.native_tool_stdout,x.independent_expected_head);
  // The receipt stores stdout trimmed, but hashes the worker's raw Git output including LF.
  assert.equal(createHash('sha256').update(x.native_tool_stdout+'\n').digest('hex'),x.native_tool_stdout_sha256);
  assert.equal(x.model_answer_exact_head,false);
});
test('failed tool-looking prose did not execute Git or become a verified step',()=>{
  const x=read('lu_native_gguf_git_diagnostic_first_attempt.json');
  assert.equal(x.status,'incomplete');
  assert.equal(x.engine_child_exited,true);
  assert.equal(x.model_first_message.tool_calls,undefined);
  assert.match(x.error,/did not select exactly one native tool/);
  assert.equal(x.independent_source_verified,undefined);
});
test('independent Qwen3 GH model gate is distinct from these Qwen2.5 diagnostics',()=>{
  assert.equal(m.github_genuine_model_workflow.status,'SEPARATE_GITHUB_CI_GATE_NOT_EVIDENCE_HERE');
  assert.notEqual(m.github_genuine_model_workflow.expected_sha256,m.model.sha256);
  assert.ok(m.limits.some(x=>x.includes('not provide an exact-head')));
});
