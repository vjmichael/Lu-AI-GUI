/**
 * B8: an explicit, bounded way to reclaim old CodeMirror working-set memory.
 *
 * Readonly planning only. It never closes a tab itself, reads the filesystem,
 * writes a project source, or treats a draft as saved. A React tab can claim
 * "clean" one render behind a CodeMirror transaction: require a live state
 * text equality witness before declaring it eligible for closure.
 *
 * Protected keys include BOTH editor panes, any pending Save, draft snapshot,
 * recovery, or reviewed LSP session. All other projects remain untouched.
 */
export function planCleanBackgroundTabClosure(
    tabs, projectId, protectedKeys, getLiveEditorText) {
  if (!Array.isArray(tabs) || typeof projectId !== 'string' || !projectId ||
      !(protectedKeys instanceof Set) || typeof getLiveEditorText !== 'function')
    return {closeKeys:[],retainedKeys:[],reason:'INVALID_TAB_CLEANUP_CONTEXT'};
  const closeKeys = [];
  const keptKeys = [];
  for (const tab of tabs) {
    if (!tab || tab.projectId !== projectId ||
        typeof tab.key !== 'string' || !tab.key) continue;
    // A file can look clean after Undo or explicit Save yet still own
    // meaningful CodeMirror history. Only never-edited (revision zero)
    // background tabs are eligible for this conservative RAM reclamation.
    if (protectedKeys.has(tab.key) || tab.dirty !== false ||
        tab.bufferRevision !== 0 ||
        typeof tab.savedText !== 'string' || tab.draft !== tab.savedText) {
      keptKeys.push(tab.key);
      continue;
    }
    let actual = null;
    try { actual = getLiveEditorText(tab.key); } catch { /* fail closed */ }
    if (typeof actual !== 'string' || actual !== tab.savedText) {
      keptKeys.push(tab.key);
      continue;
    }
    closeKeys.push(tab.key);
  }
  return {
    closeKeys,
    retainedKeys:keptKeys,
    reason:closeKeys.length ? null : 'NO_VERIFIED_CLEAN_BACKGROUND_TABS',
  };
}
