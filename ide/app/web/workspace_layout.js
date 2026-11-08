/**
 * B9: source-free persistent navigation metadata.
 * Not an editor draft, source snapshot, startup authority or OS path.
 * The native host revalidates registered roots and exact disk revisions before
 * storing, and reopens each path using the original bounded source browser.
 */
import { verifiedRelativeSourcePath, MAX_OPEN_TABS_PER_PROJECT } from './editor_working_set.js';

export const LAYOUT_SCHEMA = 1;
const revision = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const validProject = value => typeof value === 'string' && /^ide-[a-f0-9]{32}$/.test(value);

export function rememberedWorkspaceFromTabs(
  tabs, projectId, primaryKey, secondaryKey, splitOpen, tabWindowStart,
  getLiveText
) {
  if (!validProject(projectId) || !Array.isArray(tabs) ||
      typeof getLiveText !== 'function') throw Error('B9_INVALID_WORKSPACE');
  const relevant=tabs.filter(tab=>tab?.projectId===projectId);
  if (!relevant.length || relevant.length>MAX_OPEN_TABS_PER_PROJECT)
    throw Error('B9_TAB_BUDGET');
  const files=[], keys=new Set();
  for (const tab of relevant) {
    if (tab.key !== projectId+':'+tab.relativePath ||
        verifiedRelativeSourcePath(tab.relativePath)!==tab.relativePath ||
        !revision(tab.revision) || keys.has(tab.key) ||
        typeof tab.savedText!=='string' || typeof tab.draft!=='string')
      throw Error('B9_UNTRUSTED_TAB');
    keys.add(tab.key);
    // A just-typed edit may not yet have entered React state. Persist its
    // dirty INDICATOR only; never its raw bytes or editor state.
    let live;
    try { live=getLiveText(tab.key); } catch { throw Error('B9_EDITOR_UNAVAILABLE'); }
    if (typeof live!=='string') throw Error('B9_EDITOR_UNAVAILABLE');
    files.push({
      relativePath:tab.relativePath,savedRevision:tab.revision,
      wasDirty:tab.dirty===true || tab.draft!==tab.savedText || live!==tab.savedText,
    });
  }
  const primary=tabs.find(tab=>tab.key===primaryKey && tab.projectId===projectId);
  const secondary=tabs.find(tab=>tab.key===secondaryKey && tab.projectId===projectId);
  return Object.freeze({
    schemaVersion:LAYOUT_SCHEMA,projectId,files,
    primaryPath:primary?.relativePath??null,
    secondaryPath:secondary && secondary.key!==primaryKey ? secondary.relativePath : null,
    splitOpen:!!(splitOpen && primary),
    tabWindowStart:Number.isSafeInteger(tabWindowStart)
      ?Math.max(0,Math.min(MAX_OPEN_TABS_PER_PROJECT-1,tabWindowStart)) : 0,
  });
}

/** Validate even native storage output before presenting it as saved intent. */
export function checkedRememberedWorkspace(value, projectId) {
  if (!validProject(projectId) || value?.schemaVersion!==LAYOUT_SCHEMA ||
      value.projectId!==projectId || !Array.isArray(value.files) ||
      !value.files.length || value.files.length>MAX_OPEN_TABS_PER_PROJECT ||
      !Number.isSafeInteger(value.tabWindowStart) || value.tabWindowStart<0 ||
      value.tabWindowStart>=MAX_OPEN_TABS_PER_PROJECT ||
      typeof value.splitOpen!=='boolean')
    throw Error('B9_INVALID_STORED_WORKSPACE');
  const paths=new Set();
  for (const file of value.files) {
    if (!file || verifiedRelativeSourcePath(file.relativePath)!==file.relativePath ||
        !revision(file.savedRevision) || typeof file.wasDirty!=='boolean' ||
        paths.has(file.relativePath))
      throw Error('B9_INVALID_STORED_WORKSPACE');
    paths.add(file.relativePath);
  }
  const selected=[value.primaryPath,value.secondaryPath];
  if (selected.some(path=>path!==null && !paths.has(path)) ||
      (!value.primaryPath && (value.secondaryPath || value.splitOpen)) ||
      (value.primaryPath && value.primaryPath===value.secondaryPath))
    throw Error('B9_INVALID_STORED_WORKSPACE');
  // Explicitly discard unknown source, worker, and draft fields if an older
  // native record ever exposes extra keys.
  return Object.freeze({
    schemaVersion:LAYOUT_SCHEMA,projectId,
    files:value.files.map(item=>Object.freeze({
      relativePath:item.relativePath,savedRevision:item.savedRevision,
      wasDirty:item.wasDirty,
    })),
    primaryPath:value.primaryPath,secondaryPath:value.secondaryPath,
    splitOpen:value.splitOpen,tabWindowStart:value.tabWindowStart,
  });
}

/**
 * A saved layout is a request to REVALIDATE, not permission to commit a tab.
 * A native preview must independently match the registered project/file.
 * Disk revision drift is visible and the newest SAVED text can be reopened,
 * but no earlier unsaved draft is resurrected from this metadata.
 */
export function revalidatedResumeDecision(layout, file, preview, liveProjectId) {
  if (!layout || !file || layout.projectId!==liveProjectId ||
      preview?.projectId!==liveProjectId ||
      preview?.relativePath!==file.relativePath ||
      typeof preview?.text!=='string' || !revision(preview.revision))
    return {action:'skip',reason:'B9_SOURCE_NOT_VERIFIED'};
  return {action:'open_saved',changedOnDisk:preview.revision!==file.savedRevision,
    hadUnsavedDraft:file.wasDirty};
}
