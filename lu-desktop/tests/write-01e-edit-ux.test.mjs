import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fullEditChangeLines} from '../web/edit_diff.mjs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const queue=readFileSync(new URL('../src-tauri/src/edit_proposal.rs',import.meta.url),'utf8');
const executor=readFileSync(new URL('../src-tauri/src/native_host_edit.rs',import.meta.url),'utf8');

test('complete edit diff retains all old, new and unchanged lines',()=>{
  const old='alpha\nold 🍎\ntrailing\n';
  const fresh='alpha\nnew 🍐\ntrailing\n';
  assert.equal(fullEditChangeLines(old,fresh),
    '  alpha\n- old 🍎\n+ new 🍐\n  trailing\n  ');
  assert.ok(fullEditChangeLines('a\n'+ 'x\n'.repeat(6000) + 'b',
    'a\n'+ 'y\n'.repeat(6000) + 'b').includes('+ y'));
  assert.throws(()=>fullEditChangeLines(null,'x'),TypeError);
});
test('desktop proposal is inert and complete before-after review is exposed',()=>{
  assert.ok(host.includes('state.edit_proposals.stage('));
  assert.ok(host.includes('state.edit_approval_ledger.stage('));
  assert.ok(host.includes('app.dialog().file().blocking_pick_file()'));
  assert.ok(host.includes('app.dialog().message(confirm_text)'));
  assert.ok(host.includes('state.edit_approval_ledger.approve('));
  assert.ok(host.includes('execute_approved_edit_pinned('));
  assert.ok(ui.includes('fullEditChangeLines(proposal.selected.original,proposal.selected.replacement)'));
  assert.ok(ui.includes('aria-label="Complete original"'));
  assert.ok(ui.includes('aria-label="Complete replacement"'));
  assert.ok(ui.includes('disabled={!editReviewed[proposal.proposalId]}'));
  assert.ok(ui.includes("invoke('lu_confirm_edit_file'"));
  assert.ok(ui.includes("invoke('lu_inspect_edit_recovery'"));
});
test('native path authority never comes from proposed model JSON',()=>{
  assert.ok(queue.includes('proposal.kind!="propose_edit_file"'));
  assert.ok(queue.includes('open_verified_selected_file(&exact)'));
  assert.ok(queue.includes('Dir::open_ambient_dir(&canonical,ambient_authority())'));
  assert.ok(executor.includes('register_edit_tools_pinned'));
  assert.ok(executor.includes('authority.dispatch_bound_durable('));
  assert.ok(host.includes('observe_edit(&dir,&call_id)'));
  assert.ok(!host.includes('rollback_on_failure'));
});
