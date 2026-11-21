// Deferred test fixtures: intentionally NOT executed until the IDE backlog
// implementation is finished and one consolidated group regression is approved.
import test from 'node:test';
import assert from 'node:assert/strict';
import { planProjectPaneSelection, planProjectSwitchView,
  mayCommitProjectNavigation } from '../web/editor_project_navigation.js';

const tab=(projectId,name)=>({projectId,key:projectId+':'+name,relativePath:name});
const tabs=[tab('project-a','one.rs'),tab('project-a','two.rs'),
  tab('project-b','different.rs')];
const a1=tabs[0].key,a2=tabs[1].key,b1=tabs[2].key;

test('INT01C: a foreign project tab can never become the visible primary pane',()=>{
  const result=planProjectPaneSelection(tabs,'project-a',a1,a2,true,b1);
  assert.equal(result.allowed,false);
  assert.equal(result.reason,'TAB_OUTSIDE_ACTIVE_PROJECT');
});

test('INT01C: selecting secondary swaps pane keys rather than duplicating CodeMirror',()=>{
  const swapped=planProjectPaneSelection(tabs,'project-a',a1,a2,true,a2);
  assert.equal(swapped.allowed,true);
  assert.equal(swapped.primaryKey,a2);
  assert.equal(swapped.secondaryKey,a1);
  assert.notEqual(swapped.primaryKey,swapped.secondaryKey);
  assert.equal(swapped.swapped,true);
});

test('INT01C: changing primary keeps the existing unrelated secondary',()=>{
  const selected=planProjectPaneSelection([
    ...tabs,tab('project-a','third.rs')], 'project-a',a1,a2,true,
    'project-a:third.rs');
  assert.equal(selected.primaryKey,'project-a:third.rs');
  assert.equal(selected.secondaryKey,a2);
});

test('INT01C: restoration only uses tab keys owned by destination project',()=>{
  const requested={schemaVersion:1,projectId:'project-b',
    primaryKey:a1,secondaryKey:a2,splitOpen:true,tabWindowStart:100};
  const planned=planProjectSwitchView(tabs,'project-a','project-b',
    a1,a2,true,0,requested);
  assert.equal(planned.allowed,true);
  assert.equal(planned.leaving.projectId,'project-a');
  assert.equal(planned.primaryKey,b1);
  assert.notEqual(planned.secondaryKey,a2);
});

test('INT01C: source preview from earlier navigation epoch is refused',()=>{
  const request={epoch:7,fromProjectId:'project-a',
    toProjectId:'project-b',relativeDir:'src'};
  const response={projectId:'project-b',relativeDir:'src',entries:[]};
  const context={epoch:7,liveProjectId:'project-a',
    requestedProjectId:'project-b'};
  assert.equal(mayCommitProjectNavigation(request,response,context),true);
  assert.equal(mayCommitProjectNavigation(request,response,
    {...context,epoch:8}),false);
  assert.equal(mayCommitProjectNavigation(request,response,
    {...context,liveProjectId:'project-b'}),false);
  assert.equal(mayCommitProjectNavigation(request,
    {...response,relativeDir:'other'},context),false);
});

test('INT01C: navigation plan leaves original source and draft bytes unmodified',()=>{
  const original=structuredClone(tabs);
  planProjectSwitchView(tabs,'project-a','project-b',a1,a2,true,0,null);
  assert.deepEqual(tabs,original);
});

// Historical DEV-01.1 keyboard contract on the current bounded filename UI.
import { focusQuickOpenResult } from '../web/quick_open_view.js';
import { readFileSync } from 'node:fs';

test('Quick Open keyboard focus is bounded to enabled results, never activates a file',()=>{
  let focused=null;
  const buttons=[0,1,2].map(i=>({focus:()=>{focused=i;},click:()=>assert.fail('unexpected activation')}));
  const root={querySelectorAll(selector){
    assert.equal(selector,'button[data-quick-open-hit]:not(:disabled)');
    return [buttons[0],buttons[2]]; // middle result is disabled
  }};
  assert.equal(focusQuickOpenResult(root,{},'ArrowDown'),true);
  assert.equal(focused,0);
  assert.equal(focusQuickOpenResult(root,buttons[0],'ArrowDown'),true);
  assert.equal(focused,2);
  assert.equal(focusQuickOpenResult(root,buttons[2],'ArrowDown'),true);
  assert.equal(focused,2);
  assert.equal(focusQuickOpenResult(root,buttons[2],'ArrowUp'),true);
  assert.equal(focused,0);
  assert.equal(focusQuickOpenResult(root,{},'ArrowUp'),true);
  assert.equal(focused,2);
  assert.equal(focusQuickOpenResult(root,buttons[2],'Enter'),false);
  assert.equal(focusQuickOpenResult(null,{},'ArrowDown'),false);
  assert.equal(focusQuickOpenResult({querySelectorAll:()=>[]},{},'ArrowDown'),false);
});

test('Quick Open keeps project and pending-state checks and real button activation',()=>{
  const web=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const handler=web.slice(web.indexOf('function navigateQuickOpenResults('),web.indexOf('async function openQuickOpenMatch('));
  assert.match(handler,/quickOpenBusyRef\.current/);
  assert.match(handler,/workspaceTransitionRef\.current/);
  assert.match(handler,/quickOpenResults\.projectId!==activeProjectIdRef\.current/);
  assert.match(handler,/focusQuickOpenResult\(quickOpenResultsRef\.current,event\.target,event\.key\)/);
  assert.match(web,/data-quick-open-hit/);
  assert.match(web,/onClick=\{\(\)=>void openQuickOpenMatch\(hit\)\}/);
  assert.match(web,/clearQuickOpenResults\(\);\s*quickOpenInputRef\.current\?\.focus\(\)/);
});
