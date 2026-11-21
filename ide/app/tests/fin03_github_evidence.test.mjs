import test from 'node:test';
import assert from 'node:assert/strict';
import {checkedGithubPrEvidence} from '../web/github_pr_evidence.js';
const id='ide-'+'a'.repeat(32),head='b'.repeat(40),base='c'.repeat(40);
const prs={projectId:id,readOnly:true,sourceRechecked:true,
 listingPartial:false,repository:'acme/thing',localHead:head,
 pulls:[{number:17,headSha:head,baseSha:base,localVsPrHead:'same_commit'}]};
const review={projectId:id,number:17,headSha:head,sourceRechecked:true,
 listingPartial:false,mergeAuthorized:false,branchPolicyVerified:false,
 currentApprovals:2,changesRequested:0};
const rules={projectId:id,number:17,prHeadSha:head,baseSha:base,
 sourceRechecked:true,protectionComplete:false,mergeAuthorized:false};
const ci={schemaVersion:1,projectId:id,repository:'acme/thing',
 localHead:head,localHeadChangedDuringCheck:false,runsListingPartial:false,
 credentialsUsed:false,worktree:'clean',verdict:'passed_exact_head',
 installationAcceptance:'not_tested',taskCompletionAcceptance:'not_verified',
 workflows:['standalone_ide','continuity'].map(workflow=>({
   workflow,headSha:head,verdict:'passed_exact_head',runStatus:'completed'}))};
test('FIN03 exact-head observations do not grant remote permissions',()=>{
 const v=checkedGithubPrEvidence(prs,review,rules,ci,id,17);
 assert.equal(v.reviewObserved,true);
 assert.equal(v.currentExactHeadActionsObserved,true);
 assert.equal(v.mergeAuthorized,false);
 assert.equal(v.fullRulesetsVerified,false);
 assert.equal(v.installedAppAccepted,false);
});
test('FIN03 source changed and partial CI are never proof',()=>{
 assert.equal(checkedGithubPrEvidence(prs,review,rules,
   {...ci,localHead:'d'.repeat(40)},id,17).currentExactHeadActionsObserved,false);
 assert.equal(checkedGithubPrEvidence(prs,review,rules,
   {...ci,runsListingPartial:true},id,17).currentExactHeadActionsObserved,false);
 assert.equal(checkedGithubPrEvidence({...prs,listingPartial:true},
   review,rules,ci,id,17),null);
});
