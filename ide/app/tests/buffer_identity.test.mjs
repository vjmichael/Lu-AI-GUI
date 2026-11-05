import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorState } from '@codemirror/state';
import {
  newBufferIdentity, advanceBufferIdentity, captureBufferSpan,
  resolveBufferSpan, formatBufferReference,
} from '../web/buffer_identity.js';

const base = 'a'.repeat(64);
const create = text => EditorState.create({ doc: text });
const original = () => newBufferIdentity('p-12345', 'src/example.rs', base,
  '11223344-5566-7788-99aa-bbccddeeff00');

test('unsaved draft has a distinct immutable ID and correct CRLF/Unicode offsets', () => {
  const identity = original();
  const state = create('α\r\n😀z\nlast');
  const ref = captureBufferSpan(identity, state, 3, 6);
  assert.equal(ref.bufferRevision, 0);
  assert.equal(ref.baseDiskRevision, base);
  assert.equal(ref.startLine, 2);
  assert.equal(ref.endLine, 2);
  assert.equal(ref.startColumnUtf16, 1);
  assert.equal(ref.endColumnUtf16, 4);
  assert.equal(ref.fromByte, 4); // 2-byte alpha, CRLF, then emoji
  assert.equal(ref.toByte, 9);
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
