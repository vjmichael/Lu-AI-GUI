import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { belongsToSelectedProject } from '../web/job_render_guard.js';

test('P04 missing active project never matches absent build/debug job', () => {
  for (const active of [null, undefined, {}, { projectId: '' }]) {
    for (const record of [null, undefined, {}, { projectId: '' }, {projectId:'ide-someone'}]) {
      assert.equal(belongsToSelectedProject(active, record), false);
    }
  }
});
test('P04 actual selected project matches only exact scoped native job', () => {
  assert.equal(belongsToSelectedProject({projectId:'ide-01'}, {projectId:'ide-01'}), true);
  assert.equal(belongsToSelectedProject({projectId:'ide-01'}, {projectId:'ide-02'}), false);
  assert.equal(belongsToSelectedProject({projectId:'ide-01'}, {}), false);
});
test('P04 React protected Job requests view uses proven scoped guards', () => {
  const ui=readFileSync(new URL('../web/main.jsx', import.meta.url),'utf8');
  for (const name of ['buildStatus?.current','buildResult','debugStatus?.current','debugResult']) {
    assert.ok(ui.includes('belongsToSelectedProject(activeProject, '+name+')'), name);
  }
  assert.doesNotMatch(ui,/\{(?:buildStatus\?\.current|debugStatus\?\.current|buildResult|debugResult)\?\.projectId === activeProject\?\.projectId/);
});

test('IDE-05.6 all project-scoped Jobs, Packages and Editor views require a real selected project',()=>{
  const ui=readFileSync(new URL('../web/main.jsx', import.meta.url),'utf8');
  const names=[
    'projectRunProfiles','adapterReview','testDiscovery','nativeJobOutput',
    'debugTargetReview','packageInspection','packagePreview','buildStatus?.current',
    'packageRunResult','diskAuditReport','activeTab','editorTaskCatalog',
    'diskComparison','mergeReview'
  ];
  for(const name of names) assert.ok(
    ui.includes('belongsToSelectedProject(activeProject, '+name+')'),
    'missing identity guard: '+name
  );
  assert.doesNotMatch(ui,/\?\.projectId\s*={2,3}\s*activeProject\?\.projectId/);
  assert.doesNotMatch(ui,/projectRunProfiles\?\.projectId\s*!==\s*activeProject\?\.projectId/);
});
test('IDE-05.6 no-project and cross-project receipts cannot select UI panels',()=>{
  for(const record of [
    null,undefined,{}, {projectId:''}, {projectId:'ide-foreign'},
    {projectId:undefined,profiles:[{id:'rust_build'}]}
  ]) {
    assert.equal(belongsToSelectedProject(null,record),false);
    assert.equal(belongsToSelectedProject({projectId:'ide-current'},record),false);
  }
  assert.equal(belongsToSelectedProject({projectId:'ide-current'},
    {projectId:'ide-current',profiles:[]}),true);
});
