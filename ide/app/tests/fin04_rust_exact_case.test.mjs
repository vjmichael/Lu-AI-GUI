import test from 'node:test';
import assert from 'node:assert/strict';
import {visibleStructuredTestEvidence} from '../web/structured_test_evidence.js';
const sha='a'.repeat(64),head='b'.repeat(40),pid='ide-'+'c'.repeat(32),job='job-'+'d'.repeat(32);
const recipe='rust_exact_case:checks';
const result={
  projectId:pid,jobId:job,recipeId:recipe,
  planSha256:sha,terminalReceiptSha256:sha,manifestSha256:sha,
  inputSnapshotSha256:sha,toolchainSha256:sha,sourceHead:head,
  phase:'completed',requiresReconciliation:false,timedOut:false,
  nativeExitCode:0,
};
const report={
 schemaVersion:1,projectId:pid,jobId:job,recipeId:recipe,
 planSha256:sha,terminalReceiptSha256:sha,sourceHead:head,
 inputSnapshotSha256:sha,manifestSha256:sha,toolchainSha256:sha,
 observedLogSha256:sha,captureTruncated:false,casesTruncated:false,
 caseCountObserved:1,
 evidenceState:'native_terminal_framework_reported_not_assertion_verified',
 assertionIndependentlyVerified:false,acceptedByHuman:false,installedAppAccepted:false,
 cases:[{caseId:'case_'+sha,name:'checks',status:'reported_pass',
  reporter:'cargo_libtest_exact_filter',source:null,
  navigationVerified:false,assertionIndependentlyVerified:false}]
};
test('FIN04 real native exact case reports are not human acceptance',()=>{
 const visible=visibleStructuredTestEvidence({...result,testCaseEvidence:report},pid);
 assert.equal(visible?.cases.length,1);
 assert.equal(visible?.taskAcceptanceAuthorized,false);
});
test('FIN04 case name or reporter spoof cannot certify a pass',()=>{
 assert.equal(visibleStructuredTestEvidence({...result,
   recipeId:'rust_exact_case:bad;script',testCaseEvidence:report},pid),null);
 const tampered={...report,cases:[{...report.cases[0],
   reporter:'cargo_libtest'}]};
 assert.equal(visibleStructuredTestEvidence({...result,testCaseEvidence:tampered},pid),null);
});
