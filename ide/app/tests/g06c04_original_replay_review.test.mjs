import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const native = read('../../host/src/git_rebase_cherry_plan.rs');
const host = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06C04A native replay plan binds original linear commits and commits no operation', () => {
  assert.match(host, /pub mod git_rebase_cherry_plan;/);
  assert.match(native, /pub enum ReplayReviewKind/);
  assert.match(native, /RebaseCurrentOntoLocal/);
  assert.match(native, /CherryPickLocalTip/);
  assert.match(native, /pub fn review_local_rebase_cherry_pick\(/);
  assert.match(native, /repo\.merge_bases\(/);
  assert.match(native, /fn first_parent_range\(/);
  assert.match(native, /G06C04_REBASE_COMMIT_LIMIT/);
  assert.match(native, /G06C04_REBASE_MERGE_OR_ROOT_IN_RANGE/);
  assert.match(native, /G06C04_ROOT_OR_MERGE_COMMIT_REVIEW_DENIED/);
  assert.match(native, /repo\.diff_tree_to_tree\(/);
  assert.match(native, /path_ledger_sha256/);
  assert.match(native, /G06C04_SOURCE_CHANGED_DURING_REVIEW/);
  assert.match(native, /G06C04_NOT_UNIQUE_MERGE_BASE/);
  assert.match(native, /replay_resulting_trees_not_verified/);
  assert.match(native, /one_use_native_os_authorization_not_issued/);
  assert.match(native, /history_rewrite: kind == ReplayReviewKind::RebaseCurrentOntoLocal/);
  assert.match(native, /original_commits_retained: true/);
  assert.match(native, /branch_ref_updated: false/);
  assert.match(native, /worktree_or_index_modified: false/);
  assert.match(native, /execution_route_available: false/);
  assert.match(native, /one_use_os_approval_granted: false/);
  assert.match(native, /resulting_tree_verified: false/);
  assert.match(native, /source_rechecked: true/);
  const split = native.split(/#\[cfg\(test\)\]\r?\nmod tests\s*\{/);
  assert.equal(split.length, 2);
  const production = split[0];
  for (const forbidden of [
    'std::process::Command', 'repo.commit(', 'repo.reference_matching(',
    'repo.checkout(', 'repo.reset(', 'repo.rebase(', 'repo.cherrypick(',
    'repo.merge(', 'repo.fetch(', 'repo.push(', 'repo.stash_save(',
    'repo.checkout_tree(', 'repo.cherrypick_commit('
  ]) assert.ok(!production.includes(forbidden), forbidden);
});

test('G06C04A typed preview only through registered Tauri host no executor', () => {
  assert.match(tauri, /async fn ide_git_review_rebase_cherry_pick\(/);
  assert.match(tauri, /operation: lu_ide_workspace::git_rebase_cherry_plan::ReplayReviewKind/);
  assert.match(tauri, /review_local_rebase_cherry_pick\(&project_id, operation, &selected_branch\)/);
  assert.match(tauri, /ide_git_review_rebase_cherry_pick,/);
  for (const forbidden of [
    'ide_git_execute_rebase', 'ide_git_execute_cherry_pick',
    'ide_git_apply_rebase', 'ide_git_apply_cherry_pick'
  ]) assert.ok(!tauri.includes('async fn '+forbidden+'('));
});

test('G06C04A frontend never treats read-only plan as one-use OS approval or result', () => {
  assert.match(ui, /aria-label="G06C04 original rebase cherry-pick history review no execution"/);
  assert.match(ui, /invoke\('ide_git_review_rebase_cherry_pick'/);
  assert.match(ui, /G06C04_REPLAY_PREVIEW_STALE_OR_UNTRUSTED/);
  assert.match(ui, /plan\.upstreamReviewSha256 !== upstreamSha/);
  assert.match(ui, /plan\.executionRouteAvailable !== false/);
  assert.match(ui, /plan\.oneUseOsApprovalGranted !== false/);
  assert.match(ui, /plan\.resultingTreeVerified !== false/);
  assert.match(ui, /plan\.sourceRechecked !== true/);
  assert.match(ui, /replayPlanRequest\.current \+= 1/);
  assert.match(ui, /setReplayPlan\(null\)/);
  assert.match(ui, /no execution route for rebase or cherry-pick/);
  assert.doesNotMatch(ui, /invoke\('ide_git_execute_rebase'/);
  assert.doesNotMatch(ui, /invoke\('ide_git_execute_cherry_pick'/);
});
