/**
 * B5 bounded UI working set. The native picker/browser still caps source files
 * at 512 KiB; keep at most 48 tabs in a project so retaining each original
 * CodeMirror history never becomes an unbounded hidden source cache.
 * This does NOT evict a dirty tab, silently discard Undo, or close files.
 */
export const MAX_OPEN_TABS_PER_PROJECT = 48;
export const MAX_OPEN_TABS_TOTAL = 96;
export const VISIBLE_TABS = 12;
export const INITIAL_VISIBLE_TREE_ENTRIES = 120;
export const TREE_ENTRY_PAGE = 120;

export function admitOpenTabs(tabs, projectId, incomingKeys) {
  if (!projectId || !Array.isArray(tabs) || !Array.isArray(incomingKeys)) {
    return {allowed:false,remaining:0,reason:'INVALID_EDITOR_ADMISSION'};
  }
  const present = new Set(tabs.filter(tab => tab?.projectId === projectId)
    .map(tab=>tab.key));
  const additions = new Set(incomingKeys.filter(key =>
    typeof key === 'string' && key && !present.has(key)));
  const global=new Set(tabs.filter(tab=>tab && typeof tab.key==='string').map(tab=>tab.key));
  const remaining=Math.max(0,Math.min(
    MAX_OPEN_TABS_PER_PROJECT-present.size,
    MAX_OPEN_TABS_TOTAL-global.size));
  const projectExceeded=present.size+additions.size>MAX_OPEN_TABS_PER_PROJECT;
  const globalExceeded=global.size+additions.size>MAX_OPEN_TABS_TOTAL;
  return {allowed:!projectExceeded && !globalExceeded,
    remaining,required:additions.size,
    reason:projectExceeded?'OPEN_TAB_BUDGET_EXCEEDED':
      globalExceeded?'GLOBAL_OPEN_TAB_BUDGET_EXCEEDED':null};
}

export function tabWindow(projectTabs, requestedStart=0, windowSize=VISIBLE_TABS) {
  if (!Array.isArray(projectTabs)) throw Error('INVALID_TAB_WINDOW');
  const width=Math.max(1,Math.min(VISIBLE_TABS,Number.isInteger(windowSize)?windowSize:VISIBLE_TABS));
  const maxStart=Math.max(0,projectTabs.length-width);
  const start=Number.isSafeInteger(requestedStart)
    ?Math.max(0,Math.min(maxStart,requestedStart)):0;
  return {visible:projectTabs.slice(start,start+width),
    count:projectTabs.length,start,end:Math.min(projectTabs.length,start+width),
    hiddenBefore:start,hiddenAfter:Math.max(0,projectTabs.length-start-width)};
}
export function pageTabWindow(projectTabs,oldStart,direction) {
  const length=Array.isArray(projectTabs)?projectTabs.length:0;
  const offset=direction<0?-VISIBLE_TABS:VISIBLE_TABS;
  return Math.max(0,Math.min(Math.max(0,length-VISIBLE_TABS),oldStart+offset));
}
export function centerSelectedTab(projectTabs,key,oldStart=0){
  if (!Array.isArray(projectTabs))return 0;
  const index=projectTabs.findIndex(tab=>tab.key===key);
  const current=tabWindow(projectTabs,oldStart);
  if(index<0)return current.start;
  if(index>=current.start&&index<current.end)return current.start;
  return tabWindow(projectTabs,Math.max(0,index-Math.floor(VISIBLE_TABS/2))).start;
}
export function visibleTreeEntries(list,requestedCount=INITIAL_VISIBLE_TREE_ENTRIES){
  const entries=Array.isArray(list)?list:[];
  const allowed=Number.isInteger(requestedCount)
    ?Math.max(0,Math.min(entries.length,requestedCount)):INITIAL_VISIBLE_TREE_ENTRIES;
  return {visible:entries.slice(0,allowed),
    total:entries.length,remaining:Math.max(0,entries.length-allowed)};
}

/** Local validation only. Native cap-std path validation is authoritative. */
export function verifiedRelativeSourcePath(value) {
  if (typeof value!=='string') return null;
  const path=value.trim();
  if (!path || path.length>1024 || path.startsWith('/') ||
      path.endsWith('/') || path.includes('\\') || path.includes(':') ||
      /[\u0000-\u001f\u007f]/.test(path) ||
      path.split('/').some(part=>!part || part==='.' || part==='..')) return null;
  return path;
}


/**
 * B6: A native file preview can finish after the tab budget, project selection,
 * or draft state has changed. Admission at request START is not authorization
 * to replace editor state when the response arrives.
 */
export function lateSourceOpenDecision(tabs, projectId, relativePath, opened, liveProjectId, key) {
  if (!projectId || liveProjectId !== projectId ||
      opened?.projectId !== projectId || opened?.relativePath !== relativePath ||
      typeof opened.text !== 'string' || typeof opened.revision !== 'string' ||
      typeof key !== 'string' || !key) {
    return {action:'discard_stale', reason:'SOURCE_PREVIEW_CONTEXT_CHANGED'};
  }
  if (!Array.isArray(tabs))
    return {action:'discard_stale', reason:'INVALID_EDITOR_ADMISSION'};
  // An existing editor may be dirty. Never reset its identity or Undo history.
  if (tabs.some(tab=>tab?.key === key))
    return {action:'select_existing', reason:null};
  const admission=admitOpenTabs(tabs,projectId,[key]);
  return admission.allowed
    ? {action:'open_new', reason:null}
    : {action:'deny', reason:admission.reason};
}

/** Reload is a destructive replacement of a draft and requires a fresh
 *  snapshot of the same tab. In-flight edits, close/reopen or project switching
 *  make the old confirmation obsolete; leave the newer draft untouched.
 */
export function lateReloadCanCommit(expected, current, opened, liveProjectId) {
  if (!expected || !current || expected.key !== current.key ||
      current.projectId !== expected.projectId ||
      current.relativePath !== expected.relativePath) return false;
  if (lateSourceOpenDecision([],expected.projectId,expected.relativePath,opened,
        liveProjectId,expected.key).action !== 'open_new') return false;
  return current.generation === expected.generation &&
    current.bufferRevision === expected.bufferRevision &&
    current.revision === expected.revision &&
    current.savedText === expected.savedText &&
    current.draft === expected.draft &&
    current.dirty === expected.dirty;
}
