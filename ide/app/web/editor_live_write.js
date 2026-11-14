// IDE-INT-01A: one source of truth for interactive editor write decisions.
// React's tab.draft/dirty can be a render behind CodeMirror. A user Save,
// Review, Close or Reload must bind the CURRENT editor text to its exact
// buffer ID, disk base and project, and must never silently switch instances.
// This helper has no filesystem, native IPC, network or storage side effects.
const SHA256 = /^[a-f0-9]{64}$/i;

export function captureLiveEditorIntent(tab, identity, editorState, projectId) {
  if (!tab || !identity || !editorState ||
      typeof editorState.sliceDoc !== 'function' ||
      typeof tab.key !== 'string' ||
      tab.key !== tab.projectId + ':' + tab.relativePath ||
      !tab.projectId || tab.projectId !== projectId ||
      identity.projectId !== tab.projectId ||
      identity.relativePath !== tab.relativePath ||
      typeof identity.bufferId !== 'string' ||
      !Number.isSafeInteger(identity.revision) || identity.revision < 0 ||
      !SHA256.test(tab.revision ?? '') ||
      identity.savedRevision !== tab.revision ||
      typeof tab.savedText !== 'string') {
    return { status: 'stale_editor_identity' };
  }
  const text = editorState.sliceDoc();
  if (typeof text !== 'string') return { status: 'editor_text_unavailable' };
  return Object.freeze({
    status: 'current',
    key: tab.key, projectId: tab.projectId, relativePath: tab.relativePath,
    savedRevision: tab.revision, savedText: tab.savedText,
    bufferId: identity.bufferId, bufferRevision: identity.revision,
    text, dirty: text !== tab.savedText,
  });
}

export function stillCurrentEditorIntent(intent, tab, identity, editorState, projectId) {
  if (intent?.status !== 'current') return false;
  const current = captureLiveEditorIntent(tab, identity, editorState, projectId);
  return current.status === 'current' &&
    current.key === intent.key &&
    current.projectId === intent.projectId &&
    current.relativePath === intent.relativePath &&
    current.savedRevision === intent.savedRevision &&
    current.savedText === intent.savedText &&
    current.bufferId === intent.bufferId &&
    current.bufferRevision === intent.bufferRevision &&
    current.text === intent.text;
}

// After native Save returns, *only* the original tab's saved base advances.
// Typing during Save is retained as a newer dirty live CodeMirror draft.
// A replaced tab, changed file base, or missing editor state is never rebased.
export function reconcileSavedEditorTab(tab, started, savedRevision,
    currentIdentity, editorState) {
  if (!tab || started?.status !== 'current' ||
      !SHA256.test(savedRevision ?? '') ||
      tab.key !== started.key ||
      tab.projectId !== started.projectId ||
      tab.relativePath !== started.relativePath ||
      tab.revision !== started.savedRevision ||
      tab.savedText !== started.savedText ||
      currentIdentity?.bufferId !== started.bufferId ||
      currentIdentity?.projectId !== started.projectId ||
      currentIdentity?.relativePath !== started.relativePath ||
      currentIdentity?.savedRevision !== savedRevision ||
      typeof editorState?.sliceDoc !== 'function') return null;
  const latestText = editorState.sliceDoc();
  if (typeof latestText !== 'string') return null;
  return {
    ...tab,
    revision: savedRevision,
    savedText: started.text,
    draft: latestText,
    dirty: latestText !== started.text,
    bufferRevision: (tab.bufferRevision ?? 0) + 1,
  };
}
