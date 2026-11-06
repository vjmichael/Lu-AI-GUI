import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { reconcileDevelopmentEvidence } from '../web/development_reconciliation.js';
import { buildDevelopmentCheckpoint } from '../web/development_checkpoint.js';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const projectId = 'p1';
function fixture() {
  return {
    context: {
      projectId, projectName: 'Fixture',
      gitWorktree: {state: 'local_repository_inspected', branch: 'feature',
        localHead: 'a'.repeat(40), upstreamHead: null, worktree: 'dirty',
        modifiedOrStagedCount: 1, untrackedCount: 0, displayedChanges: [], truncated: false},
      inventory: {discoveryComplete: true, files: []},
      events: [], documents: [],
    },
    comparison: {
      projectId, baselinePresent: false, baselineCount: 0,
      discoveryComplete: true, entries: [], baselineUnixSeconds: null,
    },
  };
}
function receipt() {
  return {
    projectId, source: 'public_github_actions_explicit_read',
    localHead: 'a'.repeat(40), checkedUnixSeconds: 1793920000,
    credentialsUsed: false, localHeadChangedDuringCheck: false,
    verdict: 'exact_commit_ci_passed',
    workflows: [
      {workflow:'standalone_ide',runId:123,verdict:'passed_exact_head'},
      {workflow:'continuity',runId:124,verdict:'passed_exact_head'},
    ],
  };
}
const ci = matrix => matrix.checks.find(x => x.id === 'native_ci');
test('verified exact-commit CI does not declare local dirty worktree or Windows install accepted', () => {
  const {context, comparison} = fixture();
  const result = reconcileDevelopmentEvidence(projectId, context, comparison, receipt());
  assert.equal(ci(result).state, 'exact_commit_ci_passed');
  assert.match(ci(result).detail, /drafts and installation are NOT covered/);
  assert.equal(result.checks.find(x => x.id === 'installed_app').state, 'not_inspected');
});
test('stale SHA, project switch or missing receipt never turns into successful CI evidence', () => {
  const {context, comparison} = fixture();
  for (const wrong of [
    {...receipt(),localHead:'b'.repeat(40)},
    {...receipt(),projectId:'p2'},
    {...receipt(),localHeadChangedDuringCheck:true},
    {...receipt(),checkedUnixSeconds:null},
  ]) {
    assert.equal(ci(reconcileDevelopmentEvidence(projectId,context,comparison,wrong)).state,
      'stale_or_unrelated');
  }
  assert.equal(ci(reconcileDevelopmentEvidence(projectId,context,comparison)).state,'not_inspected');
});
test('checkpoint copies only cached checked receipt and never initiates GitHub queries', () => {
  const {context, comparison} = fixture();
  const fresh = buildDevelopmentCheckpoint(projectId,context,comparison,
    '2026-11-05T18:00:00Z',receipt());
  assert.equal(fresh.priorExplicitCi.commitSha,'a'.repeat(40));
  assert.equal(fresh.priorExplicitCi.workflows.length,2);
  assert.equal(fresh.priorExplicitCi.installationAcceptance,'not_tested');
  const stale = buildDevelopmentCheckpoint(projectId,context,comparison,
    '2026-11-05T18:00:00Z',{...receipt(),localHead:'b'.repeat(40)});
  assert.equal(stale.priorExplicitCi,null);
});
test('typed native CI operation uses bounded explicit public GitHub Actions with no execution', () => {
  const schema=JSON.parse(read('../../host/src/capability_schemas.json'));
  const host=read('../../host/src/github_ci.rs');
  const ui=read('../web/main.jsx');
  assert.equal(schema.operations['git.ciExactHead'].result.$ref,'#/$defs/ExactHeadCiEvidence');
  assert.equal(schema.definitions.ExactHeadCiEvidence.properties.credentialsUsed.const,false);
  assert.match(host,/actions\/runs\?head_sha=/);
  assert.match(host,/redirects\(0\)/);
  assert.match(host,/matches_workflow\(/);
  assert.doesNotMatch(host,/std::process::Command|Command::new|\.push\(|\.fetch\(/);
  assert.match(ui,/Check exact HEAD CI/);
  assert.match(ui,/invoke\('ide_read_exact_head_ci'/);
});
