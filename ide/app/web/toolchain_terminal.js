/**
 * IDE-INT05 — presentation-only toolchain, package and terminal boundaries.
 * Each native runtime probe is a momentary pin observation, not a grant.
 * Output is untrusted ephemeral text from an existing OS-supervised job,
 * never an interactive shell or durable execution evidence.
 */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const JOB=/^job-[a-f0-9]{32}$/;
const SHA=/^[a-f0-9]{64}$/;
const RECIPE=new Set([
  'rust_check','rust_build','rust_tests','lu_ide_host_tests',
  'python_pytest','js_tests','cpp_cmake_build','uv_sync_offline',
]);
const RUNTIMES=Object.freeze([
  {profileId:'rust_cargo',label:'Rust / Cargo'},
  {profileId:'python_pytest',label:'Python / pytest'},
  {profileId:'js_tests',label:'Node.js tests'},
  {profileId:'cpp_cmake_build',label:'C/C++ / CMake'},
  {profileId:'uv_sync_offline',label:'Python / uv offline Sync'},
]);
const MULTILANG=new Set(RUNTIMES.slice(1).map(row=>row.profileId));
const MAX_CHUNKS=256;
const MAX_TEXT=65536;
const STREAMS=new Set(['stdout','stderr','combined']);

function unavailable(projectId) {
  return Object.freeze({projectId,kind:'unavailable',rows:[],
    blockedByRecovery:true,executionAuthorized:false});
}
function safeCode(code) {
  return typeof code==='string' && /^[A-Z0-9_]{1,100}$/.test(code);
}
function checkedProvenance(row,profileId) {
  if (!row || row.profileId!==profileId ||
      !['installed_bytes_match_compile_time_release_pin',
        'unavailable_or_not_independently_verified']
        .includes(row.provenanceState) ||
      row.executionAuthorized!==false ||
      row.installedAppAccepted!==false ||
      row.versionEvidence!=='not_independently_verified' ||
      row.publisherEvidence!=='not_independently_verified' ||
      row.licenseEvidence!=='not_independently_verified') return null;
  const sha=row.observedSha256;
  const pin=row.releasePinSha256;
  if (sha!==null && !SHA.test(sha||'')) return null;
  if (pin!==null && !SHA.test(pin||'')) return null;
  const verified=row.provenanceState===
    'installed_bytes_match_compile_time_release_pin';
  if (verified !== (sha!==null && sha===pin)) return null;
  return Object.freeze({
    verified, observedSha256:sha,releasePinSha256:pin,
    provenanceState:row.provenanceState,
    versionVerified:false,publisherVerified:false,licenseVerified:false,
  });
}

/** The native build runtime, not PATH or an untrusted project manifest, probes pins. */
export function checkedToolchainInventory(status,projectId) {
  if (!PROJECT.test(projectId||'') || !status ||
      typeof status.ready!=='boolean' ||
      typeof status.cargoReady!=='boolean' ||
      !Array.isArray(status.multilangReady) ||
      status.multilangReady.length!==MULTILANG.size ||
      !Array.isArray(status.journal) || !Array.isArray(status.quarantined) ||
      !Array.isArray(status.recent) || !safeCode(status.code))
    return unavailable(projectId);
  const found=new Map();
  for (const row of status.multilangReady) {
    if (!row || !MULTILANG.has(row.profileId) ||
        found.has(row.profileId) || typeof row.ready!=='boolean' ||
        !safeCode(row.code)) return unavailable(projectId);
    found.set(row.profileId,row);
  }
  if (found.size!==MULTILANG.size) return unavailable(projectId);
  const cargoProvenance=checkedProvenance(status.cargoProvenance,'rust_cargo');
  if (!cargoProvenance) return unavailable(projectId);
  const nativeProvenance=new Map([['rust_cargo',cargoProvenance]]);
  for (const [profileId,row] of found) {
    const verified=checkedProvenance(row.provenance,profileId);
    if (!verified) return unavailable(projectId);
    nativeProvenance.set(profileId,verified);
  }
  // Global process/recovery custody cannot be bypassed by switching projects.
  const blockedByRecovery=status.quarantined.length>0 ||
    status.journal.some(entry=>entry?.requiresReconciliation===true);
  const busy=!!status.current;
  const rows=RUNTIMES.map(entry=>{
    const native=entry.profileId==='rust_cargo'
      ? {ready:status.cargoReady,code:status.cargoReady
          ? 'DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB'
          : status.code}
      : found.get(entry.profileId);
    const proof=nativeProvenance.get(entry.profileId);
    const ready=native.ready===true && proof.verified;
    // The runtime cannot be presented as ready without evidence of actual
    // installed bytes matching the exact reviewed compile-time pin.
    if(native.ready===true && !proof.verified) return null;
    return Object.freeze({
      ...entry,...proof,ready,
      code:native.code,
      disposition:!ready?'native_runtime_not_available':
        blockedByRecovery?'reconciliation_required':
        busy?'native_worker_busy':
        'pinned_runtime_observed_separate_os_consent_required',
      canInspect:true,canInstall:false,canStartWithoutConsent:false,
      versionVerified:false,executionAuthorized:false,
    });
  });
  if(rows.some(row=>row===null)) return unavailable(projectId);
  return Object.freeze({
    projectId,kind:'native_pin_observation',
    rows:Object.freeze(rows),blockedByRecovery,
    executionAuthorized:false,installedAppAccepted:false,
  });
}

