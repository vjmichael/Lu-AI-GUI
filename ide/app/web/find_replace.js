// Human-owned, local-buffer literal find/replace planner.
// Matches refer to UTF-16 CodeMirror offsets, not bytes on disk.
// No replacement templates, file writes, or host/model permissions.
export const MAX_FIND_MATCHES = 10000;
export const MAX_FIND_TEXT = 2048;

export function locateMatches(text, query, caseSensitive = false) {
  if (typeof text !== 'string' || typeof query !== 'string' || !query ||
      query.length > MAX_FIND_TEXT) {
    return { matches: [], truncated: typeof query === 'string' && query.length > MAX_FIND_TEXT };
  }
  const escaped = query.replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&');
  const search = new RegExp(escaped, caseSensitive ? 'g' : 'gi');
  const matches = [];
  let match;
  while ((match = search.exec(text)) !== null) {
    if (matches.length === MAX_FIND_MATCHES) {
      return { matches, truncated: true };
    }
    matches.push({ from: match.index, to: match.index + match[0].length });
  }
  return { matches, truncated: false };
}

export function planReplacement(text, query, replacement, options = {}) {
  if (typeof replacement !== 'string') return { status: 'invalid', changes: [], count: 0 };
  const { caseSensitive = false, all = false, selectedIndex = -1 } = options;
  const { matches, truncated } = locateMatches(text, query, caseSensitive);
  if (truncated) return { status: 'truncated', changes: [], count: 0 };
  if (matches.length === 0) return { status: 'no_match', changes: [], count: 0 };
  const index = Number.isInteger(selectedIndex) && selectedIndex >= 0 &&
    selectedIndex < matches.length ? selectedIndex : 0;
  const chosen = all ? matches : [matches[index]];
  return {
    status: 'ready',
    count: chosen.length,
    changes: chosen.map(({ from, to }) => ({ from, to, insert: replacement })),
  };
}
