// B4 pure layout decisions. The same CodeMirror EditorState MUST never be
// mounted in two EditorViews. A secondary pane is another already-open tab,
// never a second copy of that tab's source or a new native authority.
export function projectEditorTabs(tabs, projectId) {
  if (!projectId || !Array.isArray(tabs)) return [];
  return tabs.filter(tab => tab.projectId === projectId &&
    typeof tab.key === 'string' && typeof tab.relativePath === 'string');
}
export function secondaryEditor(tabs, projectId, primaryKey, secondaryKey) {
  if (!secondaryKey || !primaryKey || secondaryKey === primaryKey) return null;
  return projectEditorTabs(tabs,projectId).find(tab => tab.key === secondaryKey) || null;
}
export function initialSecondaryKey(tabs, projectId, primaryKey) {
  return projectEditorTabs(tabs,projectId).find(tab => tab.key !== primaryKey)?.key || null;
}
export function canSaveFocusedPane(pane, primary, secondary) {
  if (pane === 'secondary') return secondary ?? null;
  return primary ?? null;
}
