import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  lateSourceOpenDecision,lateReloadCanCommit,MAX_OPEN_TABS_PER_PROJECT,
} from '../web/editor_working_set.js';

const projectA='ide-'+'a'.repeat(32);
const projectB='ide-'+'b'.repeat(32);
const tab=(i,projectId=projectA)=>({
  key:projectId+':src/file'+i+'.rs',
  projectId, relativePath:'src/file'+i+'.rs',
  revision:'old-revision',savedText:'saved',draft:'saved',
  dirty:false,generation:0,bufferRevision:0,
});
const preview=(i,projectId=projectA)=>({
  projectId,relativePath:'src/file'+i+'.rs',
  text:'new source',revision:'new-revision',
});

test('late native preview is denied if another opened tab consumes slot meanwhile',()=>{
  const before=Array.from({length:47},(_,i)=>tab(i));
  const key=tab(48).key;
  assert.equal(lateSourceOpenDecision(before,projectA,'src/file48.rs',
    preview(48),projectA,key).action,'open_new');
  const meanwhile=[...before,tab(47)];
  const late=lateSourceOpenDecision(meanwhile,projectA,'src/file48.rs',
    preview(48),projectA,key);
  assert.equal(MAX_OPEN_TABS_PER_PROJECT,48);
  assert.deepEqual(late,{action:'deny',reason:'OPEN_TAB_BUDGET_EXCEEDED'});
  assert.equal(meanwhile.length,48);
});

test('same path opened concurrently selects existing dirty editor without resetting Undo',()=>{
  const dirty={...tab(7),dirty:true,draft:'unsaved private text',bufferRevision:12};
  const decision=lateSourceOpenDecision([dirty],projectA,dirty.relativePath,
    preview(7),projectA,dirty.key);
  assert.deepEqual(decision,{action:'select_existing',reason:null});
  assert.equal(dirty.draft,'unsaved private text');
  assert.equal(dirty.bufferRevision,12);
});

test('foreign-project, wrong-path and malformed response never mutates source',()=>{
  const key=tab(1).key;
  const args=[[],projectA,'src/file1.rs',preview(1),projectB,key];
  assert.equal(lateSourceOpenDecision(...args).action,'discard_stale');
  assert.equal(lateSourceOpenDecision([],projectA,'src/file1.rs',
    preview(2),projectA,key).action,'discard_stale');
  assert.equal(lateSourceOpenDecision([],projectA,'src/file1.rs',
    {projectId:projectA,relativePath:'src/file1.rs'},projectA,key).action,
    'discard_stale');
});

test('reload invalidates a stale user confirmation after typing, closing or switching',()=>{
  const expected={...tab(4),dirty:true,draft:'original local draft',bufferRevision:2,
    generation:3};
  const loaded=preview(4);
  assert.equal(lateReloadCanCommit(expected,{...expected},loaded,projectA),true);
  assert.equal(lateReloadCanCommit(expected,{...expected,draft:'typed afterward'},
    loaded,projectA),false);
  assert.equal(lateReloadCanCommit(expected,{...expected,bufferRevision:3},
    loaded,projectA),false);
  assert.equal(lateReloadCanCommit(expected,{...expected,generation:4},
    loaded,projectA),false);
  assert.equal(lateReloadCanCommit(expected,null,loaded,projectA),false);
  assert.equal(lateReloadCanCommit(expected,{...expected},loaded,projectB),false);
  assert.equal(lateReloadCanCommit(expected,{...expected},preview(5),projectA),
    false);
});

test('UI native reads use one synchronous ticket and post-await admission',()=>{
  const source=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(source,/const explorerIoTicket = useRef\(null\)/);
  assert.match(source,/function beginExplorerIo\(\)/);
  assert.match(source,/if \(explorerIoTicket\.current\) return null;/);
  assert.match(source,/const decision = lateSourceOpenDecision\(tabsRef\.current/);
  assert.match(source,/lateReloadCanCommit\(tab,latestTab,opened/);
  assert.match(source,/const existing = tabsRef\.current\.find\(tab => tab\.key === key\)/);
  assert.match(source,/finishExplorerIo\(ticket\)/);
  assert.doesNotMatch(source,/if \(tabs\.some\(tab => tab\.key === key\)\)/);
});
