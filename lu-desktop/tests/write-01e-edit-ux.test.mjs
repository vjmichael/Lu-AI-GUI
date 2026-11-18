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

test('undo preview requires recovery inspection and remains read-only',()=>{
  assert.ok(ui.includes("editObservations[record.proposalId]?.rollbackPreview"));
  assert.ok(ui.includes('aria-label="Verified original text"'));
  assert.ok(ui.includes('aria-label="Proposed restore change lines"'));
  assert.ok(ui.includes('A native confirmation'));
  assert.ok(ui.includes("invoke('lu_confirm_rollback'"));
  assert.ok(ui.includes("invoke('lu_deny_rollback'"));
  assert.ok(ui.includes("invoke('lu_rollback_history'"));
  assert.ok(host.includes('inspect_rollback_candidate('));
  assert.ok(host.includes('record.original_sha256.as_deref()'));
  assert.ok(!ui.includes("invoke('lu_execute_rollback'"));
});

test('verified edit receipts are required for recovery undo preview and folder links',()=>{
  assert.ok(host.includes('verified_edit_receipt('));
  assert.ok(host.includes('let receipt_verified=effect_record.as_ref()'));
  assert.ok(host.includes('let rollback_preview = if receipt_verified &&'));
  assert.ok(ui.includes("record.receiptVerified ? 'Verified' : 'Not verified'"));
  assert.ok(ui.includes("record.approval === 'Approved' && record.receiptVerified"));
  assert.ok(ui.includes('This restoration has its own authorization. A native confirmation'));
  assert.ok(!ui.includes("invoke('lu_execute_rollback'"));
});

test('separate durable rollback authorization and native confirmation are mandatory',()=>{
  assert.ok(host.includes('state.rollback_approval_ledger.stage(&approval)'));
  assert.ok(host.includes('state.rollback_approval_ledger.approve(&selected.record,&effect_id)'));
  assert.ok(host.includes("app.dialog().message(msg)"));
  assert.ok(host.includes('execute_approved_rollback_pinned('));
  assert.ok(host.includes('state.rollback_reviews.take(&review_id,&chat_id)'));
  assert.ok(ui.includes('I reviewed the complete current file, original version and restoration changes.'));
  assert.ok(ui.includes('disabled={!rollbackReviewed[editObservations[record.proposalId].rollbackReviewId]}'));
  assert.ok(ui.includes('onClick={() => confirmRollback(record)}'));
  assert.ok(ui.includes('onClick={() => denyRollback(record)}'));
  assert.ok(ui.includes('Restoration history and recovery'));
  assert.ok(!ui.includes("invoke('lu_execute_rollback'"));
});
