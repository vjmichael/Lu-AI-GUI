import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  nearTranscriptBottom, restoredScrollTop, isSendKey, isSearchKey,
  isNewChatKey, matchesChatTitle, SCROLL_BOTTOM_THRESHOLD
} from '../web/chat_ui_behavior.mjs';

const source=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');

test('Jan-style follow follows only when reader is near bottom',()=>{
  assert.equal(SCROLL_BOTTOM_THRESHOLD,48);
  assert.equal(nearTranscriptBottom({scrollTop:900,scrollHeight:1200,clientHeight:280}),true);
  assert.equal(nearTranscriptBottom({scrollTop:700,scrollHeight:1200,clientHeight:280}),false);
  assert.equal(nearTranscriptBottom({scrollTop:872,scrollHeight:1200,clientHeight:280}),true);
  assert.equal(nearTranscriptBottom({scrollTop:871,scrollHeight:1200,clientHeight:280}),false);
});

test('conversation scroll positions restore without overshooting shorter history',()=>{
  assert.equal(restoredScrollTop({top:400,follow:false},1000,300),400);
  assert.equal(restoredScrollTop({top:900,follow:false},1000,300),700);
  assert.equal(restoredScrollTop({top:100,follow:true},1000,300),700);
  assert.equal(restoredScrollTop(null,100,300),0);
});

test('search only filters chat titles, without modifying stored conversations',()=>{
  const docs=[{title:'Climate in Yukon'},{title:'Lu build notes'},{title:'ARKANSAS costs'}];
  assert.deepEqual(docs.filter(c=>matchesChatTitle(c,'yukon')).map(c=>c.title),['Climate in Yukon']);
  assert.deepEqual(docs.filter(c=>matchesChatTitle(c,'  arK  ')).map(c=>c.title),['ARKANSAS costs']);
  assert.equal(matchesChatTitle({title:''},''),true);
});

test('Enter sends, Shift Enter is a newline; IME Enter never submits',()=>{
  assert.equal(isSendKey({key:'Enter',shiftKey:false,isComposing:false}),true);
  assert.equal(isSendKey({key:'Enter',shiftKey:true,isComposing:false}),false);
  assert.equal(isSendKey({key:'Enter',isComposing:true}),false);
  assert.equal(isSendKey({key:'Enter',nativeEvent:{isComposing:true}}),false);
  assert.equal(isSendKey({key:'Enter',keyCode:229}),false);
  assert.equal(isSendKey({key:'a'}),false);
});

test('Ctrl/Cmd K search and Ctrl/Cmd Shift N new chat are distinct',()=>{
  assert.equal(isSearchKey({key:'k',ctrlKey:true}),true);
  assert.equal(isSearchKey({key:'K',metaKey:true,shiftKey:false}),true);
  assert.equal(isSearchKey({key:'k',ctrlKey:true,shiftKey:true}),false);
  assert.equal(isNewChatKey({key:'N',metaKey:true,shiftKey:true}),true);
  assert.equal(isNewChatKey({key:'n',ctrlKey:true,shiftKey:false}),false);
});

test('scrolling a transcript up suppresses forced jumping until user opts in',()=>{
  assert.ok(source.includes('if (!el || !followOutput.current) return;'));
  assert.ok(source.includes('scrollByChat.current.set(currentChat.current'));
  assert.ok(source.includes('restoredScrollTop(stored, el.scrollHeight, el.clientHeight)'));
  assert.ok(!source.includes('scrollIntoView'));
  assert.ok(source.includes('aria-label="Jump to latest message"'));
  assert.ok(css.includes('.transcriptWrap .jumpLatest{position:absolute'));
});

test('search and key handling remain accessible',()=>{
  assert.ok(source.includes('aria-label="Search conversations"'));
  assert.ok(source.includes('matchingChats.map(renderChat)'));
  assert.ok(source.includes('composerRef.current?.focus()'));
  assert.ok(source.includes('role="separator"'));
  assert.ok(source.includes('aria-valuenow={composerHeight}'));
  assert.ok(source.includes('if (isSendKey(e))'));
  assert.ok(css.includes('button:focus-visible'));
});

test('model stays explicit-load and runtime remains Lu-native',()=>{
  assert.ok(source.includes("invoke('lu_load_model'"));
  assert.ok(source.includes('Selection does not load'));
  assert.ok(!source.includes('JanInference'));
});
