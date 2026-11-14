/**
 * C7: source-task relations are observations, not source edits, test receipts,
 * execution authorization or accepted work. Durable C6 records are host-owned.
 */
import { verifiedRelativeSourcePath } from './editor_working_set.js';

const projectId = s => typeof s==='string' && /^ide-[a-f0-9]{32}$/.test(s);
const revision = s => typeof s==='string' && /^[a-f0-9]{64}$/.test(s);
const taskId = s => typeof s==='string' && /^[A-Za-z0-9_.-]{1,64}$/.test(s);
const eventId = s => typeof s==='string' && /^evt-[a-f0-9]{32}$/.test(s);
const MAX_LINKS=12;

/** Require a real C6 task and live project-scoped tab; emit metadata only. */
export function planTaskSourceLink(tasks, chosenTaskId, tab, liveText, activeProjectId) {
  if (!projectId(activeProjectId) || !Array.isArray(tasks) ||
      !taskId(chosenTaskId) || !tab ||
      tab.projectId!==activeProjectId ||
      tab.key!==activeProjectId+':'+tab.relativePath ||
      verifiedRelativeSourcePath(tab.relativePath)!==tab.relativePath ||
      !revision(tab.revision) || typeof tab.savedText!=='string' ||
      typeof tab.draft!=='string' || typeof liveText!=='string')
    throw Error('C7_INVALID_SOURCE_SELECTION');
  const task=tasks.find(item=>item?.taskId===chosenTaskId);
  if (!task || !eventId(task.updateEventId) ||
      !Array.isArray(task.sourceLinks) ||
      (task.sourceLinks.length>=MAX_LINKS &&
        !task.sourceLinks.some(link=>link.relativePath===tab.relativePath)))
    throw Error('C7_TASK_NOT_AVAILABLE');
  const alreadyLinked=task.sourceLinks.some(link=>link.relativePath===tab.relativePath);
  return {
    request:Object.freeze({projectId:activeProjectId,taskId:chosenTaskId,
      relativePath:tab.relativePath,expectedSavedRevision:tab.revision,
      expectedTaskUpdateEventId:task.updateEventId}),
    boundUpdateEventId:task.updateEventId,
    // Only a boolean is reported; the user's draft bytes stay in CodeMirror.
    hasUnrecordedDraft:liveText!==tab.savedText || tab.draft!==tab.savedText,
    relinking:alreadyLinked,
  };
}

/** Never mistake an open editor's saved BASE SHA for a live disk recheck. */
export function sourceLinkDisplay(link,task,tab,project) {
  if (!projectId(project) || task?.taskId==null ||
      !link || link.source!=='native_registered_source_read' ||
      !revision(link.savedRevision) ||
      verifiedRelativeSourcePath(link.relativePath)!==link.relativePath ||
      !eventId(link.boundUpdateEventId))
    return 'invalid_observation';
  if (link.boundUpdateEventId!==task.updateEventId)
    return 'historical_task_revision';
  if (!tab || tab.projectId!==project || tab.relativePath!==link.relativePath)
    return 'not_open_disk_not_rechecked';
  if (tab.revision!==link.savedRevision)
    return 'open_saved_base_differs';
  return 'open_saved_base_matches_disk_not_rechecked';
}

/** Fresh native registered-source read is the only current-disk comparator. */
export function checkedSourceReobservation(link,task,project,preview) {
  if (!link || !task || !projectId(project) ||
      !revision(link.savedRevision) ||
      link.source!=='native_registered_source_read' ||
      !eventId(link.boundUpdateEventId) ||
      preview?.projectId!==project ||
      preview?.relativePath!==link.relativePath ||
      !revision(preview.revision) || typeof preview.text!=='string')
    throw Error('C7_INVALID_NATIVE_REOBSERVATION');
  return Object.freeze({
    relativePath:link.relativePath,
    observedDiskRevision:preview.revision,
    matchesRecordedSavedRevision:preview.revision===link.savedRevision,
    linkedToCurrentTaskUpdate:link.boundUpdateEventId===task.updateEventId,
    // This is source identity evidence only, never task acceptance.
    taskAcceptance:'not_verified',
  });
}
