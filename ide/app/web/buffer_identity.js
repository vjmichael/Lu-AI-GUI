// Session-only identities for unsaved CodeMirror buffers. No native IPC or file writes.
const MAX_LINES = 120;
const MAX_BYTES = 32 * 1024;
const encoder = new TextEncoder();

export function newBufferIdentity(projectId, relativePath, savedRevision, bufferId) {
  if (!projectId || !relativePath || !/^[a-f0-9]{64}$/i.test(savedRevision) ||
      !/^[a-zA-Z0-9-]{8,128}$/.test(bufferId)) throw Error('INVALID_BUFFER_IDENTITY');
  return Object.freeze({ schemaVersion: 1, projectId, relativePath,
    savedRevision, bufferId, revision: 0 });
}

export function advanceBufferIdentity(identity) {
  if (!identity || !Number.isSafeInteger(identity.revision) ||
      identity.revision >= Number.MAX_SAFE_INTEGER) throw Error('BUFFER_REVISION_EXHAUSTED');
  return Object.freeze({ ...identity, revision: identity.revision + 1 });
}

function splitsSurrogate(doc, position) {
  if (position <= 0 || position >= doc.length) return false;
  const adjacent = doc.sliceString(position - 1, position + 1);
  return adjacent.length === 2 &&
    adjacent.charCodeAt(0) >= 0xD800 && adjacent.charCodeAt(0) <= 0xDBFF &&
    adjacent.charCodeAt(1) >= 0xDC00 && adjacent.charCodeAt(1) <= 0xDFFF;
}

/**
 * Rebase a successful disk Save only onto the editor instance that initiated
 * it. A reopened/staged/reloaded tab can reuse the same key and path but is a
 * different authority boundary; its buffer ID must not be rebound.
 */
export function rebaseBufferAfterSave(current, started, savedRevision) {
  if (!current || !started || current.bufferId !== started.bufferId ||
      current.projectId !== started.projectId ||
      current.relativePath !== started.relativePath ||
      current.savedRevision !== started.savedRevision ||
      !/^[a-f0-9]{64}$/i.test(savedRevision)) {
    return { status: 'replaced_buffer', identity: current ?? null };
  }
  return {
    status: 'rebased',
    identity: Object.freeze({
      ...advanceBufferIdentity(current), savedRevision,
    }),
  };
}

// CodeMirror columns/positions are UTF-16. Byte offsets are UTF-8;
// columns are one-based UTF-16 units, not Unicode grapheme indices.
export function captureBufferSpan(identity, editorState, from, to) {
  if (!identity || !editorState?.doc || !Number.isSafeInteger(from) ||
      !Number.isSafeInteger(to) || from < 0 || to < from ||
      to > editorState.doc.length) throw Error('INVALID_BUFFER_RANGE');
  if (splitsSurrogate(editorState.doc, from) ||
      splitsSurrogate(editorState.doc, to)) {
    throw Error('INVALID_BUFFER_RANGE');
  }
  const first = editorState.doc.lineAt(from);
  const last = editorState.doc.lineAt(to);
  if (last.number - first.number + 1 > MAX_LINES) throw Error('BUFFER_SPAN_TOO_LARGE');
  // CodeMirror counts a CRLF separator as one position. Never slice the
  // serialized JS string with editor offsets; use the Text tree's mapping.
  const text = editorState.sliceDoc(from, to);
  const length = encoder.encode(text).length;
  if (length > MAX_BYTES) throw Error('BUFFER_SPAN_TOO_LARGE');
  const byteStart = encoder.encode(editorState.sliceDoc(0, from)).length;
  return Object.freeze({
    schemaVersion: 1, projectId: identity.projectId,
    relativePath: identity.relativePath, bufferId: identity.bufferId,
    bufferRevision: identity.revision, baseDiskRevision: identity.savedRevision,
    fromUtf16: from, toUtf16: to, fromByte: byteStart, toByte: byteStart + length,
    startLine: first.number, startColumnUtf16: from - first.from + 1,
    endLine: last.number, endColumnUtf16: to - last.from + 1,
    text, lifetime: 'current_editor_session_only',
  });
}

