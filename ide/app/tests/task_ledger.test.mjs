import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildDevelopmentCheckpoint } from '../web/development_checkpoint.js';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('native C6 exposes append-only task journal and host-only CI collection', () => {
  const host = read('../../host/src/task_evidence.rs');
  const journal = read('../../host/src/continuity.rs');
  const tauri = read('../src-tauri/src/lib.rs');
  assert.match(host, /append_transaction\(/);
  assert.match(host, /record_task_update\(/);
  assert.match(host, /record_task_ci\(/);
  assert.match(host, /registry\.inspect_exact_head_ci\(project_id\)/);
  assert.match(host, /update_event_id/);
  assert.match(host, /"ready_for_review"/);
  assert.doesNotMatch(host, /"completed"\s*,\s*"accepted"/);
  assert.match(journal, /pub task: Option<crate::task_evidence::TaskEvent>/);
  assert.match(journal, /pub tasks: Vec<crate::task_evidence::TaskSummary>/);
  assert.match(tauri, /async fn ide_record_task_update/);
  assert.match(tauri, /async fn ide_observe_task_ci/);
});

test('C6 catalog adds only typed bounded task operations, with no user-submitted CI verdict', () => {
  const schema = JSON.parse(read('../../host/src/capability_schemas.json'));
  assert.equal(schema.operations['task.update'].request.properties.status.enum.length, 4);
  assert.equal(schema.operations['task.update'].request.properties.requiredGates.maxItems, 4);
  assert.deepEqual(schema.operations['task.ciObserve'].request.required,
    ['projectId', 'taskId']);
  assert.equal(schema.definitions.ProjectContext.properties.tasks.maxItems,64);
  assert.equal(schema.definitions.TaskSummary.properties.taskAcceptance.const, 'not_verified');
  assert.equal(schema.definitions.DevelopmentEvent.required.includes('task'),false);
});

test('C6 human interface does not conflate task progress with installation or approval', () => {
  const ui = read('../web/main.jsx');
  assert.match(ui, /aria-label="Development task ledger"/);
  assert.match(ui, /invoke\('ide_record_task_update'/);
  assert.match(ui, /invoke\('ide_observe_task_ci'/);
  assert.match(ui, /Check & record task CI/);
  assert.match(ui, /Task acceptance: <strong>Not verified<\/strong>/);
  assert.match(ui, /task\.linkedCommit !== projectContext\?\.gitWorktree\.localHead/);
});

test('checkpoint exports bounded task evidence coordinates, not private task text', () => {
  const projectId='p1';
  const context={
    projectId,projectName:'project',
    gitWorktree:{state:'local_repository_inspected',branch:'b',localHead:'a'.repeat(40),
      upstreamHead:null,displayedChanges:[],worktree:'dirty',
      modifiedOrStagedCount:1,untrackedCount:0},
    inventory:{files:[],discoveryComplete:true},
    events:[],documents:[],
    tasks:[{taskId:'C6',title:'PRIVATE TITLE',nextAction:'PRIVATE NEXT ACTION',
      status:'blocked',linkedCommit:'a'.repeat(40),linkedWorktree:'dirty',
      ciLinkState:'not_observed',requiredGates:['exact_head_ci'],
      acceptanceGates:[{gate:'exact_head_ci',state:'not_observed'}],
      taskAcceptance:'not_verified',observedCi:null}],
  };
  const comparison={projectId,baselinePresent:false,entries:[],discoveryComplete:true};
  const copied=buildDevelopmentCheckpoint(projectId,context,comparison,
    '2026-11-05T19:00:00Z');
  assert.equal(copied.developmentTasks.exported[0].taskId,'C6');
  assert.equal(copied.developmentTasks.exported[0].taskAcceptance,'not_verified');
  assert.doesNotMatch(JSON.stringify(copied),/PRIVATE TITLE|PRIVATE NEXT ACTION/);
});
