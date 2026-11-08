import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorState } from '@codemirror/state';
import {
  newBufferIdentity, advanceBufferIdentity, rebaseBufferAfterSave,
  captureBufferSpan,
  resolveBufferSpan, formatBufferReference,
} from '../web/draft_ref_port.js';

const base = 'a'.repeat(64);
const create = text => EditorState.create({ doc: text });
const createCrLf = text => EditorState.create({
  doc: text, extensions: [EditorState.lineSeparator.of('\r\n')],
});
const original = () => newBufferIdentity('p-12345', 'src/example.rs', base,
  '11223344-5566-7788-99aa-bbccddeeff00');

test('unsaved draft has a distinct immutable ID and correct CRLF/Unicode offsets', () => {
  const identity = original();
  const state = createCrLf('α\r\n😀z\r\nlast');
  const from = state.doc.line(2).from;
  const to = state.doc.line(2).to;
  const ref = captureBufferSpan(identity, state, from, to);
  assert.equal(ref.bufferRevision, 0);
  assert.equal(ref.baseDiskRevision, base);
  assert.equal(ref.startLine, 2);
  assert.equal(ref.endLine, 2);
  assert.equal(ref.startColumnUtf16, 1);
  assert.equal(ref.endColumnUtf16, 4);
  assert.equal(ref.fromByte, 4); // 2-byte alpha followed by CRLF
  assert.equal(ref.toByte, 9);
  assert.equal(state.sliceDoc(), 'α\r\n😀z\r\nlast');
  assert.equal(state.doc.toString(), 'α\n😀z\nlast');
  assert.equal(ref.text, '😀z');
  assert.equal(ref.lifetime, 'current_editor_session_only');
  assert.equal(resolveBufferSpan(ref, identity, state).status, 'current');
  assert.ok(Object.isFrozen(identity));
  assert.ok(Object.isFrozen(ref));
});

test('typing, undo and a new editor session invalidate previous source refs', () => {
  const before = original();
  const initial = create('alpha');
  const ref = captureBufferSpan(before, initial, 0, 5);
  const edited = advanceBufferIdentity(before);
  assert.equal(edited.revision, 1);
  assert.equal(resolveBufferSpan(ref, edited, create('alphabet')).status,
    'stale_buffer_reference');
  const undone = advanceBufferIdentity(edited);
  assert.equal(undone.revision, 2);
  assert.equal(resolveBufferSpan(ref, undone, initial).status,
    'stale_buffer_reference'); // identical text is not enough
  const reopened = newBufferIdentity(before.projectId, before.relativePath,
    before.savedRevision, 'ffeeddcc-bbaa-9988-7766-554433221100');
  assert.equal(resolveBufferSpan(ref, reopened, initial).status,
    'stale_buffer_reference');
});

test('saving rebases saved SHA and invalidates old draft source reference', () => {
  const identity = original();
  const state = create('edited');
  const ref = captureBufferSpan(identity, state, 0, 6);
  const afterSave = Object.freeze({ ...advanceBufferIdentity(identity),
    savedRevision: 'b'.repeat(64) });
  assert.equal(resolveBufferSpan(ref, afterSave, state).status, 'stale_buffer_reference');
  assert.equal(afterSave.savedRevision, 'b'.repeat(64));
});

test('bounds and invalid offsets refuse invented or oversized spans', () => {
  const identity = original();
  const state = create('x\ny');
  assert.throws(() => captureBufferSpan(identity, state, 2, 10), /INVALID_BUFFER_RANGE/);
  assert.throws(() => captureBufferSpan(identity, state, -1, 1), /INVALID_BUFFER_RANGE/);
  assert.throws(() => captureBufferSpan(identity, state, 2, 1), /INVALID_BUFFER_RANGE/);
  assert.throws(() => captureBufferSpan(identity,
    create('x\n'.repeat(121)), 0, 242), /BUFFER_SPAN_TOO_LARGE/);
  assert.throws(() => captureBufferSpan(identity,
    create('z'.repeat(32769)), 0, 32769), /BUFFER_SPAN_TOO_LARGE/);
  assert.throws(() => newBufferIdentity('p', 'file', 'bad', 'valid-buffer-id'),
    /INVALID_BUFFER_IDENTITY/);
});

test('clipboard source reference is explicitly volatile, with full draft digest', () => {
  const span = captureBufferSpan(original(), create('hello'), 0, 5);
  const ref = formatBufferReference(span, 'f'.repeat(64));
  assert.match(ref, /^ide-buffer:v1/);
  assert.match(ref, /baseDisk=a{64}/);
  assert.match(ref, /draftSha256=f{64}/);
  assert.match(ref, /rangeUtf16=0\.\.5/);
  assert.match(ref, /bytes=0\.\.5/);
  assert.match(ref, /lifetime=session-only/);
  assert.doesNotMatch(ref, /hello/); // source contents stay in editor
});