export function resolveBufferSpan(ref, identity, state) {
  if (!ref || !identity || !state?.doc ||
      ref.bufferId !== identity.bufferId ||
      ref.bufferRevision !== identity.revision ||
      ref.projectId !== identity.projectId ||
      ref.relativePath !== identity.relativePath ||
      ref.baseDiskRevision !== identity.savedRevision) {
    return { status: 'stale_buffer_reference' };
  }
  try {
    const actual = captureBufferSpan(identity, state, ref.fromUtf16, ref.toUtf16);
    if (actual.text !== ref.text || actual.fromByte !== ref.fromByte ||
        actual.toByte !== ref.toByte) return { status: 'stale_buffer_reference' };
    return { status: 'current', span: actual };
  } catch {
    return { status: 'stale_buffer_reference' };
  }
}

export function formatBufferReference(span, sha256) {
  if (!/^[a-f0-9]{64}$/.test(sha256)) throw Error('INVALID_BUFFER_DIGEST');
  return 'ide-buffer:v1 project=' + span.projectId +
    ' path=' + JSON.stringify(span.relativePath) +
    ' buffer=' + span.bufferId + ' revision=' + span.bufferRevision +
    ' baseDisk=' + span.baseDiskRevision + ' draftSha256=' + sha256 +
    ' rangeUtf16=' + span.fromUtf16 + '..' + span.toUtf16 +
    ' lines=' + span.startLine + ':' + span.startColumnUtf16 +
    '..' + span.endLine + ':' + span.endColumnUtf16 +
    ' bytes=' + span.fromByte + '..' + span.toByte +
    ' lifetime=session-only';
}


/** A persistent reference resolves only against an exact immutable native
 * private draft checkpoint. It does not authorize project writes or expose
 * the actual source contents to a clipboard. */
export function formatPersistentDraftReference(record) {
  if (!record || record.schemaVersion !== 1 ||
      !/^[a-f0-9]{64}$/.test(record.checkpointId ?? '') ||
      !/^[a-f0-9]{64}$/.test(record.draftSha256 ?? '') ||
      !/^[a-f0-9]{64}$/.test(record.baseDiskRevision ?? '') ||
      !Number.isSafeInteger(record.bufferRevision) ||
      typeof record.projectId !== 'string' ||
      typeof record.relativePath !== 'string')
    throw Error('INVALID_PERSISTENT_DRAFT_REFERENCE');
  return 'ide-draft:v1 project=' + record.projectId +
    ' path=' + JSON.stringify(record.relativePath) +
    ' checkpoint=' + record.checkpointId +
    ' buffer=' + record.bufferId + ' revision=' + record.bufferRevision +
    ' baseDisk=' + record.baseDiskRevision +
    ' draftSha256=' + record.draftSha256 +
    ' lifetime=persistent-private-app-data';
}

export function draftRecoveryAssessment(record, projectId, path, diskSha) {
  if (!record || record.projectId !== projectId ||
      record.relativePath !== path || !/^[a-f0-9]{64}$/.test(diskSha))
    return 'untrusted_draft_reference';
  return record.baseDiskRevision === diskSha
    ? 'same_saved_base' : 'disk_revision_changed';
}


export function parsePersistentDraftReference(input) {
  if (typeof input !== 'string' || input.length > 2048) {
    throw Error('INVALID_PERSISTENT_DRAFT_REFERENCE');
  }
  // Reject interpolation, URI schemes, newlines and forged extra arguments.
  const match = /^ide-draft:v1 project=(ide-[a-f0-9]{32}) path=("(?:\\.|[^"\\])*") checkpoint=([a-f0-9]{64}) buffer=([a-zA-Z0-9-]{8,128}) revision=([0-9]+) baseDisk=([a-f0-9]{64}) draftSha256=([a-f0-9]{64}) lifetime=persistent-private-app-data$/.exec(input);
  if (!match) throw Error('INVALID_PERSISTENT_DRAFT_REFERENCE');
  let relativePath;
  try { relativePath = JSON.parse(match[2]); }
  catch { throw Error('INVALID_PERSISTENT_DRAFT_REFERENCE'); }
  if (typeof relativePath !== 'string' || relativePath.length > 1024 ||
      relativePath.startsWith('/') || relativePath.includes('\\') ||
      relativePath.split('/').some(part => !part || part==='.' || part==='..') ||
      !Number.isSafeInteger(Number(match[5]))) {
    throw Error('INVALID_PERSISTENT_DRAFT_REFERENCE');
  }
  return Object.freeze({
    projectId:match[1],relativePath,checkpointId:match[3],
    bufferId:match[4],bufferRevision:Number(match[5]),
    baseDiskRevision:match[6],draftSha256:match[7],
  });
}
