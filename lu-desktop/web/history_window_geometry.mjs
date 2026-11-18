/** Derived-only ANSF viewport geometry. Original conversation bytes stay in SQLite. */
export const MAX_HISTORY_ROWS = 192;

export function mergeHistoryWindow(existing, incoming, direction, cap = MAX_HISTORY_ROWS) {
  if (!['older', 'newer'].includes(direction) || !Number.isSafeInteger(cap) || cap < 1 || cap > 512) {
    throw new Error('Invalid bounded history window');
  }
  const combined = new Map();
  for (const row of [...existing, ...incoming]) {
    if (!Number.isSafeInteger(row.seq) || row.seq < 1 || !['user', 'assistant'].includes(row.role)) {
      throw new Error('History window contains an invalid ANSF display sequence or role');
    }
    combined.set(row.seq, row);
  }
  const all = [...combined.values()].sort((a, b) => a.seq - b.seq);
  return {
    rows: direction === 'older' ? all.slice(0, cap) : all.slice(-cap),
    evictedOlder: direction === 'newer' && all.length > cap,
    evictedNewer: direction === 'older' && all.length > cap,
  };
}

export function captureHistoryAnchor(container) {
  if (!container) return null;
  const viewport = container.getBoundingClientRect();
  const entries = container.querySelectorAll('[data-ansf-seq]');
  for (const entry of entries) {
    const rect = entry.getBoundingClientRect();
    if (rect.bottom > viewport.top && rect.top < viewport.bottom) {
      const seq = Number(entry.dataset.ansfSeq);
      if (Number.isSafeInteger(seq) && seq > 0) {
        return {seq, offset: rect.top - viewport.top, scrollTop: container.scrollTop};
      }
    }
  }
  return {seq: null, offset: 0, scrollTop: container.scrollTop};
}

export function restoreHistoryAnchor(container, anchor) {
  if (!container || !anchor) return false;
  if (Number.isSafeInteger(anchor.seq) && anchor.seq > 0) {
    const target = container.querySelector('[data-ansf-seq="' + anchor.seq + '"]');
    if (target) {
      const current = target.getBoundingClientRect().top - container.getBoundingClientRect().top;
      container.scrollTop += current - anchor.offset;
      return true;
    }
  }
  container.scrollTop = anchor.scrollTop;
  return false;
}
