import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkedProjectDebugWitness} from '../web/project_debug_authority.js';
const h=c=>c.repeat(64);
const project='ide-'+'a'.repeat(32);
function values(){
 const b={relativePath:'target/debug/fixture',bytes:350,
   binarySha256:h('b'),outputTreeSha256:h('c'),
   format:'elf_candidate_magic_only',
   role:'post_termination_unique_cargo_binary_readback_not_launch_authority',
   debugSymbolsAttested:false,debuggerExecutionAuthorized:false,installedAppAccepted:false};
 const symbol={
   binaryRelativePath:b.relativePath,binarySha256:b.binarySha256,
   outputTreeSha256:h('c'),executableFormat:'elf64_x86_64_executable_dwarf_sections',
   symbolsKind:'embedded_dwarf_info_abbrev_line',symbolsRelativePath:null,
   symbolsSha256:h('d'),
   validationScope:'elf64_headers_load_segments_and_dwarf_unit_headers_structurally_checked_not_full_die_validation',
   role:'structural_binary_and_debug_information_only_no_debugger_authority',
   debugSymbolsAttested:true,sourceLineMappingVerified:false,
   debuggerExecutionAuthorized:false,installedAppAccepted:false
 };
 const build={
   projectId:project,jobId:'job-'+'1'.repeat(32),recipeId:'rust_build',
   phase:'completed',nativeExitCode:0,requiresReconciliation:false,
   installedAppAccepted:false,planSha256:h('e'),sourceHead:'f'.repeat(40),
   inputSnapshotSha256:h('2'),manifestSha256:h('3'),toolchainSha256:h('4'),
   terminalReceiptSha256:h('5'),artifactManifestSha256:h('c'),
   artifactEvidenceState:'read_only_post_termination_tree_hashed',
   debugBinaryCandidate:b,debugSymbolWitness:symbol,
   artifactFiles:[{relativePath:b.relativePath,bytes:b.bytes,sha256:b.binarySha256}]
 };
 const debug={
   projectId:project,jobId:'job-'+'6'.repeat(32),
   originBuildJobId:build.jobId,
   originBuildPlanSha256:build.planSha256,
   originBuildReceiptSha256:build.terminalReceiptSha256,
   binarySha256:b.binarySha256,symbolSha256:symbol.symbolsSha256,
   targetBindingSha256:h('7'),planSha256:h('8'),sourceHead:build.sourceHead,
   bundleSha256:h('9'),
   terminalPhase:'completed',target:'one_exact_private_project_build_in_protected_job',
   verifiedOsTreeReleaseByWorker:true,stoppedThread:1,
   threadsResponseObserved:true,stackResponseObserved:true,
   installedAppAccepted:false
 };
 return {build,debug};
}
test('true project debug result is bound to separate fresh D3 job and exact build receipt',()=>{
 const {build,debug}=values();
 const accepted=checkedProjectDebugWitness(debug,build,project);
 assert.ok(accepted);
 assert.equal(accepted.buildReceiptSha,build.terminalReceiptSha256);
 assert.notEqual(accepted.debugJobId,accepted.buildJobId);
 assert.equal(accepted.installedAccepted,false);
});
test('reject stale receipts, cross-project swap, missing OS cleanup or fake installed success',()=>{
 for(const [field,value] of [
  ['originBuildJobId','job-'+'e'.repeat(32)],
  ['originBuildPlanSha256',h('f')],
  ['originBuildReceiptSha256',h('f')],
  ['binarySha256',h('f')],['symbolSha256',h('f')],
  ['planSha256','broken'],['targetBindingSha256','broken'],
  ['terminalPhase','cancel_requested'],
  ['verifiedOsTreeReleaseByWorker',false],
  ['stackResponseObserved',false],['threadsResponseObserved',false],
  ['installedAppAccepted',true],['target','fixed_installer_selftest_only_not_project_debugger'],
 ]){
   const {build,debug}=values();debug[field]=value;
   assert.equal(checkedProjectDebugWitness(debug,build,project),null,field);
 }
 const {build,debug}=values();
 assert.equal(checkedProjectDebugWitness(debug,build,'ide-'+'b'.repeat(32)),null);
 debug.jobId=build.jobId;
 assert.equal(checkedProjectDebugWitness(debug,build,project),null);
});
test('missing native build/symbol evidence cannot authorize a debug session',()=>{
 const {build,debug}=values();
 build.debugSymbolWitness.sourceLineMappingVerified=true;
 assert.equal(checkedProjectDebugWitness(debug,build,project),null);
});

test('Windows AppContainer project PE/PDB stage belongs only to the protected DAP owner',()=>{
 const rust=readFileSync(new URL('../../supervisor/src/windows_lsp_worker.rs',import.meta.url),'utf8');
 const lsp=rust.indexOf('pub fn start_windows_lsp');
 const dap=rust.indexOf('fn start_windows_dap_inner');
 assert.ok(lsp>=0&&dap>lsp,'exact native entrypoints must exist');
 const generic=rust.slice(lsp,dap);
 const protectedProject=rust.slice(dap);
 assert.doesNotMatch(generic,/project_debuggee|let project_debuggee = if let Some\(target\) = project_target/,
   'ordinary language-server startup may not stage runnable project debug targets');
 assert.match(protectedProject,/let project_debuggee = if let Some\(target\) = project_target/);
 assert.match(protectedProject,/publish_into_native_parent\(\s*&target\.root,/);
 assert.match(protectedProject,/audit_published_output\(&stage_root\)/);
 assert.match(protectedProject,/verify_build_debug_symbols\(/);
 assert.match(protectedProject,/supervisor\.consume_grant_after_attestation/);
 assert.match(protectedProject,/BrokerLease::begin/);
 assert.match(protectedProject,/spawn_contained_stdio/);
 assert.match(protectedProject,/project_debuggee,/);
});
