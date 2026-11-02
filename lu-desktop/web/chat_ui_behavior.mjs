/** Small, dependency-free UI behaviors adapted from proven desktop chat UX. */
export const SCROLL_BOTTOM_THRESHOLD = 48;

export function nearTranscriptBottom(metrics, threshold = SCROLL_BOTTOM_THRESHOLD) {
  if (!metrics) return true;
  const { scrollHeight, scrollTop, clientHeight } = metrics;
  return scrollHeight - scrollTop - clientHeight <= threshold;
}

export function restoredScrollTop(snapshot, scrollHeight, clientHeight) {
  if (!snapshot || snapshot.follow) return Math.max(0, scrollHeight - clientHeight);
  return Math.max(0, Math.min(snapshot.top, scrollHeight - clientHeight));
}

export function isSendKey(event) {
  return event.key === 'Enter' && !event.shiftKey && !event.isComposing &&
    !event.nativeEvent?.isComposing && event.keyCode !== 229;
}

export function isSearchKey(event) {
  return (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey &&
    event.key?.toLowerCase() === 'k';
}

export function isNewChatKey(event) {
  return (event.ctrlKey || event.metaKey) && event.shiftKey && !event.altKey &&
    event.key?.toLowerCase() === 'n';
}

export function matchesChatTitle(chat, query) {
  const q = query.trim().toLocaleLowerCase();
  return !q || (chat.title ?? '').toLocaleLowerCase().includes(q);
}
