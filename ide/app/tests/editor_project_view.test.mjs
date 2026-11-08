import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {captureProjectView,restoreProjectView} from '../web/editor_project_view.js';

const A='ide-'+'a'.repeat(32);
const B='ide-'+'b'.repeat(32);
const tab=(projectId,i,dirty=false)=>({
  projectId,key:projectId+':src/file'+i+'.rs',relativePath:'src/file'+i+'.rs',
  draft:dirty?'unsaved private text':'file text',
  savedText:'file text',dirty,generation:dirty?9:0,bufferRevision:dirty?27:0,
});
const a=[tab(A,0,true),tab(A,1),tab(A,2)], b=[tab(B,0),tab(B,1,true)];

test('project A restores its exact selected file and independent split-editor pane',()=>{
  const tabs=[...a,...b];
  const saved=captureProjectView(tabs,A,a[1].key,a[0].key,true,0);
  const restored=restoreProjectView(tabs,A,saved);
  assert.deepEqual(restored,{
    primaryKey:a[1].key,secondaryKey:a[0].key,
    splitOpen:true,tabWindowStart:0,
  });
  assert.equal(a[0].draft,'unsaved private text');
  assert.equal(a[0].bufferRevision,27);
  assert.deepEqual(Object.keys(saved).sort(),[
    'primaryKey','projectId','schemaVersion','secondaryKey',
    'splitOpen','tabWindowStart',
  ].sort());
  assert.equal(JSON.stringify(saved).includes('unsaved private text'),false);
});

test('two registered projects keep their own navigation, not each other’s pane',()=>{
  const tabs=[...a,...b];
  const savedA=captureProjectView(tabs,A,a[0].key,a[2].key,true,0);
  const savedB=captureProjectView(tabs,B,b[1].key,b[0].key,true,0);
  const restoredA=restoreProjectView(tabs,A,savedA);
  const restoredB=restoreProjectView(tabs,B,savedB);
  assert.equal(restoredA.primaryKey,a[0].key);
  assert.equal(restoredA.secondaryKey,a[2].key);
  assert.equal(restoredB.primaryKey,b[1].key);
  assert.equal(restoredB.secondaryKey,b[0].key);
  assert.notEqual(restoredA.primaryKey,restoredB.primaryKey);
});

test('closed tab and foreign-project saved view cannot resurrect an editor or file',()=>{
  const saved=captureProjectView([...a,...b],A,a[0].key,a[1].key,true,0);
  const afterClose=[a[2],...b];
  assert.deepEqual(restoreProjectView(afterClose,A,saved),{
    primaryKey:a[2].key,secondaryKey:null,splitOpen:true,tabWindowStart:0,
  });
  const foreign=restoreProjectView([...a,...b],B,saved);
  assert.deepEqual(foreign,{
    primaryKey:b[1].key,secondaryKey:null,splitOpen:false,tabWindowStart:0,
  });
  const none=restoreProjectView(b,A,saved);
  assert.deepEqual(none,{
    primaryKey:null,secondaryKey:null,splitOpen:false,tabWindowStart:0,
  });
});

test('saved 48-tab position stays bounded and includes selected file',()=>{
  const many=Array.from({length:48},(_,i)=>tab(A,i));
  const saved=captureProjectView(many,A,many[35].key,many[12].key,true,30);
  assert.equal(saved.tabWindowStart,30);
  const same=restoreProjectView(many,A,saved);
  assert.equal(same.tabWindowStart,30);
  assert.equal(same.primaryKey,many[35].key);
  const narrowed=many.slice(0,16);
  const dropped=restoreProjectView(narrowed,A,saved);
  assert.equal(dropped.primaryKey,many[15].key);
  assert.equal(dropped.tabWindowStart,4);
  assert.equal(dropped.secondaryKey,many[12].key);
});

test('invalid or empty view is navigation-only and never creates an editor',()=>{
  assert.equal(captureProjectView([],A,'x',null,true,99),null);
  assert.equal(captureProjectView(a,B,a[0].key,null,true,99),null);
  const initial=restoreProjectView(a,A,{schemaVersion:9,projectId:A,
    primaryKey:b[0].key,splitOpen:true});
  assert.equal(initial.primaryKey,a[2].key);
  assert.equal(initial.splitOpen,false);
  assert.equal(initial.secondaryKey,null);
});

test('UI project switching captures before changing project, and restores scoped focus',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(ui,/const projectEditorViews = useRef\(new Map\(\)\)/);
  assert.match(ui,/const previousView = captureProjectView\(tabsRef\.current/);
  assert.match(ui,/restoreProjectView\(tabsRef\.current,/);
  assert.match(ui,/setActiveTabKey\(restoredView\.primaryKey\)/);
  assert.match(ui,/setSecondaryTabKey\(restoredView\.secondaryKey\)/);
  assert.match(ui,/setSplitEditorOpen\(restoredView\.splitOpen\)/);
  assert.match(ui,/projectEditorViews\.current\.delete\(projectId\)/);
  assert.match(ui,/tab\.projectId === activeProject\?\.projectId/);
  assert.doesNotMatch(ui,/setSplitEditorOpen\(false\);\s*setSecondaryTabKey\(null\);\s*setTabWindowStart\(0\);\s*setProjectProblemsOpen/);
});
