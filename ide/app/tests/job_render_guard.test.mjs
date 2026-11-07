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
