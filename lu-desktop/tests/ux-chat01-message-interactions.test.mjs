import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  DEFAULT_APPEARANCE,CHAT_APPEARANCE_PRESETS,
  MESSAGE_REACTIONS,safeAppearance,isSafeHexColor,
  textForBackground,visibleReactionBySeq
} from '../web/message_interactions.mjs';

const source=(name)=>readFileSync(new URL(name,import.meta.url),'utf8');
const desktop=source('../web/main.jsx');
const host=source('../src-tauri/src/lib.rs');
const rust=source('../src-tauri/src/chat_message_interactions.rs');
const css=source('../web/styles.css');
const design=source('../../docs/LU_CENTRAL_DESIGN.md');

test('only canonical hex colors are accepted; CSS/URLs never reach appearance',()=>{
  assert.equal(isSafeHexColor('#0aB1f2'),true);
  assert.equal(isSafeHexColor('url(https://host.example)'),false);
  assert.equal(isSafeHexColor('#ff0000!important'),false);
  assert.equal(safeAppearance({background:'#1B2C3D',
    userBubble:'#124578',assistantBubble:'#ffFEFF'}).background,'#1B2C3D');
  assert.deepEqual(safeAppearance({background:'red',
    userBubble:'#124578',assistantBubble:'#ffffff'}),DEFAULT_APPEARANCE);
  assert.equal(textForBackground('#ffffff'),'#142031');
  assert.equal(textForBackground('#07090f'),'#f5faff');
  assert.ok(CHAT_APPEARANCE_PRESETS.length>=4);
});

test('reaction source is a single consistent approved emoji set',()=>{
  assert.equal(MESSAGE_REACTIONS.length,6);
  assert.equal(new Set(MESSAGE_REACTIONS).size,MESSAGE_REACTIONS.length);
  assert.equal(visibleReactionBySeq([
    {seq:12,emoji:'❤️'}, {seq:13,emoji:'not-allowed'},
    {seq:0,emoji:'👍'}
  ])[12],'❤️');
  assert.equal(visibleReactionBySeq([
    {seq:12,emoji:'❤️'}, {seq:13,emoji:'not-allowed'}
  ])[13],undefined);
  assert.match(rust,/const ALLOWED_REACTIONS:\[&str;6\]/);
  for(const emoji of MESSAGE_REACTIONS){
    assert.ok(rust.includes(emoji));
  }
});

test('native host enforces saved original chat/sequence and does not mutate ANSF source',()=>{
  assert.match(rust,/confirm_message\(chat,seq,chats\)/);
  assert.match(rust,/chats\.history_window\(chat,Some\(seq\+1\),"older",1\)/);
  assert.match(rust,/if page\.entries\.first\(\)\.map\(\|m\|m\.seq\)!=Some\(seq\)/);
  assert.match(rust,/SQLITE_OPEN_READ_ONLY/);
  assert.match(rust,/PRIMARY KEY\(chat_id,seq\)/);
  assert.match(rust,/local_reactions_and_colors_persist_across_full_chat_store_restart/);
  assert.match(rust,/cannot_react_to_unknown_message_or_other_conversation/);
  assert.doesNotMatch(rust,/run_journaled_observed|TaskStore::resume|Command::new/);
  for(const cmd of ['lu_message_decorations','lu_set_message_reaction','lu_set_chat_appearance']){
    assert.ok(host.includes(cmd+'('),cmd);
    assert.ok(host.includes(cmd+','),cmd+' Tauri registration');
  }
});

test('UI uses original visible message ID and does not regenerate during Replay',()=>{
  assert.match(desktop,/displayedHistory\.map\(\(entry,i\)/);
  assert.match(desktop,/data-ansf-seq=\{entry\.seq/);
  assert.match(desktop,/copySavedMessage\(entry\)/);
  assert.match(desktop,/setMessageReplay\(\{/);
  assert.match(desktop,/reuseSavedUserPrompt\(entry\)/);
  assert.match(desktop,/quoteSavedMessage\(entry\)/);
  assert.match(desktop,/MESSAGE_REACTIONS\.map\(emoji/);
  assert.match(desktop,/aria-pressed=\{chatReactions\[entry\.seq\]===emoji\}/);
  assert.match(desktop,/lu_message_decorations/);
  assert.match(desktop,/lu_set_message_reaction/);
  assert.match(desktop,/lu_set_chat_appearance/);
  assert.match(desktop,/never reruns the AI, calls tools/);
  assert.match(desktop,/This only populates the editable composer/);
  assert.match(css,/\.messageReactions button\[aria-pressed="true"\]/);
  assert.match(css,/\.messageReplayDialog/);
});

test('central design explicitly records all owner-requested message interactions',()=>{
  assert.match(design,/UX-CHAT01/);
  assert.match(design,/emoji reactions/);
  assert.match(design,/customizable conversation background/);
  assert.match(design,/message-bubble colors/);
  assert.match(design,/Replay means display the exact original again/);
  assert.match(design,/Regeneration\/retry/);
});
