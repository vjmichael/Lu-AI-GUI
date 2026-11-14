import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = name => readFileSync(new URL(name, import.meta.url), 'utf8');
const host = read('../../host/src/git_batch_review.rs');
const registry = read('../../host/src/lib.rs');
const routes = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06B preview uses the existing tracked regular file capability for every path', () => {
  assert.match(registry, /pub mod git_batch_review;/);
  assert.match(host, /preview_reviewed_git_file\(project_id, path\)/);
  assert.match(host, /MAX_FILES: usize = 8/);
  assert.match(host, /MAX_BATCH_BYTES: usize = 4 \* 1024 \* 1024/);
  assert.match(host, /BTreeSet/);
  assert.match(host, /G06B_SOURCE_CHANGED_DURING_PREVIEW/);
  const production = host.split('#[cfg(test)]')[0];
  for (const forbidden of [
    'Command::new', 'std::process::', 'repo.checkout', 'repo.fetch',
    'repo.push', 'index.write()', 'repo.commit(', 'git_hooks_executed: true',
    'stage_authorized: true', 'commit_authorized: true',
  ]) assert.ok(!production.includes(forbidden), forbidden);
});
test('G06B Tauri API remains preview-only without approval bypass or raw command', () => {
  assert.match(routes, /async fn ide_git_preview_batch\(/);
  assert.match(routes, /async fn ide_git_verify_batch_review\(/);
  assert.match(routes, /registry\.preview_reviewed_git_batch\(&project_id, &relative_paths\)/);
  assert.match(routes, /registry\.revalidate_reviewed_git_batch\(&expected\)/);
  assert.match(routes, /ide_git_preview_batch,\s*ide_git_verify_batch_review,/);
});
test('G06B UI displays pinned files but has no multi-file stage or commit action', () => {
  assert.match(ui, /invoke\('ide_git_preview_batch'/);
  assert.match(ui, /invoke\('ide_git_verify_batch_review'/);
  assert.match(ui, /gitBatchReview\.files\.map/);
  assert.match(ui, /Stage authorized: <strong>No<\/strong>/);
  assert.match(ui, /Commit authorized: <strong>No<\/strong>/);
  assert.doesNotMatch(ui, /invoke\('ide_git_stage_batch'/);
  assert.doesNotMatch(ui, /invoke\('ide_git_commit_batch'/);
});