test('CRLF across multiple lines uses UTF-8 bytes despite one-unit editor breaks', () => {
  const state = createCrLf('α\r\n😀z\r\nlast');
  const afterTwoLines = state.doc.line(3).from;
  const span = captureBufferSpan(original(), state, 0, afterTwoLines);
  assert.equal(afterTwoLines, 6); // editor counts each CRLF as one
  assert.equal(span.toByte, 11); // serialized UTF-8 has two-byte CRLF pairs
  assert.equal(span.endLine, 3);
  assert.equal(span.endColumnUtf16, 1);
});

test('invalid source ranges may not split a UTF-16 surrogate pair', () => {
  const identity = original();
  const state = create('😀z');
  assert.throws(() => captureBufferSpan(identity, state, 1, 3),
    /INVALID_BUFFER_RANGE/);
  assert.throws(() => captureBufferSpan(identity, state, 0, 1),
    /INVALID_BUFFER_RANGE/);
  assert.equal(captureBufferSpan(identity, state, 0, 2).text, '😀');
});

test('adversarial CodeMirror transaction lifecycle rejects stale references', () => {
  const initial = create('header\nkeep me\ntail');
  const identity = original();
  const selected = initial.doc.line(2);
  const captured = captureBufferSpan(identity, initial, selected.from, selected.to);
  assert.equal(resolveBufferSpan(captured, identity, initial).status, 'current');

  // A modification outside the selected range still invalidates the entire
  // draft revision, even when the selected source text remains identical.
  const editedState = initial.update({ changes: {
    from: initial.doc.length, insert: '\nmodified',
  }}).state;
  const editedIdentity = advanceBufferIdentity(identity);
  assert.equal(editedState.doc.line(2).text, 'keep me');
  assert.equal(resolveBufferSpan(captured, editedIdentity, editedState).status,
    'stale_buffer_reference');

  // Undo is another edit transaction; byte-for-byte old text is not enough.
  const undoneState = editedState.update({ changes: {
    from: initial.doc.length, to: editedState.doc.length, insert: '',
  }}).state;
  const undoneIdentity = advanceBufferIdentity(editedIdentity);
  assert.equal(undoneState.sliceDoc(), initial.sliceDoc());
  assert.equal(resolveBufferSpan(captured, undoneIdentity, undoneState).status,
    'stale_buffer_reference');

  // Saving the same text rebases its disk revision; old references expire.
  const savedIdentity = Object.freeze({ ...advanceBufferIdentity(undoneIdentity),
    savedRevision: 'c'.repeat(64) });
  assert.equal(resolveBufferSpan(captured, savedIdentity, undoneState).status,
    'stale_buffer_reference');

  // A reopened tab gets a new editor identity even with identical file bytes.
  const reopened = newBufferIdentity(identity.projectId, identity.relativePath,
    savedIdentity.savedRevision, 'cceeff00-1122-3344-5566-77889900aabb');
  assert.equal(resolveBufferSpan(captured, reopened, create(initial.sliceDoc())).status,
    'stale_buffer_reference');
});

test('snapshot range tampering and wrong source identity never authorize reuse', () => {
  const identity = original();
  const state = create('first\nsecond');
  const span = captureBufferSpan(identity, state, 0, 5);
  assert.equal(resolveBufferSpan({ ...span, fromByte: 2 }, identity, state).status,
    'stale_buffer_reference');
  assert.equal(resolveBufferSpan({ ...span, text: 'other' }, identity, state).status,
    'stale_buffer_reference');
  assert.equal(resolveBufferSpan({ ...span, projectId: 'another' }, identity, state).status,
    'stale_buffer_reference');
  assert.equal(resolveBufferSpan(span, { ...identity, relativePath: 'other.rs' }, state).status,
    'stale_buffer_reference');
  assert.throws(() => advanceBufferIdentity({ ...identity,
    revision: Number.MAX_SAFE_INTEGER }), /BUFFER_REVISION_EXHAUSTED/);
});

test('late Save may rebase the original draft after typing, but not a reopened tab', () => {
  const opened = original();
  const typedAgain = advanceBufferIdentity(advanceBufferIdentity(opened));
  const saved = 'e'.repeat(64);
  const good = rebaseBufferAfterSave(typedAgain, opened, saved);
  assert.equal(good.status, 'rebased');
  assert.equal(good.identity.revision, 3);
  assert.equal(good.identity.savedRevision, saved);
  assert.equal(good.identity.bufferId, opened.bufferId);
  const reopened = newBufferIdentity(opened.projectId, opened.relativePath,
    opened.savedRevision, '12345678-abcd-4321-aaaa-123456789abc');
  const wrong = rebaseBufferAfterSave(reopened, opened, saved);
  assert.equal(wrong.status, 'replaced_buffer');
  assert.strictEqual(wrong.identity, reopened);
  assert.equal(reopened.savedRevision, opened.savedRevision);
  const wrongSource = rebaseBufferAfterSave({
    ...typedAgain, projectId: 'another',
  }, opened, saved);
  assert.equal(wrongSource.status, 'replaced_buffer');
  const badHash = rebaseBufferAfterSave(typedAgain, opened, 'not-a-hash');
  assert.equal(badHash.status, 'replaced_buffer');
});