/** Manager identification and runtime readiness are independent evidence. */
export function checkedPackageToolchainReadiness(inspection,inventory,projectId) {
  if (!PROJECT.test(projectId||'') ||
      inspection?.projectId!==projectId ||
      !SHA.test(inspection?.fingerprint||'') ||
      inspection?.executionAuthorized!==false ||
      inspection?.applySupported!==false ||
      inspection?.networkAccessed!==false ||
      !Array.isArray(inspection.candidates) ||
      inventory?.kind!=='native_pin_observation' ||
      inventory.projectId!==projectId) return Object.freeze([]);
  const uv=inventory.rows.find(row=>row.profileId==='uv_sync_offline');
  const ids=new Set();
  const results=[];
  for (const c of inspection.candidates) {
    if (!c || !['uv','vcpkg','conan'].includes(c.managerId) ||
        ids.has(c.managerId) || typeof c.state!=='string' ||
        c.executionAuthorized!==false ||
        c.applySupported!==false || c.resolutionSupported!==false)
      return Object.freeze([]);
    ids.add(c.managerId);
    const candidate=c.state==='candidate_needs_review';
    const offlineUv=c.managerId==='uv' && candidate &&
      uv?.ready===true && !inventory.blockedByRecovery;
    results.push(Object.freeze({
      managerId:c.managerId,
      state:c.state,
      reviewAvailable:candidate,
      nativeOfflineSyncAvailable:!!offlineUv,
      // Actual execution still requires NEW native revalidation and a one-use dialog.
      installAuthorized:false,
      disposition:!candidate?'manifest_not_ready':
        c.managerId!=='uv'?'review_only_no_apply':
        !uv?.ready?'offline_runtime_not_available':
        inventory.blockedByRecovery?'reconciliation_required':
        'runtime_observed_fresh_native_review_required',
    }));
  }
  return Object.freeze(results);
}

/**
 * Never promote process text into a terminal result. A current job may stream
 * before its journal entry is written; those bytes remain expressly unverified.
 * Historical journals after restart do not resurrect an output stream.
 */
export function checkedProtectedTerminal(status,output,projectId) {
  const empty=Object.freeze({projectId,kind:'unavailable',jobId:null,
    chunks:[],truncated:false,terminalReceiptSha256:null,
    executionAuthorized:false,interactive:false,installedAppAccepted:false});
  if (!PROJECT.test(projectId||'') ||
      !status || !Array.isArray(status.journal) ||
      !Array.isArray(status.recent) ||
      !Array.isArray(status.quarantined) ||
      output?.projectId!==projectId || !JOB.test(output?.jobId||'') ||
      output?.liveUnverified!==true ||
      !Array.isArray(output.chunks) || output.chunks.length>MAX_CHUNKS ||
      !Number.isSafeInteger(output.lastSequence) ||
      output.lastSequence<0 || typeof output.truncated!=='boolean') return empty;
  const current=status.current?.projectId===projectId &&
    status.current?.jobId===output.jobId &&
    RECIPE.has(status.current?.recipeId);
  const journal=status.journal.find(row=>
    row?.projectId===projectId && row.jobId===output.jobId &&
    SHA.test(row.planSha256||'') && typeof row.phase==='string');
  const last=status.lastOutputJobId===output.jobId && !!journal;
  if (!current && !last) return empty;
  if (status.quarantined.some(row=>row?.jobId===output.jobId &&
      row.projectId===projectId)) return empty;
  let sequence=0,bytes=0;
  const chunks=[];
  for(const row of output.chunks) {
    if(!row || !Number.isSafeInteger(row.sequence) ||
       row.sequence<=sequence || row.sequence>output.lastSequence ||
       !STREAMS.has(row.stream) || typeof row.text!=='string' ||
       row.text.length>8192) return empty;
    sequence=row.sequence;
    // Keep terminal escape codes, C0/C1 controls and bidi overrides inert
    // even when a future renderer ceases using React text nodes.
    const clean=row.text.replace(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g,'�');
    bytes+=clean.length;
    if(bytes>MAX_TEXT) return empty;
    chunks.push(Object.freeze({sequence,stream:row.stream,text:clean}));
  }
  // An independently checked terminal receipt can label an original
  // completed process; it NEVER authenticates the content of stdout/stderr.
  const recent=status.recent.find(row=>row?.projectId===projectId &&
    row.jobId===output.jobId && row.planSha256===journal?.planSha256 &&
    RECIPE.has(row.recipeId) &&
    ['completed','failed','cancelled','denied'].includes(row.phase) &&
    row.phase===journal?.phase && row.requiresReconciliation===false &&
    row.installedAppAccepted===false &&
    SHA.test(row.terminalReceiptSha256||''));
  return Object.freeze({
    projectId,kind:recent?'ephemeral_stream_with_native_receipt':
      current?'unverified_active_stream':'ephemeral_stream_without_reconciled_receipt',
    jobId:output.jobId,recipeId:recent?.recipeId||
      (current?status.current.recipeId:null),
    chunks:Object.freeze(chunks),
    truncated:output.truncated,
    terminalReceiptSha256:recent?.terminalReceiptSha256||null,
    receiptFromOriginalNativeProcess:!!recent,
    outputIndependentlyVerified:false,
    executionAuthorized:false,interactive:false,installedAppAccepted:false,
  });
}
