/**
 * INT05-D: presentation-only validator for native terminal sessions.
 * No client command parser grants authority; Rust decides the fixed action,
 * and native protected workers independently authorize each run.
 */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const SESSION=/^term-[a-f0-9]{16}$/;
const ACTIONS=new Set(['help','packages','status','native_job_result',
  'cancellation_requested']);
export function checkedTerminalSession(value,projectId) {
  if (!PROJECT.test(projectId||'') || value?.projectId!==projectId ||
      !SESSION.test(value.sessionId||'') || value.sequence!==0 ||
      value.interactiveTyped!==true ||
      value.arbitraryShellEnabled!==false ||
      value.authorizationGranted!==false) return null;
  return Object.freeze({projectId,sessionId:value.sessionId,sequence:0});
}
export function checkedTerminalReply(value,session,sequence) {
  if (!session || !value || value.projectId!==session.projectId ||
      value.sessionId!==session.sessionId ||
      value.sequence!==sequence || session.sequence+1!==sequence ||
      !ACTIONS.has(value.kind) ||
      typeof value.message!=='string' || value.message.length>512 ||
      value.arbitraryShellEnabled!==false ||
      value.authorizationGrantedByTerminal!==false ||
      value.installedAppAccepted!==false) return null;
  if (value.kind==='native_job_result') {
    const result=value.nativeResult;
    if (!result || result.projectId!==session.projectId ||
        !/^job-[a-f0-9]{32}$/.test(result.jobId||'') ||
        !/^[a-f0-9]{64}$/.test(result.terminalReceiptSha256||'') ||
        result.installedAppAccepted!==false ||
        !['completed','cancelled','failed','denied','recovery_required',
          'uncertain'].includes(result.phase)) return null;
  }
  return Object.freeze({kind:value.kind,message:value.message,
    nativeResult:value.nativeResult||null,
    projectId:session.projectId,sessionId:session.sessionId,sequence,
    installedAppAccepted:false,shellEnabled:false});
}
