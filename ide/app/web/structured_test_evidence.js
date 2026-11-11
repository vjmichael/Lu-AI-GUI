/**
 * INT03-E: test-case report presentation boundary.
 * Rows are project-controlled reporter output observed in an original native
 * OS-contained job, NOT independently verified assertions or an execution
 * grant. A source link becomes usable only after a *new* native review and
 * exact saved-file rehash immediately before opening a CodeMirror buffer.
 */
const ID=/^ide-[a-f0-9]{32}$/;
const JOB=/^job-[a-f0-9]{32}$/;
const SHA=/^[a-f0-9]{64}$/;
const HEAD=/^[a-f0-9]{40}$/;
const CASE=/^case_[a-f0-9]{64}$/;
const REPORTERS={
  rust_tests:'cargo_libtest',
  lu_ide_host_tests:'cargo_libtest',
  python_pytest:'pytest_reported',
  js_tests:'node_tap_reported',
  cpp_cmake_build:'ctest_reported',
};
const STATUS=new Set(['reported_pass','reported_fail','reported_skip']);
const STATES=new Set([
  'native_terminal_framework_reported_not_assertion_verified',
  'native_not_reconciled_test_evidence_suppressed',
]);
const SAFE_PATH=/^[^\\:\x00-\x1f\x7f]+$/u;
function safePath(path) {
  return typeof path==='string' && path.length>0 && path.length<=512 &&
    SAFE_PATH.test(path) &&
    !path.startsWith('/') &&
    path.split('/').every(segment=>segment && segment!=='.' &&
      segment!=='..' && !segment.includes(':'));
}
function anchor(source) {
  if (source===null) return null;
  if (!source || !safePath(source.relativePath) ||
      !SHA.test(source.diskSha256) ||
      !Number.isSafeInteger(source.line) || source.line<1 ||
      source.line>250000 ||
      source.characterUtf16!==0) return undefined;
  return Object.freeze({
    relativePath:source.relativePath,diskSha256:source.diskSha256,
    line:source.line,characterUtf16:0,
  });
}
export function visibleStructuredTestEvidence(result,projectId) {
  const report=result?.testCaseEvidence;
  if (!ID.test(projectId||'') || !report ||
      report.schemaVersion!==1 ||
      result.projectId!==projectId ||
      !JOB.test(result.jobId||'') ||
      !SHA.test(result.planSha256||'') ||
      !SHA.test(result.terminalReceiptSha256||'') ||
      !SHA.test(result.manifestSha256||'') ||
      !SHA.test(result.inputSnapshotSha256||'') ||
      !SHA.test(result.toolchainSha256||'') ||
      !HEAD.test(result.sourceHead||'') ||
      !REPORTERS[result.recipeId] ||
      report.projectId!==projectId ||
      report.jobId!==result.jobId ||
      report.recipeId!==result.recipeId ||
      report.planSha256!==result.planSha256 ||
      report.terminalReceiptSha256!==result.terminalReceiptSha256 ||
      report.sourceHead!==result.sourceHead ||
      report.inputSnapshotSha256!==result.inputSnapshotSha256 ||
      report.manifestSha256!==result.manifestSha256 ||
      report.toolchainSha256!==result.toolchainSha256 ||
      !SHA.test(report.observedLogSha256||'') ||
      !STATES.has(report.evidenceState) ||
      typeof report.captureTruncated!=='boolean' ||
      typeof report.casesTruncated!=='boolean' ||
      !Number.isSafeInteger(report.caseCountObserved) ||
      report.caseCountObserved<0 || report.caseCountObserved>8192 ||
      report.assertionIndependentlyVerified!==false ||
      report.acceptedByHuman!==false ||
      report.installedAppAccepted!==false ||
      !Array.isArray(report.cases) || report.cases.length>128 ||
      (report.caseCountObserved<report.cases.length) ||
      (report.casesTruncated!==(report.caseCountObserved>128)) ||
      (report.evidenceState==='native_terminal_framework_reported_not_assertion_verified' &&
        (!['completed','failed'].includes(result.phase) ||
         result.requiresReconciliation!==false ||
         result.timedOut!==false ||
         !Number.isSafeInteger(result.nativeExitCode))) ||
      (report.evidenceState==='native_not_reconciled_test_evidence_suppressed' &&
        report.cases.length!==0))
    return null;
  const safe=[];
  const seen=new Set();
  for(const row of report.cases) {
    if (!row || !CASE.test(row.caseId||'') || seen.has(row.caseId) ||
        row.reporter!==REPORTERS[result.recipeId] ||
        !STATUS.has(row.status) ||
        typeof row.name!=='string' ||
        row.name.length<1 || row.name.length>240 ||
        /[\u0000-\u001f\u007f]/.test(row.name) ||
        row.navigationVerified!==false ||
        row.assertionIndependentlyVerified!==false) return null;
    const source=anchor(row.source);
    if(source===undefined || (source && row.status!=='reported_fail')) return null;
    seen.add(row.caseId);
    safe.push(Object.freeze({
      id:row.caseId,name:row.name,status:row.status,
      reporter:row.reporter,source,
      independentlyVerified:false,
    }));
  }
  return Object.freeze({
    projectId,jobId:report.jobId,recipeId:report.recipeId,
    planSha256:report.planSha256,
    terminalReceiptSha256:report.terminalReceiptSha256,
    cases:Object.freeze(safe),
    observed:report.caseCountObserved,
    captureTruncated:report.captureTruncated,
    casesTruncated:report.casesTruncated,
    state:report.evidenceState,
    independentlyVerified:false,installedAccepted:false,
    taskAcceptanceAuthorized:false,
  });
}

/** Reject mismatched or server-invented paths even after native IPC. */
export function reviewedTestFailureTarget(report,caseId,response) {
  const entry=report?.cases?.find(row=>row.id===caseId);
  const expected=entry?.source;
  if (!expected || entry.status!=='reported_fail' ||
      response?.projectId!==report.projectId ||
      response.jobId!==report.jobId ||
      response.planSha256!==report.planSha256 ||
      response.terminalReceiptSha256!==report.terminalReceiptSha256 ||
      response.caseId!==caseId ||
      response.relativePath!==expected.relativePath ||
      response.diskSha256!==expected.diskSha256 ||
      response.line!==expected.line ||
      response.characterUtf16!==expected.characterUtf16 ||
      response.savedSourceVerified!==true ||
      response.testAssertionIndependentlyVerified!==false ||
      response.taskAccepted!==false) return null;
  return Object.freeze({...expected,projectId:report.projectId,
    jobId:report.jobId,caseId});
}
