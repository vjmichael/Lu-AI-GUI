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
     result.installedAppAccepted!==false ||
     (result.releaseOwnerSourceLineVerified!==undefined &&
       typeof result.releaseOwnerSourceLineVerified!=='boolean') ||
     (result.releaseOwnerControlAuthorized!==undefined &&
       result.releaseOwnerControlAuthorized!==false) ||
     (result.releaseOwnerSourceLineVerified===true &&
       (result.releaseOwnerKernelVerified!==true ||
        !SHA.test(result.releaseOwnerVerifiedSourceSha256||'') ||
        !SHA.test(result.releaseOwnerVerifiedPdbSha256||'') ||
        result.releaseOwnerVerifiedPdbSha256!==witness.symbolsSha ||
        !Number.isSafeInteger(result.releaseOwnerVerifiedSourceLine) ||
        result.releaseOwnerVerifiedSourceLine<1 ||
        result.releaseOwnerVerifiedSourceLine!==result.stoppedSource?.line ||
        result.releaseOwnerVerifiedSourceSha256!==result.stoppedSource?.diskSha256)) ||
     (result.releaseOwnerSourceLineVerified!==true &&
       (result.releaseOwnerVerifiedSourceLine!=null ||
        result.releaseOwnerVerifiedSourceSha256!=null ||
        result.releaseOwnerVerifiedPdbSha256!=null)) ||
     (result.releaseOwnerKernelVerified===true &&
       (!SHA.test(result.releaseOwnerReceiptSha256||'') ||
        !Number.isSafeInteger(result.releaseOwnerEventSequence) ||
        result.releaseOwnerEventSequence<1)) ||
     (result.releaseOwnerKernelVerified===false &&
       (result.releaseOwnerReceiptSha256!==null ||
        result.releaseOwnerEventSequence!==null)))return null;
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
