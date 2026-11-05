import test from 'node:test';
import assert from 'node:assert/strict';
import { locateMatches, planReplacement } from '../web/find_replace.js';

test('literal search escapes regex syntax and uses CodeMirror UTF-16 offsets', () => {
  assert.deepEqual(locateMatches('α[.] and [.]', '[.]').matches,
    [{ from: 1, to: 4 }, { from: 9, to: 12 }]);
  assert.deepEqual(locateMatches('A+b a+b', 'a+b').matches,
    [{ from: 0, to: 3 }, { from: 4, to: 7 }]);
  assert.deepEqual(locateMatches('😀 Z 😀', '😀').matches,
    [{ from: 0, to: 2 }, { from: 5, to: 7 }]);
});

test('case sensitive search and replace are literal, including dollars', () => {
  assert.equal(locateMatches('One one ONE', 'one', true).matches.length, 1);
  assert.equal(locateMatches('One one ONE', 'one').matches.length, 3);
  const plan = planReplacement('abc abc', 'abc', '$&', { all: true });
  assert.equal(plan.status, 'ready');
  assert.deepEqual(plan.changes, [
    { from: 0, to: 3, insert: '$&' },
    { from: 4, to: 7, insert: '$&' },
  ]);
});

test('replace one respects chosen index while replace all is one batch', () => {
  const single = planReplacement('one one one', 'one', 'TWO', { selectedIndex: 1 });
  assert.deepEqual(single.changes, [{ from: 4, to: 7, insert: 'TWO' }]);
  const all = planReplacement('one one one', 'one', '', { all: true });
  assert.equal(all.count, 3);
  assert.deepEqual(all.changes.map(x => x.from), [0, 4, 8]);
});

test('no partial Replace All when hit count exceeds bounded search budget', () => {
  const content = 'a'.repeat(10001);
  const found = locateMatches(content, 'a');
  assert.equal(found.matches.length, 10000);
  assert.equal(found.truncated, true);
  const plan = planReplacement(content, 'a', 'b', { all: true });
  assert.equal(plan.status, 'truncated');
  assert.deepEqual(plan.changes, []);
  assert.equal(planReplacement('abc', '', 'x').status, 'no_match');
  assert.equal(planReplacement('abc', 'missing', 'x').status, 'no_match');
});
