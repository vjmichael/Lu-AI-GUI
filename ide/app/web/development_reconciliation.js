// Wave C4: advisory evidence reconciliation from existing native observations.
// A UI label or historical event is never proof that a build, test, or installer ran.
const cleanCount = value => Number.isSafeInteger(value) && value >= 0 ? value : null;
const observedDigest = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const row = (id, title, state, provenance, detail) =>
  ({ id, title, state, provenance, detail });

export function reconcileDevelopmentEvidence(projectId, context, comparison) {
  if (typeof projectId !== 'string' || !projectId ||
      context?.projectId !== projectId || comparison?.projectId !== projectId ||
      !context.gitWorktree || !context.inventory ||
      !Array.isArray(context.inventory.files) || !Array.isArray(context.events) ||
      !Array.isArray(context.documents) || !Array.isArray(comparison.entries)) {
    throw Error('EVIDENCE_PROJECT_CONTEXT_MISMATCH');
  }

  const git = context.gitWorktree;
  const inventory = context.inventory;
  const files = inventory.files;
  const countsKnown = cleanCount(git.modifiedOrStagedCount) !== null &&
    cleanCount(git.untrackedCount) !== null;
  const localInspected = git.state === 'local_repository_inspected' && countsKnown;
  let source = 'unverified';
  if (localInspected && git.worktree === 'dirty') source = 'observed_dirty';
  if (localInspected && git.worktree === 'clean' &&
      git.modifiedOrStagedCount === 0 && git.untrackedCount === 0) {
    source = 'observed_clean';
  }
  const sourceNote = source === 'observed_clean'
    ? 'The local native Git reader observed no changes at read time; this does not certify a build.'
    : source === 'observed_dirty'
      ? 'The local native Git reader observed working-tree changes; preserve unique local work.'
      : 'Local working-tree state could not be confirmed from consistent native observations.';
  const hashed = files.filter(file => observedDigest(file.sha256)).length;
  const scannedComplete = inventory.discoveryComplete === true;

  let comparisonState = 'not_recorded';
  let comparisonDetail = 'No historical artifact baseline was recorded for this registered project.';
  if (comparison.baselinePresent === true) {
    const states = comparison.entries.map(entry => entry.status);
    const differing = states.some(s => [
      'changed_bytes', 'changed_size_only', 'missing_in_scanned_scope',
      'new_in_scanned_scope',
    ].includes(s));
    const uncertain = states.some(s => s !== 'matched_bytes');
    comparisonState = differing ? 'observed_differences' :
      uncertain || !scannedComplete || comparison.discoveryComplete !== true
        ? 'partial_comparison'
        : states.length > 0 ? 'matching_observations' : 'no_comparable_files';
    comparisonDetail = differing
      ? 'Known scanned artifacts differ from the recorded baseline; unscanned resources remain unknown.'
      : comparisonState === 'matching_observations'
        ? 'Compared digests matched within the bounded inspected scope; no executable was tested.'
        : 'A baseline exists but not all artifact bytes can be compared conclusively.';
  }

  return {
    schemaVersion: 1,
    projectId,
    authority: 'informational_only_no_permission_grant',
    readConsistency: 'separate_non_atomic_native_observations',
    checks: [
      row('local_source', 'Local source', source, 'native_libgit2',
        sourceNote + (git.truncated ? ' Change list was truncated.' : '')),
      row('tracking', 'Stored upstream', git.upstreamHead ? 'locally_recorded' : 'unverified',
        'local_git_tracking_ref', 'Stored tracking references may be stale and do not prove the live remote.'),
      row('live_remote', 'Live GitHub', 'not_checked', 'not_queried',
        'This reconciliation never contacts GitHub; use an explicit remote comparison separately.'),
      row('artifact_bytes', 'Project artifacts',
        !scannedComplete ? 'partial_scan' :
        !files.length ? 'none_in_scanned_scope' :
        hashed ? 'bytes_observed_not_executed' : 'present_unhashed',
        'bounded_native_project_inventory',
        String(files.length) + ' listed in known folders; ' + String(hashed) +
        ' SHA-256 digest(s). Scanned folders do not cover the entire machine.'),
      row('artifact_baseline', 'Recorded artifact baseline', comparisonState,
        'ide_app_profile_snapshot_vs_current_scan', comparisonDetail),
      row('task_claims', 'Task completion claims', 'not_verified',
        'untrusted_project_documents',
        String(context.documents.length) +
        ' orientation document slot(s); project notes are context, not test or acceptance receipts.'),
      row('host_events', 'Recorded IDE actions', context.events.length ? 'historical_events' : 'no_events',
        'local_development_ledger',
        String(context.events.length) + ' recent event(s) observed; recorded actions do not prove external effects.'),
      row('native_ci', 'Native build and CI', 'not_inspected',
        'no_build_receipts_in_context',
        'No exact-head CI job receipt is ingested by this view. A source commit is not a passed test.'),
      row('installed_app', 'Installed desktop acceptance', 'not_inspected',
        'no_installation_receipt_in_context',
        'A present executable or installer hash does not prove installation, launch or UI behavior.'),
      row('toolchain', 'Toolchain usability', 'not_tested',
        'no_supervised_worker',
        'Discovered compiler or package files are not run, version-checked or trusted.'),
    ],
    limitations: [
      'Observations can change immediately and are not an atomic source or filesystem snapshot.',
      'No project-authored document contents or remembered completion claim are authority.',
      'This read-only report cannot certify CI, installed Windows acceptance or runnable toolchains.',
    ],
  };
}
