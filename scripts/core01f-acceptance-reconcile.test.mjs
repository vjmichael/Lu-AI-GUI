import test from 'node:test';
import assert from 'node:assert/strict';
import {DEFAULT_BRANCH,NATIVE,FULL_NATIVE,INSTALLED_JOBS,reconcile} from './core01f-acceptance-reconcile.mjs';
const SHA='a'.repeat(40),BRANCH=DEFAULT_BRANCH;
const groups=(id,path,rows)=>({runId:id,
 run:{id,head_sha:SHA,head_branch:BRANCH,path,status:'completed',conclusion:'success',event:'pull_request'},
 jobs:rows.map(([name],j)=>({id:id*100+j+1,run_id:id,name,status:'completed',conclusion:'success'})),
 artifacts:rows.filter(x=>x[1]).map(([name,prefix],j)=>({
   id:id*1000+j+1,name:prefix+'-'+SHA,size_in_bytes:120,expired:false,
   digest:'sha256:'+'b'.repeat(64),workflow_run:{id,head_sha:SHA,head_branch:BRANCH}
 }))});
const make=()=>({sha:SHA,native:groups(101,'.github/workflows/lu-repo01-full-source.yml',NATIVE),
 fullNative:groups(102,'.github/workflows/lu-repo01-full-source.yml',FULL_NATIVE),
 installed:groups(103,'.github/workflows/lu-plan07d-installed.yml',INSTALLED_JOBS)});
test('fully green metadata is reviewable only, never certified native, installed or merge',()=>{
 const r=reconcile(make());assert.equal(r.status,'METADATA_REVIEWABLE_LOGS_REQUIRED');
 assert.deepEqual(r.blockers,[]);
 for(const name of ['originalArtifactContentsReopened','kernelReceiptsReconciled',
   'installedHumanInteractionVerified','sec06fAccepted','nativeWindowsLinuxAccepted',
   'hostedInstalledAccepted','mergeAuthorized','releaseAuthorized'])
   assert.equal(r[name],false);
});
test('genuine failed Chromium and model security jobs cannot pass',()=>{
 const x=make();
 for(const j of x.native.jobs.filter(j=>/Chromium and Node|real Qwen/.test(j.name)))
   j.conclusion='failure';
 const r=reconcile(x);assert.ok(r.blockers.some(s=>s.includes('Chromium')));
 assert.ok(r.blockers.some(s=>s.includes('Qwen')));
});
test('installed skips, source mismatches and missing native originals all block',()=>{
 const x=make();x.installed.jobs[0].conclusion='skipped';
 x.native.run.head_sha='c'.repeat(40);x.fullNative.jobs=[];
 const problems=reconcile(x).blockers;
 assert.ok(problems.some(x=>x.includes('installed:JOB_FAILED_SKIPPED')));
 assert.ok(problems.some(x=>x.includes('native:WRONG_SOURCE_RUN')));
 assert.ok(problems.some(x=>x.includes('full-native:MISSING_JOB')));
});
test('expired, forged-digest, wrong-run artifacts are not evidence',()=>{
 const x=make();x.native.artifacts[0].expired=true;
 x.native.artifacts[1].digest='sha256:garbage';
 x.native.artifacts[2].workflow_run.id=666;
 assert.equal(reconcile(x).blockers.filter(z=>z.includes('ARTIFACT_ORIGIN_DIGEST')).length,3);
});
test('duplicate jobs/artifacts and absent SEC06F are rejected',()=>{
 const x=make();
 x.native.jobs=x.native.jobs.filter(j=>!j.name.includes('SEC-06F'));
 x.native.jobs.push(x.native.jobs[0]);
 x.native.artifacts.push(x.native.artifacts[0]);
 const r=reconcile(x);
 assert.ok(r.blockers.some(z=>z.includes('MISSING_JOB:SEC-06F')));
 assert.ok(r.blockers.some(z=>z.includes('DUPLICATE_JOB:verify-source')));
 assert.ok(r.blockers.some(z=>z.includes('DUPLICATE_ARTIFACT')));
});
test('untrusted source SHA is rejected and no installed run is not accepted',()=>{
 assert.throws(()=>reconcile({...make(),sha:'wrong'}),/EXPECTED_SHA_INVALID/);
 const x=make();x.installed=undefined;
 assert.ok(reconcile(x).blockers.some(z=>z.includes('installed:WRONG_SOURCE_RUN')));
});

test('current candidate branch must match original run and artifact metadata',()=>{
 const x=make();x.native.run.head_branch='lu/wrong-candidate';
 assert.ok(reconcile(x).blockers.includes('native:WRONG_SOURCE_RUN_OR_WORKFLOW'));
 const y=make();y.native.artifacts[0].workflow_run.head_branch='lu/wrong-candidate';
 assert.ok(reconcile(y).blockers.some(b=>b.includes('ARTIFACT_ORIGIN_DIGEST')));
 assert.throws(()=>reconcile({...make(),expectedBranch:'../unsafe'}),/EXPECTED_BRANCH_INVALID/);
 assert.throws(()=>reconcile({...make(),expectedBranch:'lu/bad..ref'}),/EXPECTED_BRANCH_INVALID/);
});

test('historical legacy branch can be checked only with explicit original branch argument',()=>{
 const legacy='lu/core01-source-convergence-no-ide-no-kobold-20261112';
 const x=make();
 for(const group of [x.native,x.fullNative,x.installed]){
   group.run.head_branch=legacy;
   for(const a of group.artifacts)a.workflow_run.head_branch=legacy;
 }
 assert.ok(reconcile(x).blockers.some(b=>b.includes('WRONG_SOURCE_RUN')));
 const replay=reconcile({...x,expectedBranch:legacy});
 assert.deepEqual(replay.blockers,[]);
 assert.equal(replay.mergeAuthorized,false);
});
