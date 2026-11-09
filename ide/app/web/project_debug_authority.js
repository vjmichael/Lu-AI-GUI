// DEV-04.4: read-only verification of exact native project LLDB receipts.
// UI objects are not permission or native OS cleanup proof.
import {safeDebugSymbolEvidence} from './debug_symbol_evidence.js';
const SHA=/^[a-f0-9]{64}$/;
const JOB=/^job-[a-f0-9]{32}$/;
export function checkedProjectDebugWitness(result,build,projectId) {
  const witness=safeDebugSymbolEvidence(build,projectId);
  if(!witness||!result||result.projectId!==projectId||
     result.target!=='one_exact_private_project_build_in_protected_job'||
     !JOB.test(result.jobId)||result.jobId===witness.jobId||
     result.originBuildJobId!==witness.jobId||
     result.originBuildPlanSha256!==witness.planSha||
     result.originBuildReceiptSha256!==witness.terminalReceiptSha||
     result.binarySha256!==witness.binarySha||
     result.symbolSha256!==witness.symbolsSha||
     !SHA.test(result.targetBindingSha256)||
     !SHA.test(result.planSha256)||
     result.sourceHead!==build.sourceHead||
     result.bundleSha256==null||!SHA.test(result.bundleSha256)||
     result.terminalPhase!=='completed'||
     result.verifiedOsTreeReleaseByWorker!==true||
     result.threadsResponseObserved!==true||
     result.stackResponseObserved!==true||
     !(Number.isSafeInteger(result.stoppedThread)&&result.stoppedThread>0)||
     result.installedAppAccepted!==false)return null;
  // A historical journal result or raw LLDB output cannot recreate this
  // exact session-scoped native outcome after the desktop restarts.
  return Object.freeze({
    projectId,debugJobId:result.jobId,buildJobId:witness.jobId,
    debugPlanSha:result.planSha256,buildPlanSha:witness.planSha,
    buildReceiptSha:witness.terminalReceiptSha,
    binarySha:witness.binarySha,symbolSha:witness.symbolsSha,
    targetBindingSha:result.targetBindingSha256,
    kernelTreeReleased:true,installedAccepted:false,
  });
}
