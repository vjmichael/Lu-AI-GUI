/** DEV-04.2: presentation-only exact native build and binary evidence.
 * Neither a file path nor this fingerprint is execution authority. */
const SHA=/^[a-f0-9]{64}$/;
const HEAD=/^[a-f0-9]{40}$/;
const PROJECT=/^ide-[a-f0-9]{32}$/;
const JOB=/^job-[a-f0-9]{32}$/;
const PATH=/^target\/debug\/[a-zA-Z0-9_+.-]{1,200}$/;
export function safeDebugBinaryEvidence(result,projectId) {
  if(!PROJECT.test(projectId)||!result||result.projectId!==projectId||
     !JOB.test(result.jobId)||result.recipeId!=='rust_build'||
     result.phase!=='completed'||result.nativeExitCode!==0||
     result.requiresReconciliation!==false||
     result.installedAppAccepted!==false||
     !SHA.test(result.planSha256)||!HEAD.test(result.sourceHead)||
     !SHA.test(result.inputSnapshotSha256)||
     !SHA.test(result.toolchainSha256)||
     !SHA.test(result.manifestSha256)||
     !SHA.test(result.terminalReceiptSha256)||
     !SHA.test(result.artifactManifestSha256)||
     result.artifactEvidenceState!=='read_only_post_termination_tree_hashed'||
     !result.debugBinaryCandidate) return null;
  const binary=result.debugBinaryCandidate;
  if(!PATH.test(binary.relativePath)||binary.relativePath.includes('..')||
     !Number.isSafeInteger(binary.bytes)||binary.bytes<4||
     binary.bytes>64*1024*1024||
     !SHA.test(binary.binarySha256)||
     binary.outputTreeSha256!==result.artifactManifestSha256||
     !['elf_candidate_magic_only','pe_candidate_mz_only'].includes(binary.format)||
     binary.role!=='post_termination_unique_cargo_binary_readback_not_launch_authority'||
     binary.debugSymbolsAttested!==false||
     binary.debuggerExecutionAuthorized!==false||
     binary.installedAppAccepted!==false||
     !Array.isArray(result.artifactFiles)||
     !result.artifactFiles.some(file=>file.relativePath===binary.relativePath &&
       file.bytes===binary.bytes&&file.sha256===binary.binarySha256)) return null;
  return Object.freeze({
    projectId,jobId:result.jobId,planSha:result.planSha256,
    terminalReceiptSha:result.terminalReceiptSha256,
    sourceHead:result.sourceHead,snapshotSha:result.inputSnapshotSha256,
    toolchainSha:result.toolchainSha256,outputTreeSha:binary.outputTreeSha256,
    binarySha:binary.binarySha256,relativePath:binary.relativePath,
    bytes:binary.bytes,format:binary.format,debuggerAuthorized:false,
  });
}
