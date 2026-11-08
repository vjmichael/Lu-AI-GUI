import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {planTaskSourceLink,sourceLinkDisplay,
  checkedSourceReobservation} from '../web/task_source_evidence.js';
const P='ide-'+'a'.repeat(32),OTHER='ide-'+'b'.repeat(32);
const REV='a'.repeat(64),NEXT='b'.repeat(64);
const EVENT='evt-'+'c'.repeat(32),OLD='evt-'+'d'.repeat(32);
const tab=(props={})=>({
  key:P+':src/main.rs',projectId:P,relativePath:'src/main.rs',
  revision:REV,savedText:'saved',draft:'saved',dirty:false,
  bufferRevision:0,generation:0,...props,
});
const link=(props={})=>({
  relativePath:'src/main.rs',savedRevision:REV,
  boundUpdateEventId:EVENT,
  source:'native_registered_source_read',observedUnixSeconds:1234,
  ...props,
});
const task=(props={})=>({
  taskId:'C7',updateEventId:EVENT,title:'Review source',
  sourceLinks:[],status:'in_progress',taskAcceptance:'not_verified',
  ...props,
});
test('link request contains only typed project/task/path/revision and event identity',()=>{
  const t=tab();
  const result=planTaskSourceLink([task()], 'C7',t,'saved',P);
  assert.deepEqual(result.request,{
    projectId:P,taskId:'C7',relativePath:'src/main.rs',
    expectedSavedRevision:REV,expectedTaskUpdateEventId:EVENT,
  });
  assert.equal(result.hasUnrecordedDraft,false);
  assert.equal(result.relinking,false);
  assert.equal(JSON.stringify(result).includes('"savedText"'),false);
  assert.equal(JSON.stringify(result).includes('"draft"'),false);
});
test('live editor changes only produce a boolean, never draft content',()=>{
  const t=tab({draft:'secret source draft',dirty:true});
  const result=planTaskSourceLink([task()],'C7',t,'typed after render',P);
  assert.equal(result.hasUnrecordedDraft,true);
  assert.equal(JSON.stringify(result).includes('secret source draft'),false);
  assert.equal(JSON.stringify(result).includes('typed after render'),false);
});
test('project spoof, unsafe relative path and invalid hash rejected',()=>{
  assert.throws(()=>planTaskSourceLink([task()],'C7',tab({projectId:OTHER}),
    'saved',P),/C7_INVALID_SOURCE_SELECTION/);
  assert.throws(()=>planTaskSourceLink([task()],'C7',tab({relativePath:'../escape'}),
    'saved',P),/C7_INVALID_SOURCE_SELECTION/);
  assert.throws(()=>planTaskSourceLink([task()],'C7',tab({revision:'bad'}),
    'saved',P),/C7_INVALID_SOURCE_SELECTION/);
  assert.throws(()=>planTaskSourceLink([task()],'../escape',tab(),
    'saved',P),/C7_INVALID_SOURCE_SELECTION/);
});
test('missing, corrupted or full task scope cannot issue source-link request',()=>{
  assert.throws(()=>planTaskSourceLink([],'C7',tab(),'saved',P),
    /C7_TASK_NOT_AVAILABLE/);
  assert.throws(()=>planTaskSourceLink([task({updateEventId:'bad'})],
    'C7',tab(),'saved',P),/C7_TASK_NOT_AVAILABLE/);
  const full=task({sourceLinks:Array.from({length:12},(_,i)=>link({
    relativePath:'other'+i+'.rs',
  }))});
  assert.throws(()=>planTaskSourceLink([full],'C7',tab(),'saved',P),
    /C7_TASK_NOT_AVAILABLE/);
  const already=task({sourceLinks:[link()]});
  assert.equal(planTaskSourceLink([already],'C7',tab(),'saved',P).relinking,true);
});
test('open tab saved SHA is not misreported as a fresh disk observation',()=>{
  assert.equal(sourceLinkDisplay(link(),task(),tab(),P),
    'open_saved_base_matches_disk_not_rechecked');
  assert.equal(sourceLinkDisplay(link(),task(),null,P),
    'not_open_disk_not_rechecked');
  assert.equal(sourceLinkDisplay(link(),task(),tab({revision:NEXT}),P),
    'open_saved_base_differs');
  assert.equal(sourceLinkDisplay(link(),task({updateEventId:OLD}),tab(),P),
    'historical_task_revision');
});
test('a native recheck distinguishes exact SHA, drift and historical revision',()=>{
  const t=task(),l=link();
  const preview={projectId:P,relativePath:'src/main.rs',revision:REV,text:'verified'};
  const match=checkedSourceReobservation(l,t,P,preview);
  assert.equal(match.matchesRecordedSavedRevision,true);
  assert.equal(match.taskAcceptance,'not_verified');
  assert.equal(checkedSourceReobservation(l,t,P,{...preview,revision:NEXT})
    .matchesRecordedSavedRevision,false);
  assert.equal(checkedSourceReobservation(l,task({updateEventId:OLD}),P,preview)
    .linkedToCurrentTaskUpdate,false);
});
test('foreign native preview, missing data and arbitrary fields are denied',()=>{
  const preview={projectId:P,relativePath:'src/main.rs',revision:REV,text:'file'};
  assert.throws(()=>checkedSourceReobservation(link(),task(),P,
    {...preview,projectId:OTHER}),/C7_INVALID_NATIVE_REOBSERVATION/);
  assert.throws(()=>checkedSourceReobservation(link(),task(),P,
    {...preview,relativePath:'other.rs'}),/C7_INVALID_NATIVE_REOBSERVATION/);
  assert.throws(()=>checkedSourceReobservation(link(),task(),P,
    {...preview,text:null}),/C7_INVALID_NATIVE_REOBSERVATION/);
});
test('native request is explicit, backend verifies source, no CI status supplied',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const host=readFileSync(new URL('../../host/src/task_evidence.rs',import.meta.url),'utf8');
  const tauri=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(ui,/aria-label="Associate source with development task"/);
  assert.match(ui,/aria-label="Link saved source revision to selected task"/);
  assert.match(ui,/invoke\('ide_link_task_source',plan.request\)/);
  assert.match(ui,/invoke\('ide_read_project_context',\{projectId\}\)/);
  assert.match(tauri,/async fn ide_link_task_source\(/);
  assert.match(tauri,/expected_task_update_event_id: String/);
  assert.match(tauri,/ide_link_task_source,/);
  assert.match(host,/registry\.preview_source_file\(project_id, relative_path\)/);
  assert.match(host,/preview\.revision != expected_saved_revision/);
  assert.match(host,/before\.update_event_id != expected_task_update_event_id/);
  const begin=ui.indexOf('async function linkActiveSourceToTask(tab)');
  const end=ui.indexOf('async function recheckLinkedTaskSource(task,link)',begin);
  assert.ok(begin>=0&&end>begin);
  const handler=ui.slice(begin,end);
  assert.doesNotMatch(handler,/ide_save_source_file|ide_observe_task_ci|ide_draft_load/);
  assert.doesNotMatch(handler,/sourceText:|draftText:/);
});
