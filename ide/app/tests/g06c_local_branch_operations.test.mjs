import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const native = read('../../host/src/git_branch_operations.rs');
const lib = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06C01 host classifies only bounded existing local branches, no remote or mutation', () => {
  assert.match(lib, /pub mod git_branch_operations;/);
  for (const operation of ['Merge','Rebase','CherryPick','Revert','Stash'])
    assert.match(native, new RegExp('\\b'+operation+'\\b'));
  assert.match(native, /BranchType::Local/);
  assert.match(native, /G06C_LINKED_OR_ALIASED_GITDIR/);
  assert.match(native, /G06C_DETACHED_HEAD_DENIED/);
  assert.match(native, /G06C_STASH_TARGET_DENIED/);
  assert.match(native, /G06C_LOCAL_BRANCH_REQUIRED/);
  assert.match(native, /git_index_lock_requires_manual_review/);
  assert.match(native, /untracked_stash_policy_unset/);
  assert.match(native, /merge_base\(/);
  assert.match(native, /graph_ahead_behind\(/);
  assert.match(native, /graph_descendant_of\(/);
  for (const forbidden of [
    'std::process::Command', 'repo.checkout(', 'repo.stash_save(',
    'repo.cherrypick(', 'repo.revert(', 'repo.merge(', 'repo.rebase(',
    'repo.reference_matching(', 'repo.reference(', 'repo.commit(',
    'repo.push(', 'repo.fetch(', 'git2::build::CheckoutBuilder'
  ]) {
    const production = native.split('#[cfg(test)]')[0];
    assert.ok(!production.includes(forbidden), forbidden);
  }
  assert.match(native, /operation_authorized: false/);
  assert.match(native, /operation_executed: false/);
  assert.match(native, /execution_route_available: false/);
  assert.match(native, /remote_queried: false/);
  assert.match(native, /repository_modified: false/);
});

test('G06C01 Tauri has exactly one typed preview-only command, never a raw git executor', () => {
  assert.match(tauri, /async fn ide_git_review_branch_operation\(/);
  assert.match(tauri, /operation: lu_ide_workspace::git_branch_operations::BranchOperationKind/);
  assert.match(tauri, /registry\s*\.review_local_branch_operation\(/);
  assert.match(tauri, /ide_git_review_branch_operation,/);
  for (const command of [
    'ide_git_merge_branch', 'ide_git_rebase_branch', 'ide_git_cherry_pick',
    'ide_git_revert_commit', 'ide_git_stash_apply'
  ]) assert.ok(!tauri.includes('async fn '+command+'('), command);
});

test('G06C01 source-control UI only inspects, source-scopes and denies forged completion', () => {
  assert.match(ui, /aria-label="G06C local Git branch operation review only"/);
  assert.match(ui, /invoke\('ide_git_review_branch_operation'/);
  assert.match(ui, /G06C_PREVIEW_UNTRUSTED_OR_MISMATCHED/);
  assert.match(ui, /review\.operationAuthorized !== false/);
  assert.match(ui, /review\.executionRouteAvailable !== false/);
  assert.match(ui, /review\.repositoryModified !== false/);
  assert.match(ui, /branchReviewRequest\.current \+= 1/);
  assert.match(ui, /setBranchReview\(null\)/);
  assert.match(ui, /No static safety blockers were observed/);
  assert.match(ui, /This does not[\s\S]{0,140}authorize a branch operation/);
  assert.match(ui, /<option value="cherry_pick">/);
  assert.match(ui, /<option value="stash">/);
  assert.doesNotMatch(ui, /invoke\('ide_git_execute_branch_operation'/);
});
