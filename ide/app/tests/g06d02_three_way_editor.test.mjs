import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source = readFileSync('web/main.jsx', 'utf8');
test('6D02 draft stage selection cannot treat truncated or binary text as complete', () => {
  assert.match(source, /function selectConflictDraft\(entry, source\)/);
  assert.match(source, /selected\.binaryOrNonUtf8 \|\| selected\.previewTruncated/);
  assert.match(source, /selected\.preview !== 'string'/);
  assert.match(source, /G06D02_INCOMPLETE_OR_BINARY_BLOB_REQUIRES_FULL_REVIEW/);
  assert.match(source, /entry\.ours\.previewTruncated/);
  assert.match(source, /entry\.theirs\.previewTruncated/);
});
test('6D02 proposal is local and bound to unchanged source evidence', () => {
  assert.match(source, /conflictDraft\.indexSha256 === nativeConflicts\.indexSha256/);
  assert.match(source, /conflictDraft\.evidenceSha256 === nativeConflicts\.evidenceSha256/);
  assert.match(source, /conflictDraft\.projectId === activeProject\.projectId/);
  assert.match(source, /setConflictDraft\(null\);/);
  assert.match(source, /value=\{conflictDraft\.candidateText\}/);
  assert.match(source, /maxLength=\{131072\}/);
  assert.match(source, /Unsaved resolution candidate/);
  assert.match(source, /No index stage,/);
});
test('6D02 does not add an executor or OS authorization for drafts', () => {
  const start = source.indexOf('function selectConflictDraft(entry, source)');
  const end = source.indexOf('async function reviewNativeConflictCandidate()', start);
  assert.ok(start >= 0 && end > start);
  const draft = source.slice(start, end);
  assert.doesNotMatch(draft, /invoke\(|execute_|commit\(|checkout|index\.write/);
});
