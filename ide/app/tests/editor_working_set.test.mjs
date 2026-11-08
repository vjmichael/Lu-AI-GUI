import test from 'node:test';
import assert from 'node:assert/strict';
import {performance} from 'node:perf_hooks';
import {
 MAX_OPEN_TABS_PER_PROJECT,VISIBLE_TABS,
 admitOpenTabs,tabWindow,pageTabWindow,centerSelectedTab,
 visibleTreeEntries,INITIAL_VISIBLE_TREE_ENTRIES,TREE_ENTRY_PAGE
} from '../web/editor_working_set.js';
const A='ide-'+'a'.repeat(32),B='ide-'+'b'.repeat(32);
const tab=(i,projectId=A)=>({key:projectId+':src/f'+i+'.rs',
 projectId,relativePath:'src/f'+i+'.rs',dirty:i%2===0});
const populated=Array.from({length:48},(_,i)=>tab(i));
test('48 current project tabs preserve all draft identities and refuse hidden 49th',()=>{
 assert.equal(MAX_OPEN_TABS_PER_PROJECT,48);
 const full=admitOpenTabs(populated,A,[tab(48).key]);
 assert.equal(full.allowed,false);
 assert.equal(full.reason,'OPEN_TAB_BUDGET_EXCEEDED');
 assert.equal(full.remaining,0);
 assert.equal(admitOpenTabs(populated,A,[tab(0).key]).allowed,true);
 assert.equal(admitOpenTabs(populated,B,[tab(0,B).key]).allowed,true);
 assert.equal(populated.filter(x=>x.dirty).length,24);
 assert.equal(populated.length,48,'no eviction from limit decision');
});
test('proposal batch admission is atomic and deduplicates existing files',()=>{
 const first=populated.slice(0,46);
 assert.equal(admitOpenTabs(first,A,[tab(46).key,tab(47).key]).allowed,true);
 assert.equal(admitOpenTabs(first,A,[tab(46).key,tab(47).key,tab(48).key]).allowed,false);
 assert.equal(admitOpenTabs(first,A,[tab(46).key,tab(46).key]).required,1);
 assert.equal(admitOpenTabs(first,'',[]).allowed,false);
});
test('virtual tab strip is bounded and selects every open file without duplicating editor states',()=>{
 assert.equal(VISIBLE_TABS,12);
 let offset=0,rendered=[];
 for(let page=0;page<4;page++){
   const window=tabWindow(populated,offset);
   assert.ok(window.visible.length<=VISIBLE_TABS);
   assert.equal(window.count,48);
   rendered.push(...window.visible.map(x=>x.key));
   offset=pageTabWindow(populated,offset,1);
 }
 assert.equal(new Set(rendered).size,48);
 const last=tabWindow(populated,centerSelectedTab(populated,tab(47).key,0));
 assert.ok(last.visible.some(x=>x.key===tab(47).key));
 assert.equal(last.hiddenAfter,0);
 assert.equal(tabWindow([],999).visible.length,0);
 assert.equal(tabWindow(populated,-1).start,0);
});
test('native directory listing renders in bounded incremental slices, no content mutation',()=>{
 const entries=Array.from({length:400},(_,i)=>({name:'source-'+i}));
 let count=INITIAL_VISIBLE_TREE_ENTRIES,n=0;
 while(true){
   const rendered=visibleTreeEntries(entries,count);
   assert.ok(rendered.visible.length<=400);
   assert.equal(rendered.total,400);
   if(!rendered.remaining)break;
   count+=TREE_ENTRY_PAGE;n++;
 }
 assert.equal(n,3);
 assert.equal(entries.length,400);
 assert.equal(visibleTreeEntries(entries,0).visible.length,0);
});
// A reproducible JavaScript-only stress probe, NOT installed Tauri measurement.
// Time is diagnostic; no arbitrary CI speed threshold or false GUI acceptance.
test('B5 10k-tab synthetic strip benchmark remains bounded, reports actual time',()=>{
 const huge=Array.from({length:10000},(_,i)=>tab(i));
 const now=performance.now();
 let rows=0;
 for(let i=0;i<1200;i++){
   const next=tabWindow(huge,(i*13)%10000);
   rows+=next.visible.length;
 }
 const elapsedMs=performance.now()-now;
 assert.equal(rows,1200*VISIBLE_TABS);
 console.log('B5_SYNTHETIC_10000_TAB_WINDOW_1200_RENDER_SELECTIONS_MS='+elapsedMs.toFixed(3));
 console.log('B5_GENUINE_INSTALLED_EDITOR_MEMORY_LATENCY_ACCEPTANCE=OPEN');
});
