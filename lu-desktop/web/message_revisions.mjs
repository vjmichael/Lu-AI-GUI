// UX-CHAT02: original ANSF messages stay immutable. Revision text is
// explicitly a human DISPLAY correction, never an authenticated tool prompt.
export const MAX_EDIT_BYTES = 16*1024;
export const MAX_VISIBLE_REVISIONS=16;
export function revisionsBySequence(entries){
  const result={};
  for(const entry of entries||[]){
    if(!Number.isSafeInteger(entry?.seq)||entry.seq<=0)continue;
    if(!Number.isSafeInteger(entry.currentVersion)||entry.currentVersion<0
      ||entry.currentVersion>MAX_VISIBLE_REVISIONS)continue;
    if(!isSha(entry.originalSha256)||!isSha(entry.currentSha256))continue;
    // A completed assistant original may exceed the correction limit.
    // Display the real original up to a bounded viewer size rather than
    // pretending that a long, genuine message has no provenance.
    if(typeof entry.originalText!=='string'||typeof entry.currentText!=='string'
      ||entry.originalText.length>256*1024||entry.currentText.length>256*1024
      ||(entry.currentVersion>0&&entry.currentText.length>MAX_EDIT_BYTES))continue;
    result[entry.seq]=entry;
  }
  return result;
}
export function isSha(value){
  return typeof value==='string'&&/^[0-9a-f]{64}$/.test(value);
}
export function canEdit(entry){
  return Number.isSafeInteger(entry?.seq)&&entry.seq>0
    &&(entry.role==='user'||entry.role==='assistant');
}
export function provenanceLabel(revision){
  if(!revision||revision.currentVersion===0)return 'Original ANSF message';
  return 'Human display correction v'+revision.currentVersion+
    ' — original model input and earlier tool actions unchanged';
}
export function draftReplyTarget(entry,revision){
  if(!canEdit(entry)||!revision||!isSha(revision.currentSha256)
    ||!Number.isSafeInteger(revision.currentVersion))return null;
  return {
    seq:entry.seq,version:revision.currentVersion,sha:revision.currentSha256,
    originalSha:revision.originalSha256,
    role:entry.role,
    displayText:revision.currentText.slice(0,1200)
  };
}

// Explicit bridge from a human DISPLAY correction to a *new* authenticated
// user reply. This only stages local composer data; the existing native
// lu_post_linked_reply command rechecks current parent version/SHA under an
// exclusive SQLite transaction. No old ANSF/model/tool turn is rewritten.
export function correctedMessageFollowUp(entry,revision){
  const target=draftReplyTarget(entry,revision);
  if(!target||!Number.isSafeInteger(revision.currentVersion)
    ||revision.currentVersion<1||revision.currentVersion>MAX_VISIBLE_REVISIONS
    ||!isSha(revision.originalSha256)
    ||revision.originalSha256===revision.currentSha256
    ||typeof revision.currentText!=='string'
    ||revision.currentText.trim().length===0)return null;
  const draft='Human correction to saved message #'+entry.seq+
    ' (display revision '+revision.currentVersion+'). '+
    'Please treat the following as a NEW human follow-up, not a change to '+
    'any earlier model answer or tool result:\n\n'+revision.currentText;
  // The native reply cap is in bytes, not JavaScript UTF-16 code units.
  if(new TextEncoder().encode(draft).byteLength>MAX_EDIT_BYTES)return null;
  return {target,draft};
}
