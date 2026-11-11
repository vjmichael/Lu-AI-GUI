import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {visibleStructuredTestEvidence,
  reviewedTestFailureTarget} from '../web/structured_test_evidence.js';

const projectId='ide-'+'a'.repeat(32);
const jobId='job-'+'b'.repeat(32);
const planSha='c'.repeat(64);
const terminalSha='d'.repeat(64);
const sourceSha='e'.repeat(64);
const caseId='case_'+'f'.repeat(64);
const receipt=()=>({
  projectId,jobId,recipeId:'python_pytest',phase:'failed',
  planSha256:planSha,terminalReceiptSha256:terminalSha,
  sourceHead:'1'.repeat(40),inputSnapshotSha256:'2'.repeat(64),
  manifestSha256:'3'.repeat(64),toolchainSha256:'4'.repeat(64),
  nativeExitCode:1,requiresReconciliation:false,timedOut:false,
  installedAppAccepted:false,
  testCaseEvidence:{
    schemaVersion:1,projectId,jobId,recipeId:'python_pytest',
    planSha256:planSha,terminalReceiptSha256:terminalSha,
    sourceHead:'1'.repeat(40),inputSnapshotSha256:'2'.repeat(64),
    manifestSha256:'3'.repeat(64),toolchainSha256:'4'.repeat(64),
    observedLogSha256:'5'.repeat(64),
    captureTruncated:false,casesTruncated:false,caseCountObserved:1,
    evidenceState:'native_terminal_framework_reported_not_assertion_verified',
    assertionIndependentlyVerified:false,acceptedByHuman:false,
    installedAppAccepted:false,
    cases:[{
      caseId,name:'tests/test_math.py::test_subtract',
      status:'reported_fail',reporter:'pytest_reported',
      source:{relativePath:'tests/test_math.py',diskSha256:sourceSha,
        line:17,characterUtf16:0},
      navigationVerified:false,assertionIndependentlyVerified:false,
    }],
  },
});

test('one original native receipt can yield bounded reported failures only',()=>{
  const accepted=visibleStructuredTestEvidence(receipt(),projectId);
  assert.equal(accepted.cases.length,1);
  assert.equal(accepted.cases[0].source.line,17);
  assert.equal(accepted.independentlyVerified,false);
  assert.equal(accepted.installedAccepted,false);
  assert.equal(accepted.taskAcceptanceAuthorized,false);
});

test('native review requires exactly same case, terminal receipt and saved SHA',()=>{
  const view=visibleStructuredTestEvidence(receipt(),projectId);
  const review={
    projectId,jobId,planSha256:planSha,
    terminalReceiptSha256:terminalSha,caseId,
    relativePath:'tests/test_math.py',diskSha256:sourceSha,
    line:17,characterUtf16:0,
    savedSourceVerified:true,
    testAssertionIndependentlyVerified:false,taskAccepted:false,
  };
  assert.equal(reviewedTestFailureTarget(view,caseId,review).line,17);
  for(const tamper of [
    {line:18},{diskSha256:'f'.repeat(64)},
    {relativePath:'../../outside.py'},
    {caseId:'case_'+'1'.repeat(64)},
    {planSha256:'7'.repeat(64)},
    {terminalReceiptSha256:'8'.repeat(64)},
    {savedSourceVerified:false},
    {testAssertionIndependentlyVerified:true},
    {taskAccepted:true},
  ]){
    assert.equal(reviewedTestFailureTarget(view,caseId,{...review,...tamper}),null);
  }
});

test('malicious source anchors and same-name cross-project claims are denied',()=>{
  for(const path of [
    '../../secret.py','/etc/passwd','C:\\secrets\\hidden.py',
    'tests/../private.py','tests//hidden.py','file:///tmp/secret.py',
  ]){
    const data=receipt();
    data.testCaseEvidence.cases[0].source.relativePath=path;
    assert.equal(visibleStructuredTestEvidence(data,projectId),null);
  }
  const foreign=receipt();foreign.projectId='ide-'+'9'.repeat(32);
  assert.equal(visibleStructuredTestEvidence(foreign,projectId),null);
  const bad=receipt();bad.testCaseEvidence.terminalReceiptSha256='8'.repeat(64);
  assert.equal(visibleStructuredTestEvidence(bad,projectId),null);
  const lie=receipt();lie.testCaseEvidence.assertionIndependentlyVerified=true;
  assert.equal(visibleStructuredTestEvidence(lie,projectId),null);
});

test('interrupted or unfinished worker cannot manufacture source-linked test cases',()=>{
  const notReconciled=receipt();
  notReconciled.requiresReconciliation=true;
  assert.equal(visibleStructuredTestEvidence(notReconciled,projectId),null);
  const interrupted=receipt();
  interrupted.testCaseEvidence.evidenceState=
    'native_not_reconciled_test_evidence_suppressed';
  assert.equal(visibleStructuredTestEvidence(interrupted,projectId),null);
  const fabricated=receipt();
  fabricated.phase='running';
  assert.equal(visibleStructuredTestEvidence(fabricated,projectId),null);
  const duplicate=receipt();
  duplicate.testCaseEvidence.cases.push({...duplicate.testCaseEvidence.cases[0]});
  duplicate.testCaseEvidence.caseCountObserved=2;
  assert.equal(visibleStructuredTestEvidence(duplicate,projectId),null);
});

test('bounded report never invents assertion counts from truncated output',()=>{
  const long=receipt();
  long.testCaseEvidence.cases=Array.from({length:128},(_,i)=>({
    caseId:'case_'+i.toString(16).padStart(64,'0'),
    name:'test_'+i,status:'reported_pass',reporter:'pytest_reported',
    source:null,navigationVerified:false,assertionIndependentlyVerified:false,
  }));
  long.testCaseEvidence.casesTruncated=true;
  long.testCaseEvidence.captureTruncated=true;
  long.testCaseEvidence.caseCountObserved=205;
  const data=visibleStructuredTestEvidence(long,projectId);
  assert.equal(data.cases.length,128);
  assert.equal(data.casesTruncated,true);
  assert.equal(data.independentlyVerified,false);
});

test('source test target IPC accepts only opaque job/plan/case, not paths',()=>{
  const lib=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const backend=readFileSync(new URL('../src-tauri/src/build_backend.rs',
    import.meta.url),'utf8');
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(lib,/async fn ide_build_review_test_target\(/);
  assert.match(lib,/ide_build_review_test_target,/);
  assert.match(backend,/pub fn review_test_case_target\(/);
  assert.match(backend,/last\.entry_hash!=report\.terminal_receipt_sha256/);
  assert.match(backend,/registry\.preview_source_file\(project_id,&source\.relative_path\)/);
  assert.match(ui,/invoke\('ide_build_review_test_target'/);
  assert.match(ui,/opened\.revision!==target\.diskSha256/);
  assert.match(ui,/current\.dirty/);
  assert.match(ui,/not independently verified/);
});
