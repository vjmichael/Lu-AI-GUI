import test from 'node:test';
import assert from 'node:assert/strict';
import {safeDebugSymbolEvidence} from '../web/debug_symbol_evidence.js';
const pid='ide-'+'a'.repeat(32);
const h=c=>c.repeat(64);
function base(linux=true){
 const path='target/debug/fixture'+(linux?'':'.exe');
 const candidate={
  relativePath:path,bytes:350,binarySha256:h('b'),
  outputTreeSha256:h('c'),format:linux?'elf_candidate_magic_only':'pe_candidate_mz_only',
  role:'post_termination_unique_cargo_binary_readback_not_launch_authority',
  debugSymbolsAttested:false,debuggerExecutionAuthorized:false,installedAppAccepted:false
 };
 const symbol={
  binaryRelativePath:path,binarySha256:h('b'),outputTreeSha256:h('c'),
  executableFormat:linux?'elf64_x86_64_executable_dwarf_sections':'pe32plus_x86_64_executable_codeview_rsds',
  symbolsKind:linux?'embedded_dwarf_info_abbrev_line':'external_pdb_guid_age_dbi_match',
  symbolsRelativePath:linux?null:'target/debug/fixture.pdb',
  symbolsSha256:h('d'),
  validationScope:linux?'elf64_headers_load_segments_and_dwarf_unit_headers_structurally_checked_not_full_die_validation':
    'pe_debug_directory_rsds_guid_age_matches_pdb_info_and_dbi_not_full_symbol_parse',
  role:'structural_binary_and_debug_information_only_no_debugger_authority',
  debugSymbolsAttested:true,sourceLineMappingVerified:false,
  debuggerExecutionAuthorized:false,installedAppAccepted:false
 };
 const artifacts=[{relativePath:path,bytes:350,sha256:h('b')}];
 if(!linux)artifacts.push({relativePath:'target/debug/fixture.pdb',bytes:100,sha256:h('d')});
 return {
  projectId:pid,jobId:'job-'+'a'.repeat(32),recipeId:'rust_build',
  phase:'completed',nativeExitCode:0,requiresReconciliation:false,
  installedAppAccepted:false,planSha256:h('e'),
  sourceHead:'f'.repeat(40),inputSnapshotSha256:h('1'),
  toolchainSha256:h('2'),manifestSha256:h('3'),
  terminalReceiptSha256:h('4'),artifactManifestSha256:h('c'),
  artifactEvidenceState:'read_only_post_termination_tree_hashed',
  debugBinaryCandidate:candidate,debugSymbolWitness:symbol,
  artifactFiles:artifacts,
 };
}
test('preserves source→receipt→binary→ELF embedded DWARF evidence, no authority',()=>{
 const v=safeDebugSymbolEvidence(base(true),pid);
 assert.ok(v);assert.equal(v.symbolsPath,null);
 assert.equal(v.structuralValidation,true);
 assert.equal(v.sourceMappingVerified,false);
 assert.equal(v.debuggerAuthorized,false);
});
test('preserves exact external matching PDB manifest and binary receipt',()=>{
 const v=safeDebugSymbolEvidence(base(false),pid);
 assert.equal(v.symbolsPath,'target/debug/fixture.pdb');
 assert.equal(v.kind,'external_pdb_guid_age_dbi_match');
});
test('rejects broken GUID/age evidence identity, fabricated symbol paths and authority',()=>{
 for(const [prop,value] of [
  ['binarySha256',h('9')],['symbolsSha256','invalid'],
  ['symbolsRelativePath','../../foreign.pdb'],
  ['debuggerExecutionAuthorized',true],
  ['sourceLineMappingVerified',true],
  ['installedAppAccepted',true],['validationScope','fully_verified']
 ]) {
   const record=base(false);record.debugSymbolWitness[prop]=value;
   assert.equal(safeDebugSymbolEvidence(record,pid),null,prop);
 }
 const r=base(false);r.artifactFiles.pop();
 assert.equal(safeDebugSymbolEvidence(r,pid),null);
});
test('rejects cancelled, nonzero, cross-project and post-restart false identity',()=>{
 const result=base();
 result.phase='cancelled';
 assert.equal(safeDebugSymbolEvidence(result,pid),null);
 result.phase='completed';result.nativeExitCode=1;
 assert.equal(safeDebugSymbolEvidence(result,pid),null);
 result.nativeExitCode=0;
 assert.equal(safeDebugSymbolEvidence(result,'ide-'+'b'.repeat(32)),null);
});
