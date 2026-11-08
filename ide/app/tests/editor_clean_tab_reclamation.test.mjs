import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { planCleanBackgroundTabClosure } from '../web/editor_clean_tab_reclamation.js';

const A='ide-'+'a'.repeat(32), B='ide-'+'b'.repeat(32);
const tab=(p,i,overrides={})=>({
  projectId:p,key:p+':src/file'+i+'.rs',relativePath:'src/file'+i+'.rs',
  draft:'saved code',savedText:'saved code',dirty:false,generation:0,
  bufferRevision:0,...overrides,
});
function plan(tabs,protect=[],actual=new Map(tabs.map(t=>[t.key,t.draft]))) {
  return planCleanBackgroundTabClosure(tabs,A,new Set(protect),
    key=>actual.get(key));
}

test('closes only clean backgrounds and keeps both visible panes',()=>{
  const items=[tab(A,0),tab(A,1),tab(A,2),tab(A,3)];
  const result=plan(items,[items[0].key,items[1].key]);
  assert.deepEqual(result.closeKeys,[items[2].key,items[3].key]);
  assert.deepEqual(result.retainedKeys,[items[0].key,items[1].key]);
  assert.equal(result.reason,null);
});

test('never closes a dirty editor, even when React tab text claims clean',()=>{
  const dirty=tab(A,1,{dirty:true,draft:'unsaved text'});
  const stale=tab(A,2,{dirty:false,draft:'saved code'});
  const items=[tab(A,0),dirty,stale];
  const live=new Map(items.map(t=>[t.key,t.draft]));
  live.set(stale.key,'typed after last React render');
  const result=plan(items,[items[0].key],live);
  assert.deepEqual(result.closeKeys,[]);
  assert.deepEqual(result.retainedKeys,items.map(t=>t.key));
});

test('misleading dirty flag cannot override divergent React draft',()=>{
  const t=tab(A,3,{dirty:false,draft:'unsaved newer version'});
  assert.deepEqual(plan([t]).closeKeys,[]);
});

test('Undo-back-to-clean and Save do not discard existing edit history',()=>{
  // The displayed document may equal saved source even after many edits.
  // Closing its tab would also discard the per-tab CodeMirror Undo stack.
  const afterUndo=tab(A,4,{dirty:false,bufferRevision:12});
  const afterSave=tab(A,5,{dirty:false,bufferRevision:3});
  const result=plan([afterUndo,afterSave]);
  assert.deepEqual(result.closeKeys,[]);
  assert.deepEqual(result.retainedKeys,[afterUndo.key,afterSave.key]);
});

test('missing or inaccessible CodeMirror state fails closed',()=>{
  const clean=tab(A,0);
  assert.deepEqual(plan([clean],[],new Map()).closeKeys,[]);
  assert.deepEqual(planCleanBackgroundTabClosure([clean],A,new Set(),
    ()=>{throw Error('state inaccessible');}).closeKeys,[]);
});

test('close plan never touches another project or revives tabs',()=>{
  const a=tab(A,0), b=tab(B,0);
  const result=plan([a,b]);
  assert.deepEqual(result.closeKeys,[a.key]);
  assert.equal(result.closeKeys.includes(b.key),false);
  assert.equal(JSON.stringify(result).includes('saved code'),false);
});

test('bounded 48-tab plan works without silent eviction of dirty/private buffers',()=>{
  const items=Array.from({length:48},(_,i)=>tab(A,i));
  items[11]={...items[11],dirty:true,draft:'private'};
  const protect=[items[0].key,items[47].key,items[9].key];
  const result=plan(items,protect);
  assert.equal(result.closeKeys.length,44);
  assert.equal(result.closeKeys.includes(items[11].key),false);
  assert.equal(result.closeKeys.includes(items[9].key),false);
  assert.equal(result.closeKeys.includes(items[47].key),false);
});

test('malformed context returns deny instead of deciding to close',()=>{
  const t=tab(A,0);
  assert.equal(planCleanBackgroundTabClosure([t],A,[],()=>t.draft).reason,
    'INVALID_TAB_CLEANUP_CONTEXT');
  assert.deepEqual(planCleanBackgroundTabClosure([t],A,new Set(),null).closeKeys,[]);
  assert.deepEqual(planCleanBackgroundTabClosure([],A,new Set(),()=>'' ).closeKeys,[]);
});

test('UI explicitly opts in and snapshots actual CodeMirror state before closure',()=>{
  const source=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(source,/aria-label="Close clean background tabs"/);
  assert.match(source,/onClick=\{closeCleanBackgroundTabs\}/);
  assert.match(source,/planCleanBackgroundTabClosure\(tabsRef\.current,projectId,/);
  assert.match(source,/key=>editorStates\.current\.get\(key\)\?\.sliceDoc\(\)/);
  assert.match(source,/for \(const key of draftInFlight\.current\)/);
  assert.match(source,/for \(const key of draftTimers\.current\.keys\(\)\)/);
  assert.match(source,/setTabs\(previous=>previous\.filter\(tab=>!closing\.has\(tab\.key\)\)\)/);
  const beginning=source.indexOf('function closeCleanBackgroundTabs()');
  const end=source.indexOf('function closeTab(key)',beginning);
  assert.ok(beginning>=0 && end>beginning);
  const handler=source.slice(beginning,end);
  assert.doesNotMatch(handler,/invoke\(|ide_save_source_file|await /);
});
