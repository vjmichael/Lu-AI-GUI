import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {MAX_HISTORY_ROWS,mergeHistoryWindow,captureHistoryAnchor,restoreHistoryAnchor}
  from '../web/history_window_geometry.mjs';
const row=seq=>({seq,role:seq%2?'user':'assistant',text:'Immutable '+seq});
const range=(a,b)=>Array.from({length:b-a+1},(_,i)=>row(a+i));
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
test('older and newer 6-month-style windows evict only the opposite end and never duplicate stable source rows',()=>{
  let window=range(9901,10000);
  for(let high=9900;high>=1;high-=48){
    const incoming=range(Math.max(1,high-47),high);
    const merged=mergeHistoryWindow(window,incoming,'older');
    window=merged.rows;
    assert.ok(window.length<=MAX_HISTORY_ROWS);
    assert.equal(new Set(window.map(v=>v.seq)).size,window.length);
    assert.ok(window.every((v,i)=>i===0||window[i-1].seq<v.seq));
  }
  const forward=mergeHistoryWindow(window,range(9900,10000),'newer');
  assert.ok(forward.rows.length<=MAX_HISTORY_ROWS);
  assert.equal(forward.rows.at(-1).seq,10000);
});
test('different assistant progress snapshots are represented by one sequence only',()=>{
  const seq=21;
  const merged=mergeHistoryWindow([row(seq),row(22)],[{...row(seq),text:'Newest immutable original checkpoint'}],'older');
  assert.equal(merged.rows.length,2);
  assert.equal(merged.rows[0].text,'Newest immutable original checkpoint');
});
test('invalid and unbounded pages fail before rendering',()=>{
  assert.throws(()=>mergeHistoryWindow([], [{seq:4,role:'tool'}],'older'));
  assert.throws(()=>mergeHistoryWindow([], [row(-1)],'older'));
  assert.throws(()=>mergeHistoryWindow([], [],'older',99999));
});
test('pixel-stable source anchor survives variable message heights and far-end eviction',()=>{
  const viewport={scrollTop:160,clientHeight:100,items:[
    {seq:1,top:0,height:70},{seq:2,top:70,height:250},{seq:3,top:320,height:60}
  ]};
  const el={
    get scrollTop(){return viewport.scrollTop},
    set scrollTop(n){viewport.scrollTop=n},
    getBoundingClientRect(){return {top:0,bottom:viewport.clientHeight}},
    querySelectorAll(){return viewport.items.map(item=>({
      dataset:{ansfSeq:String(item.seq)},
      getBoundingClientRect:()=>({top:item.top-viewport.scrollTop,bottom:item.top+item.height-viewport.scrollTop})
    }))},
    querySelector(selector){const seq=Number(selector.match(/\d+/)?.[0]);return this.querySelectorAll().find(e=>Number(e.dataset.ansfSeq)===seq)}
  };
  const anchor=captureHistoryAnchor(el);
  assert.equal(anchor.seq,2);
  assert.equal(anchor.offset,-90);
  viewport.items=[{seq:0,top:0,height:500},{seq:1,top:500,height:70},{seq:2,top:570,height:250}];
  assert.equal(restoreHistoryAnchor(el,anchor),true);
  assert.equal(viewport.scrollTop,660);
});
test('UI restores source-ID location after chat switching without eagerly fetching the full transcript',()=>{
  assert.match(ui,/loadRememberedUiHistory\(id\)/);
  assert.match(ui,/captureHistoryAnchor\(el\)/);
  assert.match(ui,/restoreHistoryAnchor\(el,stored.anchor\)/);
  assert.match(ui,/pendingPrepend\.current=\{chatId:id,anchor\}/);
  assert.match(ui,/data-ansf-seq=\{entry.seq\?\?undefined\}/);
  assert.match(ui,/MAX_VISIBLE_CHAT_ROWS=MAX_HISTORY_ROWS/);
  assert.doesNotMatch(ui,/invoke\('lu_history'/);
});

test('paged historical rows are not repeatedly spoken as fresh live model output',()=>{
  assert.match(ui,/role="log" aria-live="off" aria-label="Conversation history, cursor paginated"/);
  assert.match(ui,/className="message assistant streaming"><span className="messageRole" role="status" aria-live="polite"/);
  assert.match(ui,/historyWindow\.hasOlder/);
  assert.match(ui,/historyWindow\.hasNewer/);
});
