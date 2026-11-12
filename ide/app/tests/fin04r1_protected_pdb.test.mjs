// FIN-04R-1 source-boundary security contracts. Run with Node native
// test runner; not a substitute for actual LLVM/Windows Job/AppContainer,
// native human approval, kernel-restart or installed-app acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const root=new URL('../../',new URL('../',import.meta.url));
const read=p=>readFileSync(new URL(p,root),'utf8');
const worker=read('ide/supervisor/src/windows_lsp_worker.rs');
const source=read('ide/supervisor/src/windows_stopped_line_oracle.rs');
const gate=read('ide/supervisor/src/dap_transport.rs');
const transport=read('ide/supervisor/src/lsp_transport.rs');
const controls=read('ide/supervisor/src/debug_control_dispatch.rs');
const reviews=read('ide/supervisor/src/debug_command_review.rs');
const receipt=read('ide/supervisor/src/debug_action_receipts.rs');
const desktop=read('ide/app/src-tauri/src/debug_backend.rs');
const tauri=read('ide/app/src-tauri/src/lib.rs');
const browser=read('ide/app/web/main.jsx');
const authority=read('ide/app/web/project_debug_authority.js');

test('old host Command symbolizer is strictly dev-only',()=>{
 assert.match(source,/#\[cfg\(all\(debug_assertions, feature = "dev045d4c-ci-oracle"\)\)\]\s*fn run_measured_reader/);
 assert.match(source,/fn independently_verify_pdb_line_with_reader/);
 assert.match(source,/let json = protected_read\(/);
 assert.match(source,/fn independently_verify_pdb_line_for_ci/);
});
test('release helper is independently AppContainer+SCM Job-contained, never generic OS spawn',()=>{
 assert.match(worker,/fn read_protected_pe_pdb\(/);
 assert.match(worker,/spawn_contained_stdio\([\s\S]*?false,\s*\)\?/);
 assert.match(worker,/if owner_channel\.is_some\(\)/);
 assert.match(worker,/native_lsp_bundle_digest\(&bundle,"dap_lldb_project"\)/);
 assert.match(worker,/verify_suspended_appcontainer_token/);
 assert.match(worker,/let jobs = \[job\.handle\.0\]/);
 assert.match(worker,/if !self\.lease\.alive\(\)/);
 assert.match(worker,/output\.len\(\)>=MAX_OUTPUT/);
 assert.match(worker,/GetExitCodeProcess\(child\.0/);
 assert.match(worker,/check_file\(pdb,pdb_sha256\)\?/);
 assert.match(gate,/fn read_pe_pdb_inside_owned_job/);
 assert.match(transport,/fn read_protected_pe_pdb/);
});
test('mapped source is exact original kernel stop and original source/PDB/Job seals',()=>{
 assert.match(worker,/pub fn observe_release_owned_stop\(/);
 assert.match(worker,/independently_verify_pdb_line_with_reader/);
 assert.match(worker,/read_pe_pdb_inside_owned_job/);
 assert.match(worker,/end!=event/);
 assert.match(worker,/!self\.gateway\.poll\(\)\?\.is_empty\(\)/);
 assert.match(worker,/target\.verify\(\)\?/);
 assert.match(source,/equal_mapped_line\(/);
 assert.match(source,/compiled_source_identity\(/);
 assert.match(desktop,/release_owner_source_line_verified/);
 assert.match(desktop,/release_owner_verified_pdb_sha256/);
 assert.match(authority,/releaseOwnerVerifiedPdbSha256!==witness\.symbolsSha/);
});
test('one-use native release review is immediately revoked without any DAP dispatch',()=>{
 assert.match(worker,/fn review_one_release_action_without_dispatch/);
 assert.match(worker,/review_from_protected_release_owner/);
 assert.match(worker,/NativeConsentMarker::for_protected_release_modal/);
 assert.match(worker,/action\.revoke\(crate::now_ms\(\)\?\)/);
 assert.match(worker,/verify_native_debug_action_after_restart/);
 assert.match(receipt,/fn for_protected_release_modal/);
 assert.match(reviews,/fn review_from_protected_release_owner/);
 assert.match(desktop,/release_one_use_human_review_revoked_no_dispatch/);
 assert.match(tauri,/ide_debug_review_release_action/);
 assert.match(browser,/invoke\('ide_debug_review_release_action'/);
});
test('post-command step requires newer OS/PDB stop; breakpoint ACK alone never verifies installation',()=>{
 assert.match(controls,/fn verify_independent_step_result/);
 assert.match(controls,/fn verify_independent_breakpoint_hit/);
 assert.match(controls,/new_kernel_owned_pe_pdb_hit_is_distinct_from_dap_breakpoint_ack/);
 assert.match(controls,/after\.owner_sequence != dispatched\.from\.owner_sequence/);
 assert.match(controls,/after\.owner_receipt_sha256 == dispatched\.from\.owner_receipt_sha256/);
 assert.match(controls,/if cfg!\(all\(windows, feature = "fin04-release-owner-evidence"\)\)/);
 assert.match(controls,/return Err\(ControlDenial::Unreviewed\)/);
});
test('unknown interrupted results remain non-replay and distinct from physical human acceptance',()=>{
 assert.match(desktop,/record_action_os_recovery/);
 assert.match(desktop,/actionOutcome":"unknown_never_replay"/);
 assert.match(desktop,/post_command_result_independently_verified:false/);
 assert.match(desktop,/global_marker_cleared:false/);
 assert.match(desktop,/installed_app_accepted:false/);
});
