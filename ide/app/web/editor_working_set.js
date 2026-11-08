/**
 * B5 bounded UI working set. The native picker/browser still caps source files
 * at 512 KiB; keep at most 48 tabs in a project so retaining each original
 * CodeMirror history never becomes an unbounded hidden source cache.
 * This does NOT evict a dirty tab, silently discard Undo, or close files.
 */
export const MAX_OPEN_TABS_PER_PROJECT = 48;
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
  const remaining = Math.max(0,MAX_OPEN_TABS_PER_PROJECT-present.size);
  return {allowed:additions.size <= remaining,
    remaining,required:additions.size,
    reason:additions.size>remaining?'OPEN_TAB_BUDGET_EXCEEDED':null};
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
