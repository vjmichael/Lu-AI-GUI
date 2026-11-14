// IDE-INT01-D: a saved workspace is navigation, not a draft or authority.
// This module makes a read-only, source-free recovery report. All source bytes
// remain in ephemeral native previews; no disk writes, worker launches, or
// automatic checkpoint application occur here.
import { checkedRememberedWorkspace, revalidatedResumeDecision } from './workspace_layout.js';
import { captureLiveEditorIntent } from './editor_live_write.js';
import { verifiedRelativeSourcePath } from './editor_working_set.js';

const sha = value => typeof value === 'string' && /^[a-f0-9]{64}$/i.test(value);
const keyFor = (projectId, path) => projectId + ':' + path;

export function checkedPrivateDraftHead(record, projectId, relativePath) {
  if (record == null) return null;
  if (record.schemaVersion !== 1 || record.projectId !== projectId ||
      record.relativePath !== relativePath ||
      !sha(record.checkpointId) || !sha(record.draftSha256) ||
      !sha(record.baseDiskRevision) ||
      !Number.isSafeInteger(record.bufferRevision) ||
      record.bufferRevision < 0 ||
      typeof record.bufferId !== 'string' ||
      !/^[a-zA-Z0-9-]{8,128}$/.test(record.bufferId))
    throw Error('INT01D_UNTRUSTED_PRIVATE_DRAFT_HEAD');
  // The native vault returned a verified immutable checkpoint. The report
  // never retains the source text included in the response.
  return Object.freeze({
    schemaVersion:1,projectId,relativePath,
    checkpointId:record.checkpointId,draftSha256:record.draftSha256,
    baseDiskRevision:record.baseDiskRevision,bufferId:record.bufferId,
    bufferRevision:record.bufferRevision,
  });
}

export function planRestartReconciliation(layout, observations, existingTabs,
    getLiveIdentity, getEditorState) {
  const selected=checkedRememberedWorkspace(layout,layout?.projectId);
  if (!Array.isArray(observations) ||
      observations.length !== selected.files.length ||
      !Array.isArray(existingTabs) ||
      typeof getLiveIdentity !== 'function' ||
      typeof getEditorState !== 'function')
    throw Error('INT01D_RESTART_OBSERVATIONS_INVALID');
  const seen=new Set(), rows=[], toOpen=[];
  for (let index=0;index<selected.files.length;index++) {
    const file=selected.files[index], observation=observations[index];
    if (!observation || observation.relativePath!==file.relativePath ||
        seen.has(file.relativePath) ||
        verifiedRelativeSourcePath(file.relativePath)!==file.relativePath)
      throw Error('INT01D_RESTART_OBSERVATIONS_INVALID');
    seen.add(file.relativePath);
    const key=keyFor(selected.projectId,file.relativePath);
    const sameTab=existingTabs.find(tab=>tab?.key===key &&
      tab.projectId===selected.projectId);
    const preview=observation.preview??null;
    const decision=revalidatedResumeDecision(selected,file,preview,selected.projectId);
    const verified=decision.action==='open_saved';
    // Missing or untrusted native preview is explicitly visible. Do not
    // manufacture a tab from a private draft without a valid saved source.
    const diskRevision=verified?preview.revision:null;
    const draft=checkedPrivateDraftHead(observation.draft??null,
      selected.projectId,file.relativePath);
    const live=sameTab?captureLiveEditorIntent(sameTab,
      getLiveIdentity(key),getEditorState(key),selected.projectId):null;
    const status=!verified?'unavailable_or_untrusted_source':
      sameTab ? live.status!=='current'?'existing_editor_unverified':
        live.dirty?'existing_unsaved_editor_preserved':
          'existing_editor_preserved':
      decision.changedOnDisk?'reopen_changed_saved_file':'reopen_saved_file';
    if (verified && !sameTab) toOpen.push(preview);
    rows.push(Object.freeze({
      key,relativePath:file.relativePath,status,
      savedRevision:file.savedRevision,
      currentDiskRevision:diskRevision,
      diskChanged:verified && decision.changedOnDisk,
      wasDirtyBeforeRestart:file.wasDirty,
      draftStatus:observation.draftUnavailable===true?'checkpoint_inspection_unavailable':
        !draft?'no_checkpoint_observed':
        !verified?'checkpoint_source_unavailable':
          draft.baseDiskRevision===diskRevision?'checkpoint_same_disk_base':
            'checkpoint_disk_changed',
      checkpointId:draft?.checkpointId??null,
      // This row deliberately contains no source text or arbitrary OS path.
    }));
  }
  const summary=Object.freeze({
    files:rows.length,
    available:rows.filter(row=>row.currentDiskRevision!==null).length,
    missing:rows.filter(row=>row.currentDiskRevision===null).length,
    changedOnDisk:rows.filter(row=>row.diskChanged).length,
    previousUnsaved:rows.filter(row=>row.wasDirtyBeforeRestart).length,
    checkpoints:rows.filter(row=>row.checkpointId!==null).length,
    draftChecksUnavailable:rows.filter(row=>
      row.draftStatus==='checkpoint_inspection_unavailable').length,
    existingEditorsPreserved:rows.filter(row=>row.status.startsWith('existing_')).length,
  });
  return Object.freeze({projectId:selected.projectId,rows,summary,toOpen});
}
