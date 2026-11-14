/**
 * DEV-03.5: projection ONLY of already-native project-scoped job evidence.
 * No outcome, authorization, kernel cleanup or installed acceptance is
 * inferred from stdout, source discovery, elapsed time or a UI status flag.
 */
const PROJECT = /^ide-[a-f0-9]{32}$/;
const JOB = /^job-[a-f0-9]{32}$/;
const SHA = /^[a-f0-9]{64}$/;
const terminal = new Set(['completed','failed','denied','cancelled']);
const phases = new Set(['prepared','authorized','admission_pending','running',
  'release_pending','cancel_requested','cancelled','completed','failed',
  'denied','recovery_required','uncertain']);
const RECIPES = new Set(['rust_check','rust_build','rust_tests','lu_ide_host_tests',
  'python_pytest','js_tests','cpp_cmake_build']);

function validRecord(r, projectId) {
  return r && r.projectId === projectId && JOB.test(r.jobId) &&
    SHA.test(r.planSha256) && phases.has(r.phase) &&
    typeof r.requiresReconciliation === 'boolean';
}
function validNativeResult(r, projectId, record) {
  return !!(r && r.projectId === projectId &&
    r.jobId === record.jobId && r.planSha256 === record.planSha256 &&
    r.phase === record.phase && terminal.has(r.phase) &&
    r.requiresReconciliation === false &&
    RECIPES.has(r.recipeId) &&
    // Source HEAD is exactly a Git SHA-1 (40 hex); do not accept 64 hex.
    /^[a-f0-9]{40}$/.test(r.sourceHead) &&
    r.installedAppAccepted === false &&
    (r.nativeExitCode === null || Number.isInteger(r.nativeExitCode)));
}

/**
 * Status response is a native view; the durable project-scoped journal is
 * authoritative for phase. After restart the native runtime result is lost,
 * so completed journals are NEVER promoted into a fresh verified run.
 */
export function projectJobLifecycle(status, projectId, output) {
  if (!PROJECT.test(projectId) || !status ||
      !Array.isArray(status.journal) || status.journal.length > 32 ||
      !Array.isArray(status.quarantined) || status.quarantined.length > 1024 ||
      !Array.isArray(status.recent) || status.recent.length > 32)
    return Object.freeze({kind:'unavailable',projectId,jobs:[],current:null});
  const records = new Map();
  // A journal can be a bounded view; quarantined entries must still be
  // surfaced even when older than the journal's 32-item display window.
  for (const raw of [...status.journal,...status.quarantined]) {
    if (!validRecord(raw,projectId)) continue;
    const last = records.get(raw.jobId);
    if (!last || raw.requiresReconciliation) records.set(raw.jobId,raw);
  }
  const entries=[];
  const recent = new Map(status.recent.filter(r=>
    r && r.projectId === projectId && JOB.test(r.jobId)).map(r=>[r.jobId,r]));
  for (const [jobId,record] of records) {
    const active=status.current?.projectId === projectId &&
      status.current?.jobId === jobId ? status.current : null;
    const quarantined=record.requiresReconciliation;
    let state;
    if (quarantined) state='quarantined_os_reconciliation_required';
    else if (active?.cancellationRequested) state='cancellation_requested_not_yet_reaped';
    else if (active) state='native_job_in_progress';
    else if (record.phase==='cancelled') state='kernel_reconciled_or_cancelled';
    else if (record.phase==='completed') state='durable_completed_runtime_proof_not_replayed';
    else if (terminal.has(record.phase)) state='durable_terminal_not_success';
    else state='interrupted_or_nonterminal_journal';
    const run=recent.get(jobId);
    const nativeResult=validNativeResult(run,projectId,record);
    const matchedOutput=output?.projectId===projectId &&
      output?.jobId===jobId && output?.liveUnverified===true;
    entries.push(Object.freeze({
      jobId, planSha256:record.planSha256, phase:record.phase,
      requiresReconciliation:quarantined,state,
      nativeResult, // only in this process, never an installed acceptance claim
      outputObserved:!!matchedOutput,
      outputTruncated:matchedOutput && output.truncated===true,
      installedAccepted:false,
      canReconcile:quarantined && !status.current,
      canCancel:!!active && !active.cancellationRequested,
    }));
  }
  const current = status.current?.projectId===projectId &&
      JOB.test(status.current?.jobId) ? status.current : null;
  if (current && !records.has(current.jobId)) {
    // The worker may have entered preflight before supervisor wrote a
    // durable record. Display only a request, not verified execution.
    entries.push(Object.freeze({
      jobId:current.jobId,planSha256:null,phase:'preflight_unverified',
      requiresReconciliation:false,
      state:current.cancellationRequested
        ? 'cancellation_requested_not_yet_reaped'
        : 'native_preflight_not_yet_journal_observed',
      nativeResult:false,outputObserved:false,outputTruncated:false,
      installedAccepted:false,canReconcile:false,
      canCancel:!current.cancellationRequested,
    }));
  }
  return Object.freeze({kind:'native_journal_projection',projectId,
    jobs:Object.freeze(entries.slice(-64)),
    current:current?.jobId ?? null});
}
