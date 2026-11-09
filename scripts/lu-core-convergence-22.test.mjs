import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
const doc=JSON.parse(readFileSync(new URL('../docs/LU_CORE_DIVERGENT_TIP_RESOLUTION_20261109.json',import.meta.url),'utf8'));
const graph=JSON.parse(readFileSync(new URL('../docs/LU_CORE_OPEN_PR_STACK_20261109.json',import.meta.url),'utf8'));
const read=p=>readFileSync(new URL('../'+p,import.meta.url),'utf8');
test("every original divergent Core tip individually classified with exact source SHA and decision",()=>{
 const candidates=graph.outsideCoreBaseChain.filter(x=>x.gitRelation==='diverged');
 assert.equal(candidates.length,22);assert.equal(doc.entries.length,22);
 const byNumber=new Map(doc.entries.map(x=>[x.pr,x]));
 assert.equal(byNumber.size,22);
 for(const p of candidates){
  const row=byNumber.get(p.number);assert.ok(row,"unclassified PR "+p.number);
  assert.equal(row.branch,p.headBranch);assert.equal(row.sourceSha,p.headSha);
  assert.equal(row.sourceCommitsNotInOriginalCore,p.commitsNotInCoreHead);
  assert.ok(row.disposition&&row.finding.length>25&&row.decision.length>25);
  assert.ok(row.evidencePath.length>0);assert.equal(row.nativeAcceptanceForTip,false);
 }
});
test("Observe and custody retained source repairs without permission replay or fake acceptance",()=>{
 const get=n=>doc.entries.find(x=>x.pr===n);
 assert.equal(get(61).disposition,'SELECTIVELY_ADOPTED');
 assert.equal(get(133).disposition,'SELECTIVELY_ADOPTED');
 assert.match(read('lu-desktop/src-tauri/src/archive_desktop.rs'),/drop\(pinned\);\s*drop\(q\);/);
 assert.match(read('src-tauri/lu-win-worker-job/src/bin/lu-linux-cgroup-guard.rs'),/remove_owned_empty_subtree/);
 assert.match(read('benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py'),/expected_nested/);
 assert.match(read('lu-desktop/tests/browser-ci-candidate.test.mjs'),/LU_BROWSER_REQUIRE_CANDIDATE/);
});
test("queue #112 does not bring automatic native fanout or unsafe cancellation",()=>{
 const queue=doc.entries.find(x=>x.pr===112);
 assert.equal(queue.disposition,'SUPERSEDED_CI_POLICY');
 const policy=read('scripts/lu-ci-group-policy.mjs');
 const current=read('scripts/lu-ci-superseded-queue.mjs');
 assert.match(policy,/HEAVY_WORKFLOW_AUTOMATIC_FANOUT/);
 assert.match(current,/freshBranch/);assert.match(current,/r\.status === "queued"/);
 assert.ok(!doc.entries.some(x=>x.nativeAcceptanceForTip));
});
