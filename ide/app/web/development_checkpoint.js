import { reconcileDevelopmentEvidence } from './development_reconciliation.js';

// Wave C3: portable, read-only context from fresh host-backed observations.
// Intentionally excludes source text, local absolute paths, and effect authority.
const MAX_FILES = 24;
const MAX_EVENTS = 20;
const MAX_DOCS = 12;
const MAX_CHANGES = 24;

function limitedText(value, max = 240) {
  return typeof value === 'string' ? value.slice(0, max) : null;
}

function relativePath(path) {
  if (typeof path !== 'string' || !path || path.length > 1024 ||
      path.startsWith('/') || path.includes('\\') || path.includes('\0') ||
      path.split('/').some(part => !part || part === '.' || part === '..' || part.includes(':'))) {
    return null;
  }
  return path;
}

function items(values, limit, mapper) {
  const array = Array.isArray(values) ? values : [];
  return { totalObserved: array.length, exported: array.slice(0, limit).map(mapper),
    omittedFromExport: Math.max(0, array.length - limit) };
}

export function buildDevelopmentCheckpoint(projectId, context, comparison, copiedAt) {
  if (typeof projectId !== 'string' || !projectId ||
      context?.projectId !== projectId || comparison?.projectId !== projectId ||
      !context.gitWorktree || !context.inventory || !Array.isArray(context.documents) ||
      !Array.isArray(context.events) || !Array.isArray(context.inventory.files)) {
    throw Error('CHECKPOINT_PROJECT_CONTEXT_MISMATCH');
  }
  if (typeof copiedAt !== 'string' || !Number.isFinite(Date.parse(copiedAt))) {
    throw Error('CHECKPOINT_INVALID_COPY_TIME');
  }
  const git = context.gitWorktree;
  const inventory = context.inventory;
  return {
    format: 'lu-ide-development-checkpoint', schemaVersion: 1,
    copiedAt, observationKind: 'separate_bounded_host_reads_not_atomic',
    project: { projectId, displayName: limitedText(context.projectName, 120) },
    git: {
      provenance: 'local_libgit2_inspection', state: limitedText(git.state, 80),
      branch: limitedText(git.branch, 160), localHead: limitedText(git.localHead, 40),
      worktree: limitedText(git.worktree, 80),
      modifiedOrStagedCount: git.modifiedOrStagedCount,
      untrackedCount: git.untrackedCount,
      changesTruncatedByHost: Boolean(git.truncated),
      storedUpstreamHead: limitedText(git.upstreamHead, 40),
      storedUpstreamComparison: limitedText(git.upstreamComparison, 80),
      commitsAheadOfStoredUpstream: git.commitsAhead ?? null,
      commitsBehindStoredUpstream: git.commitsBehind ?? null,
      liveRemote: 'not_queried_for_this_checkpoint',
      changedPaths: items(git.displayedChanges, MAX_CHANGES, entry => ({
        path: relativePath(entry.relativePath), kind: limitedText(entry.kind, 80),
        stage: limitedText(entry.stage, 80),
      })),
    },
    evidenceReconciliation: reconcileDevelopmentEvidence(projectId, context, comparison),
    resources: {
      provenance: 'bounded_local_project_inventory',
      discoveryComplete: inventory.discoveryComplete === true,
      artifactReadiness: 'not_tested',
      files: items(inventory.files, MAX_FILES, file => ({
        path: relativePath(file.relativePath), category: limitedText(file.category, 80),
        sizeBytes: file.sizeBytes, sha256: limitedText(file.sha256, 64),
        state: limitedText(file.state, 80), digestStatus: limitedText(file.digestStatus, 80),
      })),
      recordedBaseline: {
        present: comparison.baselinePresent === true,
        count: comparison.baselineCount ?? 0,
        observedUnixSeconds: comparison.baselineUnixSeconds ?? null,
        discoveryComplete: comparison.discoveryComplete === true,
        localGitHeadChange: limitedText(comparison.localGitHeadChange, 80),
        gitWorktreeChange: limitedText(comparison.gitWorktreeChange, 80),
        liveRemote: 'not_queried_for_this_checkpoint',
        changes: items(comparison.entries, MAX_FILES, entry => ({
          path: relativePath(entry.relativePath), status: limitedText(entry.status, 80),
          baselineSha256: limitedText(entry.baselineSha256, 64),
          currentSha256: limitedText(entry.currentSha256, 64),
        })),
      },
    },
    hostEvents: items(context.events, MAX_EVENTS, event => ({
      eventId: limitedText(event.eventId, 80),
      observedUnixSeconds: event.observedUnixSeconds,
      action: limitedText(event.operation, 80), outcome: limitedText(event.outcome, 80),
      sourcePath: event.sourcePath ? relativePath(event.sourcePath) : null,
      savedRevision: limitedText(event.sourceRevision, 64),
    })),
    orientationDocuments: items(context.documents, MAX_DOCS, doc => ({
      path: relativePath(doc.relativePath), availability: limitedText(doc.availability, 80),
      trust: 'untrusted_project_document', savedRevision: limitedText(doc.sourceRevision, 64),
      excerptExcluded: true,
    })),
    limitations: [
      'This export is not a source snapshot, execution receipt, installer test, or authorization.',
      'No source/document text, project root, absolute filesystem paths, secrets or live GitHub content are included.',
      'Host reads are bounded and non-atomic. Git and local files may change immediately after inspection.',
      'Saved Git tracking refs do not establish current remote state or ancestry.',
      'A file digest verifies observed bytes only; toolchain and installer usability are not tested.',
    ],
  };
}
