import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildDevelopmentCheckpoint } from '../web/development_checkpoint.js';

function fixture() {
  return {
    context: {
      projectId: 'id-1', projectName: 'Sample',
      gitWorktree: {
        state: 'local_repository_inspected', branch: 'feature', localHead: 'a'.repeat(40),
        upstreamHead: null, upstreamComparison: 'not_verified', commitsAhead: null,
        commitsBehind: null, worktree: 'dirty', modifiedOrStagedCount: 1,
        untrackedCount: 2, displayedChanges: [{relativePath: 'src/file.rs', kind:'modified',stage:'worktree'}], truncated: false,
      },
      inventory: {discoveryComplete: false, files: [
        {relativePath: 'Delivered/app.exe', category: 'installer', sizeBytes: 200, sha256: 'b'.repeat(64), state: 'present_unverified', digestStatus:'sha256_observed'},
      ]},
      documents: [{relativePath:'PROJECT_STATE.md', availability:'present', sourceRevision:'c'.repeat(64), excerpt:'SECRET PRIVATE SOURCE TEXT'}],
      events: [{eventId:'ev1', observedUnixSeconds: 123, operation:'source_save', outcome:'success', sourcePath:'src/file.rs', sourceRevision:'d'.repeat(64), summary:'SECRET EVENT SUMMARY'}],
    },
    compare: {projectId:'id-1', baselinePresent:false, baselineCount:0, discoveryComplete:false, entries:[], localGitHeadChange:'not_compared', gitWorktreeChange:'not_compared'},
  };
}

test('checkpoint preserves evidence distinctions but excludes document content and private host roots', () => {
  const {context,compare} = fixture();
  const result = buildDevelopmentCheckpoint('id-1', context, compare, '2026-11-05T18:00:00Z');
  const output = JSON.stringify(result);
  assert.equal(result.git.liveRemote, 'not_queried_for_this_checkpoint');
  assert.equal(result.resources.artifactReadiness, 'not_tested');
  assert.equal(result.resources.discoveryComplete, false);
  assert.equal(result.resources.recordedBaseline.present, false);
  assert.equal(result.git.changedPaths.exported[0].path, 'src/file.rs');
  assert.equal(result.orientationDocuments.exported[0].excerptExcluded, true);
  assert.doesNotMatch(output, /SECRET|projectRoot|absolutePath/);
});

test('mixing project evidence or missing host observations fails closed', () => {
  const {context,compare} = fixture();
  assert.throws(() => buildDevelopmentCheckpoint('id-2',context,compare,'2026-11-05T18:00:00Z'), /MISMATCH/);
  assert.throws(() => buildDevelopmentCheckpoint('id-1',context,{...compare,projectId:'other'},'2026-11-05T18:00:00Z'), /MISMATCH/);
  assert.throws(() => buildDevelopmentCheckpoint('id-1',{},compare,'2026-11-05T18:00:00Z'), /MISMATCH/);
});

test('bounded export labels omitted evidence and rejects invalid paths', () => {
  const {context,compare} = fixture();
  context.inventory.files = Array.from({length: 35}, (_,i)=>({
    relativePath:i === 0 ? '/home/private/credentials' : 'bin/' + i, category:'unknown',
    sizeBytes: 3, sha256:null, state:'present_unverified',digestStatus:'not_computed',
  }));
  const result = buildDevelopmentCheckpoint('id-1',context,compare,'2026-11-05T18:00:00Z');
  assert.equal(result.resources.files.totalObserved,35);
  assert.equal(result.resources.files.exported.length,24);
  assert.equal(result.resources.files.omittedFromExport,11);
  assert.equal(result.resources.files.exported[0].path,null);
});

test('human copy workflow validates current project and needs an explicit clipboard click', () => {
  const ui = readFileSync(new URL('../web/main.jsx', import.meta.url), 'utf8');
  assert.match(ui, /Copy development checkpoint/);
  assert.match(ui, /activeProjectIdRef\.current !== projectId/);
  assert.match(ui, /buildDevelopmentCheckpoint\(projectId, context, comparison/);
  assert.match(ui, /navigator\.clipboard\.writeText/);
  assert.doesNotMatch(ui, /invoke\('ide_execute_|invoke\('ide_git_push'/);
});
