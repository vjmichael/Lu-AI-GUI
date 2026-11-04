// Complete, deterministic line-oriented change view. Nothing is silently
// truncated. This is not a minimal diff: the unmatched middle is represented
// as deletions followed by additions, while common prefix/suffix stay visible.
// Both original and replacement must additionally be displayed verbatim.
export function fullEditChangeLines(before, after) {
  if (typeof before !== 'string' || typeof after !== 'string')
    throw new TypeError('edit diff requires two exact UTF-8 strings');
  const a = before.split('\n'), b = after.split('\n');
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix++;
  let suffix = 0;
  while (suffix < a.length - prefix && suffix < b.length - prefix &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]) suffix++;
  const out = [];
  for (let i = 0; i < prefix; i++) out.push('  ' + a[i]);
  for (let i = prefix; i < a.length - suffix; i++) out.push('- ' + a[i]);
  for (let i = prefix; i < b.length - suffix; i++) out.push('+ ' + b[i]);
  for (let i = a.length - suffix; i < a.length; i++) out.push('  ' + a[i]);
  return out.join('\n');
}
