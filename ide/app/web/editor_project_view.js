// B7 in-memory project workspace view. This is a navigation preference,
// never a source snapshot, a durable draft checkpoint, or file authorization.
// Only live registered editor tabs are eligible to become visible again.
import { projectEditorTabs, initialSecondaryKey } from './editor_layout.js';
import { centerSelectedTab, tabWindow } from './editor_working_set.js';

function liveKey(projectTabs, key) {
  return typeof key === 'string' &&
    projectTabs.some(tab => tab.key === key) ? key : null;
}

export function captureProjectView(tabs, projectId, primaryKey, secondaryKey,
    splitOpen, requestedTabWindowStart) {
  const projectTabs = projectEditorTabs(tabs, projectId);
  if (!projectId || !projectTabs.length) return null;
  const primary = liveKey(projectTabs, primaryKey);
  const secondary = liveKey(projectTabs, secondaryKey);
  return Object.freeze({
    schemaVersion: 1, projectId,
    primaryKey: primary,
    secondaryKey: secondary && secondary !== primary ? secondary : null,
    splitOpen: !!splitOpen,
    tabWindowStart: tabWindow(projectTabs, requestedTabWindowStart).start,
  });
}

/**
 * Restore ONLY visibility and selection. Do not read files, create tabs,
 * resume workers, restore private editor buffers, or implicitly save.
 * A removed/closed tab can never be revived by stale navigation metadata.
 */
export function restoreProjectView(tabs, projectId, savedView) {
  const projectTabs = projectEditorTabs(tabs, projectId);
  const sameProject = savedView?.schemaVersion === 1 &&
    savedView?.projectId === projectId;
  const original = sameProject ? savedView : null;
  const primaryKey = liveKey(projectTabs, original?.primaryKey) ??
    projectTabs[projectTabs.length - 1]?.key ?? null;
  const rememberedSecondary = liveKey(projectTabs, original?.secondaryKey);
  const secondaryKey = rememberedSecondary && rememberedSecondary !== primaryKey
    ? rememberedSecondary
    : original?.splitOpen
      ? initialSecondaryKey(projectTabs, projectId, primaryKey)
      : null;
  const splitOpen = !!(original?.splitOpen && primaryKey);
  const initialStart = Number.isSafeInteger(original?.tabWindowStart)
    ? original.tabWindowStart : 0;
  return Object.freeze({
    primaryKey,
    secondaryKey: secondaryKey && secondaryKey !== primaryKey
      ? secondaryKey : null,
    splitOpen,
    tabWindowStart: centerSelectedTab(projectTabs, primaryKey, initialStart),
  });
}
