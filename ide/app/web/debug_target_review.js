// DEV-04.1 host-authored non-executing debugger target review.
const SHA=/^[a-f0-9]{64}$/;
const HEAD=/^[a-f0-9]{40}$/;
const PROJECT=/^ide-[a-f0-9]{32}$/;
export function safeDebugTargetReview(raw,projectId,expectedSha){
  if(!PROJECT.test(projectId)||!SHA.test(expectedSha)||
     raw?.schemaVersion!==1||raw.projectId!==projectId||
     raw.profileId!=="rust_cargo_debug_candidate"||
     raw.manifestRelativePath!=="Cargo.toml"||
     raw.manifestSha256!==expectedSha||
     !HEAD.test(raw.cleanGitHead)||!SHA.test(raw.reviewFingerprint)||
     raw.sourceSnapshotState!=="not_sealed"||
     raw.buildReceiptState!=="no_completed_native_build_bound"||
     raw.binaryIdentityState!=="no_binary_sha_or_provenance"||
     raw.debugSymbolsState!=="not_attested"||
     raw.debuggerAdapterState!=="fixed_self_test_only_no_project_target"||
     raw.targetExecutable!==null || raw.sourceTextExported!==false||
     raw.binaryExecuted!==false||raw.workerGrantIssued!==false||
     raw.debuggerAuthorized!==false||raw.installedAppAccepted!==false)
     throw Error("DEV041_DEBUG_REVIEW_NOT_TRUSTED");
  return Object.freeze({projectId,profileId:raw.profileId,
    cleanGitHead:raw.cleanGitHead,manifestSha:raw.manifestSha256,
    fingerprint:raw.reviewFingerprint,executable:null,
    authorized:false});
}
