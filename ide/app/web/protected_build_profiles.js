/**
 * IDE-INT03: display-only eligibility, one saved-manifest identity and
 * conservative original OS receipt interpretation.
 * A ready profile is NOT a grant. Native Run still freezes and rechecks the
 * exact manifest/source tree and requires one-use human OS authorization.
 */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const SHA=/^[0-9a-f]{64}$/;
const HEAD=/^[0-9a-f]{40}$/;
const JOB=/^job-[0-9a-f]{32}$/;
const CARGO=new Set(['rust_check','rust_build','rust_tests','lu_ide_host_tests']);
const MULTILANG=new Set(['python_pytest','js_tests','cpp_cmake_build']);
const PROFILES=new Set([...CARGO,...MULTILANG]);
const PHASES=new Set(['completed','cancelled','failed','denied',
  'recovery_required','uncertain','running','cancel_requested',
  'release_pending','authorized','admission_pending','prepared']);

export function nativeProfileAvailability(nativeStatus, profileId) {
  if (!PROFILES.has(profileId)) return 'not_allowlisted';
  if (!nativeStatus || nativeStatus.ready!==true ||
      !Array.isArray(nativeStatus.quarantined) ||
      !Array.isArray(nativeStatus.journal) ||
      nativeStatus.current ||
      nativeStatus.globalRecoveryRequired!==false ||
      nativeStatus.activeInAnotherProject!==false ||
      nativeStatus.globalWorkerBusy!==false ||
      nativeStatus.quarantined.some(row=>row?.requiresReconciliation===true) ||
      nativeStatus.journal.some(row=>row?.requiresReconciliation===true))
    return 'worker_unavailable_or_recovery_required';
  if (CARGO.has(profileId))
    return nativeStatus.cargoReady===true ? 'reviewable_native' :
      'cargo_toolchain_not_verified';
  const providers=nativeStatus.multilangReady;
  if (!Array.isArray(providers)||providers.length>4) return 'adapter_status_unverified';
  const matching=providers.filter(row=>row?.profileId===profileId);
  return matching.length===1 && matching[0].ready===true &&
    matching[0].code==='DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB'
    ? 'reviewable_native' : 'adapter_runtime_not_approved';
}

/**
 * Returned object contains no process args, environment, executable, or grant.
 * It names the exact fixed recipe and independently observed saved-manifest SHA.
 */
export function planProtectedBuildProfile(catalog,status,projectId,profileId) {
  if (!PROJECT.test(projectId||'') || catalog?.projectId!==projectId ||
      !Array.isArray(catalog.profiles) || !PROFILES.has(profileId))
    throw Error('INT03_PROJECT_OR_PROFILE_STALE');
  const profile=catalog.profiles.find(row=>row?.id===profileId);
  if (!profile || profile.manifestState!=='saved_file_sha_observed_not_validated' ||
      typeof profile.sha!=='string' || !SHA.test(profile.sha) ||
      (CARGO.has(profileId) && profile.existingRecipe!==profileId) ||
      (MULTILANG.has(profileId) && profile.existingRecipe!==null))
    throw Error('INT03_SAVED_MANIFEST_UNAVAILABLE');
  const availability=nativeProfileAvailability(status,profileId);
  if (availability!=='reviewable_native')
    throw Error('INT03_'+availability.toUpperCase());
  return Object.freeze({
    projectId, recipeId:profileId,
    expectedManifestSha256:profile.sha,
    manifestRelativePath:profile.manifest,
    source:'read_only_native_profile_observation',
    executionAuthorized:false,
  });
}

export function isCurrentProtectedBuildPlan(plan,catalog,status,projectId) {
  if (!plan || plan.projectId!==projectId ||
      plan.executionAuthorized!==false ||
      plan.source!=='read_only_native_profile_observation' ||
      !SHA.test(plan.expectedManifestSha256||'')) return false;
  try {
    const current=planProtectedBuildProfile(
      catalog,status,projectId,plan.recipeId);
    return current.expectedManifestSha256===plan.expectedManifestSha256 &&
      current.manifestRelativePath===plan.manifestRelativePath;
  } catch { return false; }
}

/**
 * Only a matching, original current-process native receipt can be called a
 * completed protected run. Not CI proof, installed acceptance, or test
 * assertion count. Never infer a result from stdout or a previous journal.
 */
export function protectedBuildOutcome(reply,plan,projectId) {
  if (!plan || !reply || plan.projectId!==projectId ||
      reply.projectId!==projectId ||
      reply.recipeId!==plan.recipeId ||
      reply.manifestSha256!==plan.expectedManifestSha256 ||
      !JOB.test(reply.jobId||'') ||
      !SHA.test(reply.planSha256||'') ||
      !SHA.test(reply.inputSnapshotSha256||'') ||
      !SHA.test(reply.toolchainSha256||'') ||
      !SHA.test(reply.terminalReceiptSha256||'') ||
      !HEAD.test(reply.sourceHead||'') ||
      !PHASES.has(reply.phase) ||
      typeof reply.requiresReconciliation!=='boolean' ||
      typeof reply.timedOut!=='boolean' ||
      reply.installedAppAccepted!==false ||
      !(reply.nativeExitCode===null || Number.isSafeInteger(reply.nativeExitCode)))
    throw Error('INT03_NATIVE_RECEIPT_OR_SCOPE_DENIED');
  const clean=reply.phase==='completed' &&
    reply.nativeExitCode===0 && !reply.requiresReconciliation &&
    reply.timedOut===false;
  const disposition=reply.requiresReconciliation ? 'native_recovery_required' :
    clean ? 'native_completed_exit_zero' :
    reply.phase==='cancelled' ? 'native_cancelled' :
    reply.phase==='failed' || (reply.phase==='completed' && reply.nativeExitCode!==0)
      ? 'native_failed_or_nonzero' : 'native_incomplete_or_unverified';
  return Object.freeze({
    projectId,recipeId:plan.recipeId,jobId:reply.jobId,
    manifestSha256:reply.manifestSha256,
    planSha256:reply.planSha256,
    originalTerminalReceiptSha256:reply.terminalReceiptSha256,
    phase:reply.phase,disposition,
    nativeCompleted:clean,
    installedAccepted:false,
    testAssertionsVerified:false,
    taskAcceptanceAuthorized:false,
  });
}
