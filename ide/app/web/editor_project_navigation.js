// IDE-INT01-C: project-owned pane navigation. No disk reads, IPC, stored
// source contents, editor replacement, or worker execution.
import { captureProjectView, restoreProjectView } from './editor_project_view.js';

function owned(tabs, projectId, key) {
  return typeof key === 'string' &&
    tabs.some(tab => tab?.key === key && tab.projectId === projectId);
}

// Selecting a file already displayed in the secondary pane swaps the two
// visible file keys instead of mounting the same CodeMirror state twice.
export function planProjectPaneSelection(tabs, projectId, primaryKey,
    secondaryKey, splitOpen, requestedKey) {
  if (!projectId || !Array.isArray(tabs) ||
      !owned(tabs, projectId, requestedKey))
    return Object.freeze({ allowed: false, reason: 'TAB_OUTSIDE_ACTIVE_PROJECT' });
  const previousPrimary=owned(tabs,projectId,primaryKey)?primaryKey:null;
  const previousSecondary=owned(tabs,projectId,secondaryKey) &&
    secondaryKey!==previousPrimary ? secondaryKey:null;
  const swapped=!!splitOpen && requestedKey===previousSecondary &&
    !!previousPrimary && previousPrimary!==requestedKey;
  const nextSecondary=!splitOpen ? null :
    swapped ? previousPrimary :
      previousSecondary===requestedKey ? null : previousSecondary;
  return Object.freeze({ allowed: true, primaryKey:requestedKey,
    secondaryKey:nextSecondary, splitOpen:!!splitOpen,
    swapped });
}

export function planProjectSwitchView(tabs, fromProjectId, toProjectId,
    primaryKey, secondaryKey, splitOpen, tabWindowStart, previousView) {
  if (!toProjectId || !Array.isArray(tabs))
    return { allowed:false, reason:'NO_TARGET_PROJECT' };
  const leaving=fromProjectId && fromProjectId!==toProjectId ?
    captureProjectView(tabs,fromProjectId,primaryKey,secondaryKey,
      splitOpen,tabWindowStart) : null;
  const target=restoreProjectView(tabs,toProjectId,previousView);
  if (target.primaryKey && !owned(tabs,toProjectId,target.primaryKey))
    return { allowed:false, reason:'RESTORED_PRIMARY_NOT_OWNED' };
  if (target.secondaryKey &&
      (!owned(tabs,toProjectId,target.secondaryKey) ||
        target.secondaryKey===target.primaryKey))
    return { allowed:false, reason:'RESTORED_SECONDARY_NOT_OWNED' };
  return Object.freeze({allowed:true,fromProjectId,toProjectId,leaving,
    primaryKey:target.primaryKey,secondaryKey:target.secondaryKey,
    splitOpen:target.splitOpen,tabWindowStart:target.tabWindowStart});
}

// A project switch may be requested while an unrelated preview is in flight.
// The caller only commits a response if its exact epoch, source project,
// registered target, and native directory identity are still current.
export function mayCommitProjectNavigation(request, response, context) {
  if (!request || !response || !context ||
      request.epoch!==context.epoch ||
      request.fromProjectId!==context.liveProjectId ||
      request.toProjectId!==context.requestedProjectId ||
      response.projectId!==request.toProjectId ||
      response.relativeDir!==request.relativeDir ||
      !Array.isArray(response.entries)) return false;
  return true;
}
