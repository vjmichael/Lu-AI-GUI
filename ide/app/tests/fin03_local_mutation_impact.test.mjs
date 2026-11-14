import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const host=readFileSync(new URL('../../host/src/git_fin03_operation_preview.rs',import.meta.url),'utf8');
const implementation=host.split('#[cfg(test)]')[0];
const desktop=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
test('FIN03G original Git objects only and no mutation API',()=>{
 assert.match(host,/pub fn review_fin03_local_impact/);
 assert.match(host,/Fin03ImpactKind \{ RevertCommit,StashApply,StashDrop \}/);
 assert.match(host,/repo\.diff_tree_to_tree/);
 assert.match(host,/repo\.graph_descendant_of/);
 assert.match(host,/repo\.reflog\("refs\/stash"\)/);
 assert.match(host,/return Err\("FIN03G_STASH_TIP_DISAGREES_WITH_LOG"\)/);
 assert.match(host,/return Err\("FIN03G_SOURCE_CHANGED_DURING_PREVIEW"\)/);
 assert.doesNotMatch(implementation,/\.stash_save\(|\.stash_apply\(|\.stash_drop\(|\.checkout_\w*\(|\.commit\(|\.write_tree_to\(|std::process::Command/);
});
test('FIN03G does not treat path overlap as proof of conflict-free execution',()=>{
 assert.match(host,/risk_assessment:"path_overlap_only_not_conflict_or_result_tree_proof"/);
 assert.match(host,/resulting_tree_and_conflicts_not_verified/);
 assert.match(host,/fin01_kernel_acceptance_required_before_mutation/);
 assert.match(host,/execution_route_available:false,execution_authorized:false/);
 assert.match(host,/original_git_objects_preserved:true/);
 assert.match(host,/index_or_worktree_modified:false,stash_or_branch_ref_modified:false/);
 assert.match(host,/index_sha256:before.index_sha/);
});
test('FIN03G native read-only command and accessible UI never supply action authority',()=>{
 assert.match(desktop,/async fn ide_git_preview_local_mutation_impact/);
 assert.match(desktop,/ide_git_preview_local_mutation_impact,/);
 assert.match(ui,/invoke\('ide_git_preview_local_mutation_impact'/);
 assert.match(ui,/FIN-03G read-only local revert and stash impact review/);
 assert.match(ui,/result\.executionAuthorized!==false/);
 assert.match(ui,/result\.executionRouteAvailable!==false/);
 assert.match(ui,/Preview impact \(no changes\)/);
});
