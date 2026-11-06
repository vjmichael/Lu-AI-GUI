import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { reconcileDevelopmentEvidence } from '../web/development_reconciliation.js';
import { buildDevelopmentCheckpoint } from '../web/development_checkpoint.js';

function fixture() {
  const projectId = 'test-project';
  return {
    projectId,
    context: {
      projectId, projectName: 'Test',
      gitWorktree: {state: 'local_repository_inspected', worktree: 'clean',
        modifiedOrStagedCount: 0, untrackedCount: 0, truncated: false,
        localHead: 'a'.repeat(40), branch: 'feature',
        upstreamHead: null, upstreamComparison: 'not_verified',
        commitsAhead: null, commitsBehind: null, displayedChanges: []},
      inventory: {discoveryComplete: true, files: [{
        relativePath: 'Delivered/tool.exe', category: 'tool',
        state: 'present_unverified', sizeBytes: 10,
        sha256: 'b'.repeat(64), digestStatus: 'sha256_observed',
      }]},
      documents: [{relativePath: 'PROJECT_STATE.md', excerpt: 'SECRET COMPLETED CLAIM',
        availability: 'present', sourceRevision: 'c'.repeat(64)}],
      events: [{eventId: 'ev1', operation: 'source_save', outcome: 'success',
        observedUnixSeconds: 20, summary: 'SECRET PRIVATE EVENT',
        sourcePath: 'src/main.rs', sourceRevision: 'd'.repeat(64)}],
    },
    comparison: {projectId, baselinePresent: true, baselineCount: 1,
      baselineUnixSeconds: 10, discoveryComplete: true,
      entries: [{relativePath: 'Delivered/tool.exe', status: 'matched_bytes',
        baselineSha256: 'b'.repeat(64), currentSha256: 'b'.repeat(64)}],
      localGitHeadChange: 'same_observed_head',
      gitWorktreeChange: 'remained_clean', liveRemote: 'not_queried'},
  };
}
const state = (matrix, id) => matrix.checks.find(x => x.id === id).state;

test('clean Git and matched artifact bytes never imply passing CI or usable install', () => {
  const {projectId, context, comparison} = fixture();
  const matrix = reconcileDevelopmentEvidence(projectId, context, comparison);
  assert.equal(state(matrix, 'local_source'), 'observed_clean');
  assert.equal(state(matrix, 'artifact_baseline'), 'matching_observations');
  assert.equal(state(matrix, 'native_ci'), 'not_inspected');
  assert.equal(state(matrix, 'installed_app'), 'not_inspected');
  assert.equal(state(matrix, 'toolchain'), 'not_tested');
  assert.equal(state(matrix, 'live_remote'), 'not_checked');
  assert.equal(matrix.authority, 'informational_only_no_permission_grant');
  assert.doesNotMatch(JSON.stringify(matrix), /SECRET/);
});

test('dirty Git, changed bytes and truncated source list remain distinct evidence', () => {
  const {projectId, context, comparison} = fixture();
  Object.assign(context.gitWorktree, {worktree: 'dirty', untrackedCount: 1, truncated: true});
  comparison.entries[0].status = 'changed_bytes';
  const matrix = reconcileDevelopmentEvidence(projectId, context, comparison);
  assert.equal(state(matrix, 'local_source'), 'observed_dirty');
  assert.match(matrix.checks[0].detail, /truncated/);
  assert.equal(state(matrix, 'artifact_baseline'), 'observed_differences');
});

test('bounded incomplete scans and unverifiable historical comparisons are not called matching', () => {
  const {projectId, context, comparison} = fixture();
  context.inventory.discoveryComplete = false;
  comparison.discoveryComplete = false;
  comparison.entries[0].status = 'unverifiable';
  const matrix = reconcileDevelopmentEvidence(projectId, context, comparison);
  assert.equal(state(matrix, 'artifact_bytes'), 'partial_scan');
  assert.equal(state(matrix, 'artifact_baseline'), 'partial_comparison');
});

test('contradictory local counts and missing baseline are not incorrectly marked clean', () => {
  const {projectId, context, comparison} = fixture();
  context.gitWorktree.modifiedOrStagedCount = 2;
  comparison.baselinePresent = false;
  const matrix = reconcileDevelopmentEvidence(projectId, context, comparison);
  assert.equal(state(matrix, 'local_source'), 'unverified');
  assert.equal(state(matrix, 'artifact_baseline'), 'not_recorded');
});

test('mixing projects or incomplete native evidence fails closed', () => {
  const {projectId, context, comparison} = fixture();
  assert.throws(() => reconcileDevelopmentEvidence('other', context, comparison), /MISMATCH/);
  assert.throws(() => reconcileDevelopmentEvidence(projectId, context, {...comparison,projectId:'other'}), /MISMATCH/);
  assert.throws(() => reconcileDevelopmentEvidence(projectId, {...context,inventory:null}, comparison), /MISMATCH/);
});

test('portable checkpoint carries conservative reconciliation without source or notes', () => {
  const {projectId, context, comparison} = fixture();
  const exportData = buildDevelopmentCheckpoint(projectId, context, comparison, '2026-11-05T18:00:00Z');
  assert.equal(exportData.evidenceReconciliation.checks.length, 10);
  assert.equal(state(exportData.evidenceReconciliation, 'native_ci'), 'not_inspected');
  assert.doesNotMatch(JSON.stringify(exportData), /SECRET/);
});

test('visible human reconciliation is read-only and uses existing native data', () => {
  const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
  assert.match(ui, /aria-label="Development evidence reconciliation"/);
  assert.match(ui, /reconcileDevelopmentEvidence\(/);
  assert.match(ui, /evidenceReconciliation\.checks\.map/);
  assert.doesNotMatch(ui, /invoke\('ide_execute_|invoke\('ide_git_push'/);
});
