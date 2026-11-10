import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  isSha,canEdit,provenanceLabel,draftReplyTarget,
  revisionsBySequence,correctedMessageFollowUp
} from '../web/message_revisions.mjs';

const source=file=>readFileSync(new URL(file,import.meta.url),'utf8');
const rust=source('../src-tauri/src/chat_message_revisions.rs');
const chat=source('../src-tauri/src/ansf_chat_store.rs');
const core=source('../../src-tauri/lu-core/src/ansf_live_chat.rs');
const host=source('../src-tauri/src/lib.rs');
const ui=source('../web/main.jsx');
const design=source('../../docs/LU_CENTRAL_DESIGN.md');

test('source revision data is strongly typed and never changes model input',()=>{
  assert.equal(isSha('a'.repeat(64)),true);
  assert.equal(isSha('A'.repeat(64)),false);
  assert.equal(isSha('ab'),false);
  assert.equal(canEdit({seq:11,role:'assistant'}),true);
  assert.equal(canEdit({seq:1,role:'user'}),true);
  assert.equal(canEdit({seq:0,role:'tool'}),false);
  assert.match(provenanceLabel({currentVersion:2}),/Human display correction v2/);
  const entries=revisionsBySequence([{
    seq:9,role:'assistant',currentVersion:1,
    originalSha256:'a'.repeat(64),currentSha256:'b'.repeat(64),
    originalText:'original',currentText:'human correction'
  }]);
  assert.equal(entries[9].currentText,'human correction');
  assert.equal(revisionsBySequence([{seq:-1,currentVersion:0,
    originalSha256:'a'.repeat(64),currentSha256:'a'.repeat(64),
    originalText:'x',currentText:'x'}])[-1],undefined);
});

test('pinned reply target never loses selected immutable original identity',()=>{
  const revision={currentVersion:3,currentSha256:'c'.repeat(64),
    originalSha256:'a'.repeat(64),currentText:'corrected source text'};
  const selected=draftReplyTarget({seq:42,role:'assistant'},revision);
  assert.deepEqual(selected,{
    seq:42,role:'assistant',version:3,sha:'c'.repeat(64),
    originalSha:'a'.repeat(64),displayText:'corrected source text'
  });
  assert.equal(draftReplyTarget({seq:42,role:'assistant'},{
    ...revision,currentSha256:'no hash'
  }),null);
});

test('Rust host original-turn-to-visible-sequence is indexed, not content guessed',()=>{
  assert.match(core,/pub fn authenticated_chat_turn_visible_seq/);
  assert.match(core,/e\.origin='host' AND e\.user_turn_id=\?2/);
  assert.match(core,/d\.first_seq=e\.seq/);
  assert.match(chat,/pub fn original_user_turn_visible_seq/);
  assert.match(rust,/chats\.original_user_turn_visible_seq\(chat,turn\)/);
  assert.match(rust,/original\(chats,chat,seq\)/);
  assert.match(rust,/revision changed; reopen latest version/i);
});

test('immutable revision chain, pinned reply and crash recovery cannot replay',()=>{
  assert.match(rust,/CREATE TABLE IF NOT EXISTS lu_message_revisions/);
  assert.match(rust,/CREATE TABLE IF NOT EXISTS lu_message_reply_intents/);
  assert.match(rust,/lu_message_revision_no_update/);
  assert.match(rust,/lu_message_revision_no_delete/);
  assert.match(rust,/lu_reply_identity_immutable/);
  assert.match(rust,/OLD\.state!='PREPARED'/);
  assert.match(rust,/TransactionBehavior::Immediate/);
  assert.match(rust,/let Some\(\(seq,text\)\)=chats\.original_user_turn_visible_seq/);
  assert.match(rust,/crash_between_prepare_and_original_message_reconciles_without_reposting/);
  assert.match(rust,/linked_reply_pins_parent_version_and_original_authenticated_turn/);
  assert.doesNotMatch(rust,/run_journaled_observed|Command::new|\.spawn\(/);
});

test('actual native commands are registered and UI requires separate explicit actions',()=>{
  for(const name of [
    'lu_message_revision_page','lu_edit_message_display',
    'lu_post_linked_reply','lu_reconcile_message_replies'
  ]){
    assert.ok(host.includes('fn '+name+'('),name);
    assert.ok(host.includes(name+','),name+' registered');
    assert.ok(ui.includes("'"+name+"'"),name+' invoked');
  }
  assert.match(ui,/messageProvenance\.revision\.originalText/);
  assert.match(ui,/messageProvenance\.revision\.versions\.map/);
  assert.match(ui,/Human-edited display · revision/);
  assert.match(ui,/Post linked human reply/);
  assert.match(ui,/do not repost automatically/);
  assert.match(ui,/Reconcile saved reply links/);
  assert.match(ui,/Original model input and tool history unchanged/);
});

test('central Lu design explicitly carries provenance and display-only constraints',()=>{
  assert.match(design,/UX-CHAT02/);
  assert.match(design,/immutable original/);
  assert.match(design,/reply relationships/i);
  assert.match(design,/human display correction/i);
  assert.match(design,/not rewrite.*tool author/i);
});

test('display correction can be prepared as an explicit NEW human follow-up, never executed',()=>{
  const entry={seq:32,role:'assistant'};
  const revision={
    currentVersion:2,currentSha256:'c'.repeat(64),
    originalSha256:'a'.repeat(64),currentText:'Corrected source-backed answer'
  };
  const prepared=correctedMessageFollowUp(entry,revision);
  assert.equal(prepared.target.seq,32);
  assert.equal(prepared.target.version,2);
  assert.equal(prepared.target.sha,revision.currentSha256);
  assert.match(prepared.draft,/NEW human follow-up/);
  assert.match(prepared.draft,/Corrected source-backed answer/);
  assert.equal(correctedMessageFollowUp(entry,{...revision,currentVersion:0}),null);
  assert.equal(correctedMessageFollowUp(entry,{...revision,currentSha256:'invalid'}),null);
  assert.equal(correctedMessageFollowUp(entry,{...revision,currentSha256:revision.originalSha256}),null);
  assert.equal(correctedMessageFollowUp(entry,{...revision,currentText:'☕'.repeat(7000)}),null);
  assert.match(ui,/Prepare human follow-up from correction/);
  assert.match(ui,/setLinkedReplyTarget\(\{\.\.\.prepared\.target,chatId\}\)/);
  assert.match(ui,/setLinkedReplyText\(prepared\.draft\)/);
  assert.match(ui,/Posting it does not trigger inference or inherit earlier approval/);
  assert.doesNotMatch(rust,/run_journaled_observed|Command::new|\.spawn\(/);
});
