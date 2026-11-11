import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const file = relative => readFileSync(new URL(relative, import.meta.url), 'utf8');
const native = file('../../host/src/git_merge_plan.rs');
const base = file('../../host/src/git_branch_operations.rs');
const host = file('../../host/src/lib.rs');
const tauri = file('../src-tauri/src/lib.rs');
const ui = file('../web/main.jsx');

test('G06C02 source planner reads commit trees and index without mutating the actual Git repo', () => {
  assert.match(host, /pub mod git_merge_plan;/);
  assert.match(native, /pub fn preview_reviewed_local_merge\(/);
  assert.match(native, /merge_bases\(/);
  assert.match(native, /tree_manifest\(/);
  assert.match(native, /tree\.walk\(TreeWalkMode::PreOrder/);
  assert.match(native, /ordinary_index_sha\(/);
  assert.match(native, /source_fence\(&repo, selected_branch\)/);
  assert.match(native, /G06C02_SOURCE_CHANGED_DURING_PREVIEW/);
  assert.match(native, /fast_forward_candidate/);
  assert.match(native, /divergent_merge_candidate/);
  assert.match(native, /multiple_merge_bases_unresolved/);
  assert.match(native, /potential_conflicts_require_manual_review/);
  assert.match(native, /no_same_path_overlap_observed_not_a_clean_merge_proof/);
  assert.match(native, /divergent_result_tree_not_verified/);
  const implementation = native.split('#[cfg(test)]')[0];
  for (const forbidden of [
    'std::process::Command', 'repo.merge_trees(', 'repo.merge_commits(',
    'repo.checkout(', 'repo.merge(', 'repo.cherrypick(', 'repo.rebase(',
    'repo.reference(', 'repo.reference_matching(', 'repo.commit(',
    'repo.stash_save(', 'repo.fetch(', 'repo.push(', 'repo.index().write('
  ]) assert.ok(!implementation.includes(forbidden), forbidden);
  assert.match(native, /three_way_content_merge_performed: false/);
  assert.match(native, /rename_analysis_performed: false/);
  assert.match(native, /authorization_granted: false/);
  assert.match(native, /branch_ref_updated: false/);
  assert.match(native, /index_or_worktree_modified: false/);
  assert.match(native, /execution_route_available: false/);
  assert.match(base, /pub\(crate\) fn exact_repo\(/);
});

test('G06C02 source gates are typed, registered, read-only and do not export an executor', () => {
  assert.match(tauri, /async fn ide_git_preview_local_merge\(/);
  assert.match(tauri, /preview_reviewed_local_merge\(&project_id, &selected_branch\)/);
  assert.match(tauri, /ide_git_preview_local_merge,/);
  for (const name of ['ide_git_apply_merge', 'ide_git_execute_merge', 'ide_git_merge_commit']) {
    assert.ok(!tauri.includes('async fn ' + name + '('), name);
  }
});

test('G06C02 UI displays conservative conflict review and rejects forged/stale plans', () => {
  assert.match(ui, /aria-label="G06C02 reviewed local merge plan no execution"/);
  assert.match(ui, /invoke\('ide_git_preview_local_merge'/);
  assert.match(ui, /G06C02_MERGE_PREVIEW_STALE_OR_UNTRUSTED/);
  assert.match(ui, /plan\.upstreamReviewSha256 !== expectedUpstreamSha/);
  assert.match(ui, /plan\.sourceRechecked !== true/);
  assert.match(ui, /plan\.authorizationGranted !== false/);
  assert.match(ui, /plan\.branchRefUpdated !== false/);
  assert.match(ui, /plan\.indexOrWorktreeModified !== false/);
  assert.match(ui, /plan\.executionRouteAvailable !== false/);
  assert.match(ui, /mergePlanRequest\.current \+= 1/);
  assert.match(ui, /setMergePlan\(null\)/);
  assert.match(ui, /Potential conflicts:/);
  assert.match(ui, /not a completed[\s\S]{0,100}three-way content merge/);
  assert.doesNotMatch(ui, /invoke\('ide_git_execute_local_merge'/);
});
