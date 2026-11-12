import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {debugAdmissionStatus,checkedDebugStoppedSource,
  checkedReviewedDebugSource,checkedRedactedDebugLocals} from
  '../web/debugger_integration.js';

const project='ide-'+'a'.repeat(32);
const foreign='ide-'+'b'.repeat(32);
const sha=n=>String(n).repeat(64);
const build=()=>({
  projectId:project,jobId:'job-'+'c'.repeat(32),
  recipeId:'rust_build',phase:'completed',nativeExitCode:0,
  planSha256:sha('1'),sourceHead:'d'.repeat(40),
  terminalReceiptSha256:sha('2'),
  inputSnapshotSha256:sha('3'),manifestSha256:sha('4'),
  toolchainSha256:sha('5'),requiresReconciliation:false,
  installedAppAccepted:false,
  artifactManifestSha256:sha('7'),
  artifactEvidenceState:'read_only_post_termination_tree_hashed',
  artifactFiles:[{relativePath:'target/debug/sample',bytes:350,
    sha256:sha('6')}],
  debugBinaryCandidate:{
    relativePath:'target/debug/sample',bytes:350,
    binarySha256:sha('6'),outputTreeSha256:sha('7'),
    format:'elf_candidate_magic_only',
    role:'post_termination_unique_cargo_binary_readback_not_launch_authority',
    debugSymbolsAttested:false,debuggerExecutionAuthorized:false,
    installedAppAccepted:false,
  },
  debugSymbolWitness:{
    binaryRelativePath:'target/debug/sample',binarySha256:sha('6'),
    outputTreeSha256:sha('7'),
    executableFormat:'elf64_x86_64_executable_dwarf_sections',
    symbolsKind:'embedded_dwarf_info_abbrev_line',
    symbolsRelativePath:null,
    symbolsSha256:sha('8'),
    validationScope:'elf64_headers_load_segments_and_dwarf_unit_headers_structurally_checked_not_full_die_validation',
    role:'structural_binary_and_debug_information_only_no_debugger_authority',
    debugSymbolsAttested:true,
    sourceLineMappingVerified:false,
    debuggerExecutionAuthorized:false,installedAppAccepted:false,
  },
});
const result=()=>({
  projectId:project,jobId:'job-'+'e'.repeat(32),
  sourceHead:'d'.repeat(40),planSha256:sha('9'),
  terminalReceiptSha256:sha('a'),bundleSha256:sha('b'),
  target:'one_exact_private_project_build_in_protected_job',
  originBuildJobId:'job-'+'c'.repeat(32),
  originBuildPlanSha256:sha('1'),
  originBuildReceiptSha256:sha('2'),
  binarySha256:sha('6'),symbolSha256:sha('8'),
  targetBindingSha256:sha('c'),
  terminalPhase:'completed',verifiedOsTreeReleaseByWorker:true,
  threadsResponseObserved:true,stackResponseObserved:true,
  stoppedThread:7,installedAppAccepted:false,
  stoppedSource:{
    relativePath:'src/main.rs',diskSha256:sha('f'),
    line:23,characterUtf16:0,frameId:3,
    sourceBytesVerified:true,
    osInstructionSourceLineVerified:false,
    installedAppAccepted:false,
  },
});
test('unreconciled debugger in any project blocks all new debug operations',()=>{
  const status={ready:true,projectDebugSupported:true,
    operationControlSupported:true,current:null,journal:[],quarantined:[],
    globalRecoveryRequired:true,activeInAnotherProject:false,
    orphanedDebugActionRequired:false};
  const blocked=debugAdmissionStatus(status,project);
  assert.equal(blocked.ready,false);
  assert.equal(blocked.operationControlSupported,false);
  status.globalRecoveryRequired=false;
  status.activeInAnotherProject=true;
  assert.equal(debugAdmissionStatus(status,project).ready,false);
  status.activeInAnotherProject=false;
  status.orphanedDebugActionRequired=true;
  assert.equal(debugAdmissionStatus(status,project).ready,false);
  status.orphanedDebugActionRequired=false;
  assert.equal(debugAdmissionStatus(status,project).projectDebugSupported,true);
  assert.equal(debugAdmissionStatus(status,foreign).ready,true);
});
test('source presentation remains bound to a strict, completed original job',()=>{
  const stop=checkedDebugStoppedSource(result(),build(),project);
  assert.ok(stop,'native debugger source must be backed by complete build/symbol evidence');
  assert.equal(stop.independentOsLineMapVerified,false);
  assert.equal(stop.executionAuthorized,false);
  assert.equal(stop.relativePath,'src/main.rs');
  assert.equal(checkedDebugStoppedSource(result(),build(),foreign),null);
  const foreignPath=result();foreignPath.stoppedSource.relativePath='../private.rs';
  assert.equal(checkedDebugStoppedSource(foreignPath,build(),project),null);
  const falselyVerified=result();
  falselyVerified.stoppedSource.osInstructionSourceLineVerified=true;
  assert.equal(checkedDebugStoppedSource(falselyVerified,build(),project),null);
});
test('host reviewed source cannot change path, saved SHA, frame or authority',()=>{
  const candidate={
    projectId:project,jobId:'job-'+'e'.repeat(32),
    planSha256:sha('9'),terminalReceiptSha256:sha('a'),
    relativePath:'src/main.rs',diskSha256:sha('f'),
    line:23,frameId:3,
  };
  const response={
    projectId:project,jobId:candidate.jobId,
    planSha256:candidate.planSha256,
    terminalReceiptSha256:candidate.terminalReceiptSha256,
    source:{
      relativePath:candidate.relativePath,diskSha256:candidate.diskSha256,
      line:candidate.line,frameId:candidate.frameId,
      characterUtf16:0,sourceBytesVerified:true,
      osInstructionSourceLineVerified:false,installedAppAccepted:false,
    },
    navigationAutomatic:false,debuggerExecutionAuthorized:false,
    humanTaskAccepted:false,
  };
  assert.equal(checkedReviewedDebugSource(candidate,response),candidate);
  for(const patch of [
    {projectId:foreign},
    {terminalReceiptSha256:sha('8')},
    {debuggerExecutionAuthorized:true},
    {humanTaskAccepted:true},
    {navigationAutomatic:true},
    {source:{...response.source,relativePath:'../../secrets.rs'}},
    {source:{...response.source,line:24}},
    {source:{...response.source,osInstructionSourceLineVerified:true}},
  ]){
    assert.equal(checkedReviewedDebugSource(candidate,{...response,...patch}),null);
  }
});
test('redacted variables never include values, memory or duplicate names',()=>{
  const witness=result();
  witness.operationOutcomeCode='native_dialog_variables_redacted_uncertain';
  witness.operationReceiptSha256=sha('8');
  witness.inspectedFrameId=3;
  witness.redactedVariables=[{name:'counter',declaredType:'i32',
    valueRedacted:true}];
  const parsed=checkedRedactedDebugLocals(witness,build(),project);
  assert.ok(parsed,'redacted names and types should remain displayable');
  assert.equal(parsed[0].value,'redacted');
  assert.equal('variablesReference' in parsed[0],false);
  witness.redactedVariables.push({name:'counter',declaredType:'i32',valueRedacted:true});
  assert.equal(checkedRedactedDebugLocals(witness,build(),project),null);
});
test('debugger native UI routes never take a source path, PID or DAP payload',()=>{
  const lib=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const native=readFileSync(new URL('../src-tauri/src/debug_backend.rs',import.meta.url),'utf8');
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(lib,/async fn ide_debug_review_stopped_source\(/);
  assert.match(lib,/ide_debug_review_stopped_source,/);
  assert.match(native,/pub fn review_stopped_source\(/);
  assert.match(native,/terminal\.entry_hash\.clone\(\)/);
  assert.match(native,/verify_input\(input\)/);
  assert.match(native,/registry\.preview_source_file\(project_id,&source\.relative_path\)/);
  assert.match(ui,/invoke\('ide_debug_review_stopped_source'/);
  assert.match(ui,/opened\.revision!==reviewed\.diskSha256/);
  assert.match(ui,/current\.dirty/);
});
