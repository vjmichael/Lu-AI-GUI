/** FIN03 6E-04: exact PR HEAD / original CI / review / public rules join.
 * This is observation-only; it can NEVER produce merge authority. */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const HEAD=/^[a-f0-9]{40}$/i;
const SAFE_REPO=/^[a-z0-9_.-]+\/[a-z0-9_.-]+$/i;
export function checkedGithubPrEvidence(pulls,review,rules,ci,projectId,number) {
 if(!PROJECT.test(projectId||'') ||
    pulls?.projectId!==projectId || pulls.readOnly!==true ||
    pulls.sourceRechecked!==true || pulls.listingPartial!==false ||
    !Array.isArray(pulls.pulls) || pulls.pulls.length>20 ||
    !SAFE_REPO.test(pulls.repository||'') ||
    !HEAD.test(pulls.localHead||'') ||
    !Number.isSafeInteger(number) || number<=0)return null;
 const pr=pulls.pulls.find(p=>p.number===number);
 if(!pr || pr.localVsPrHead!=='same_commit' ||
    pr.headSha!==pulls.localHead || !HEAD.test(pr.baseSha||''))return null;
 const reviewObserved=review?.projectId===projectId &&
    review.number===number && review.headSha===pr.headSha &&
    review.sourceRechecked===true &&
    review.listingPartial===false &&
    review.mergeAuthorized===false &&
    review.branchPolicyVerified===false &&
    Number.isSafeInteger(review.currentApprovals) &&
    Number.isSafeInteger(review.changesRequested);
 const ruleObserved=rules?.projectId===projectId &&
    rules.number===number && rules.prHeadSha===pr.headSha &&
    rules.baseSha===pr.baseSha &&
    rules.sourceRechecked===true &&
    rules.protectionComplete===false &&
    rules.mergeAuthorized===false;
 const exactHeadCi=ci?.schemaVersion===1 &&
    ci.projectId===projectId && ci.repository===pulls.repository &&
    ci.localHead===pr.headSha &&
    ci.localHeadChangedDuringCheck===false &&
    ci.runsListingPartial===false &&
    ci.credentialsUsed===false &&
    ci.worktree==='clean' &&
    ci.verdict==='passed_exact_head' &&
    ci.installationAcceptance==='not_tested' &&
    ci.taskCompletionAcceptance==='not_verified' &&
    Array.isArray(ci.workflows) &&
    ci.workflows.length===2 && ci.workflows.every(w=>
      w.headSha===pr.headSha && w.verdict==='passed_exact_head' &&
      w.runStatus==='completed');
 return Object.freeze({
   projectId,number,prHeadSha:pr.headSha,
   state:'public_non_atomic_evidence_only',
   reviewObserved:!!reviewObserved,
   observedApprovals:reviewObserved?review.currentApprovals:null,
   observedChangesRequested:reviewObserved?review.changesRequested:null,
   branchProtectionPartial:!!ruleObserved,
   currentExactHeadActionsObserved:!!exactHeadCi,
   fullRulesetsVerified:false,mergeAuthorized:false,
   remotePublicationAuthorized:false,
   installedAppAccepted:false,taskAccepted:false,
 });
}
