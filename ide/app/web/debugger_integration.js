/**
 * IDE-INT04 — original OS-contained LLDB job to readable editor evidence.
 * The debugger is never controlled from this module. DAP strings are claims;
 * exact saved file bytes and original journal remain separate native checks.
 */
import {checkedProjectDebugWitness} from './project_debug_authority.js';
const SHA=/^[a-f0-9]{64}$/;
const JOB=/^job-[a-f0-9]{32}$/;
const PROJECT=/^ide-[a-f0-9]{32}$/;
const SOURCE=/^[^\\:\u0000-\u001f\u007f]+$/u;
function safeSource(relativePath) {
  return typeof relativePath==='string' && relativePath.length>0 &&
    relativePath.length<=512 && SOURCE.test(relativePath) &&
    !relativePath.startsWith('/') && relativePath.split('/')
      .every(part=>part && part!=='.' && part!=='..');
}
export function debugAdmissionStatus(value,projectId) {
  if (!PROJECT.test(projectId||'') || !value ||
      typeof value.ready!=='boolean' ||
      typeof value.projectDebugSupported!=='boolean' ||
      typeof value.operationControlSupported!=='boolean' ||
      !Array.isArray(value.journal) || !Array.isArray(value.quarantined))
    return Object.freeze({ready:false,projectDebugSupported:false,
      operationControlSupported:false,productionOperationsAvailable:false,
      operationControlMode:'unavailable',blockedReason:'Debug host unavailable'});
  const global=value.globalRecoveryRequired===true ||
    value.quarantined.some(row=>row?.requiresReconciliation===true);
  const elsewhere=value.activeInAnotherProject===true;
  const actions=value.orphanedDebugActionRequired===true;
  const occupied=!!value.current;
  const ready=value.ready && !global && !elsewhere && !actions && !occupied;
  const developerOnly=value.operationControlCode===
    'DEV046D_NATIVE_DIALOG_DEV_ONLY_NOT_RELEASE_ACCEPTED';
  const productionAction= value.operationControlCode===
    'FIN04_NATIVE_RELEASE_ACTIONS_INDEPENDENTLY_ADMITTED';
  return Object.freeze({
    ready,projectDebugSupported:ready && value.projectDebugSupported,
    operationControlSupported:ready && value.operationControlSupported &&
      (developerOnly || productionAction),
    productionOperationsAvailable:ready && value.operationControlSupported &&
      productionAction,
    operationControlMode:developerOnly?'developer_only':
      productionAction?'release_candidate':'unavailable',
    blockedReason:global?'Original protected debugger OS cleanup required':
      actions?'Interrupted debugger action journal requires native cleanup':
      elsewhere?'Another registered project owns the protected debugger':
      occupied?'Protected debugger session already running':
      ready && developerOnly?'Developer-only debugger action; not production accepted':
      ready?'Native debugger available with fresh OS approval':
      'Native debugger release resources or authorization unavailable',
  });
}

/** Client accepts only source metadata already bound to a completed original
 * project-debugging witness. No untrusted DAP absolute path survives. */
export function checkedDebugStoppedSource(result,build,projectId) {
  const verified=checkedProjectDebugWitness(result,build,projectId);
  const source=result?.stoppedSource;
  if (!verified || !SHA.test(result?.terminalReceiptSha256||'') ||
      !source || !safeSource(source.relativePath) ||
      !SHA.test(source.diskSha256||'') ||
      !Number.isSafeInteger(source.line) ||
      source.line<1 || source.line>250000 ||
      source.characterUtf16!==0 ||
      !Number.isSafeInteger(source.frameId) ||
      source.frameId<1 || source.frameId>1000000 ||
      source.sourceBytesVerified!==true ||
      source.osInstructionSourceLineVerified!==false ||
      source.installedAppAccepted!==false) return null;
  return Object.freeze({
    projectId,jobId:result.jobId,
    planSha256:result.planSha256,
    terminalReceiptSha256:result.terminalReceiptSha256,
    relativePath:source.relativePath,diskSha256:source.diskSha256,
    line:source.line,frameId:source.frameId,
    initialStopOnly:true,adapterReported:true,
    independentOsLineMapVerified:false,
    executionAuthorized:false,
  });
}
export function checkedReviewedDebugSource(candidate,response) {
  const source=response?.source;
  if (!candidate || !source ||
      response.projectId!==candidate.projectId ||
      response.jobId!==candidate.jobId ||
      response.planSha256!==candidate.planSha256 ||
      response.terminalReceiptSha256!==candidate.terminalReceiptSha256 ||
      response.navigationAutomatic!==false ||
      response.debuggerExecutionAuthorized!==false ||
      response.humanTaskAccepted!==false ||
      source.relativePath!==candidate.relativePath ||
      source.diskSha256!==candidate.diskSha256 ||
      source.line!==candidate.line || source.frameId!==candidate.frameId ||
      source.characterUtf16!==0 || source.sourceBytesVerified!==true ||
      source.osInstructionSourceLineVerified!==false ||
      source.installedAppAccepted!==false) return null;
  return candidate;
}
export function checkedRedactedDebugLocals(result,build,projectId) {
  if (!checkedProjectDebugWitness(result,build,projectId) ||
      result.operationOutcomeCode!=='native_dialog_variables_redacted_uncertain' ||
      !SHA.test(result.operationReceiptSha256||'') ||
      !Number.isSafeInteger(result.inspectedFrameId) ||
      result.inspectedFrameId<1 || result.inspectedFrameId>1000000 ||
      !Array.isArray(result.redactedVariables) ||
      result.redactedVariables.length>16) return null;
  const names=new Set();
  const rows=[];
  for(const row of result.redactedVariables){
    if (!row || row.valueRedacted!==true ||
        typeof row.name!=='string' || !/^[A-Za-z0-9_]{1,64}$/.test(row.name) ||
        names.has(row.name) ||
        !(row.declaredType==null ||
          (typeof row.declaredType==='string' &&
           row.declaredType.length<=64 &&
           !/[\u0000-\u001f\u007f]/.test(row.declaredType)))) return null;
    names.add(row.name);
    rows.push(Object.freeze({
      name:row.name,declaredType:row.declaredType||'type not reported',
      value:'redacted',adapterReported:true,
    }));
  }
  return Object.freeze(rows);
}
