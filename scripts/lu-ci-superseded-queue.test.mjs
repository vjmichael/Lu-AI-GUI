import test from "node:test";
import assert from "node:assert/strict";
import { selectSuperseded, argsFrom, execute } from "./lu-ci-superseded-queue.mjs";
const newSha = "a".repeat(40), oldSha = "b".repeat(40), branch = "lu/memory-loop03b-example";
const mk = (id, overrides = {}) => ({
  id, name: "Lu tests", status: "queued", event: "pull_request",
  head_branch: branch, head_sha: oldSha, ...overrides
});

test("selects only queued old-SHA same-branch automatic runs", () => {
  const out = selectSuperseded([
    mk(1),mk(2,{head_sha:newSha}),mk(3,{event:"workflow_dispatch"}),
    mk(4,{status:"in_progress"}),mk(5,{head_branch:"lu/other"}),
    mk(6,{head_sha:"invalid"}),mk(1)
  ],branch,newSha);
  assert.deepEqual(out.map((r)=>r.id),[1]);
});

test("explicit authorization, branch and head lease are mandatory to apply", () => {
  assert.throws(()=>argsFrom(["--branch",branch,"--apply"]));
  assert.throws(()=>argsFrom(["--branch","main"]));
  assert.throws(()=>argsFrom(["--branch",branch,"--limit","500"]));
  assert.equal(argsFrom(["--branch",branch]).apply,false);
  assert.equal(argsFrom(["--branch",branch,"--apply","--confirm-head",newSha]).apply,true);
});

test("dry run never sends cancellation requests", async () => {
  const paths = [];
  const fetcher = async (url,init={}) => {
    paths.push({url,method:init.method||"GET"});
    return {ok:true,status:200,json:async()=>url.includes("/branches/") ?
      {commit:{sha:newSha}}:{workflow_runs:[mk(10),mk(12,{event:"workflow_dispatch"})]}};
  };
  const report = await execute(argsFrom(["--branch",branch]),fetcher);
  assert.deepEqual(report.selected.map((x)=>x.id),[10]);
  assert.equal(report.mode,"DRY_RUN");
  assert.ok(paths.every((p)=>p.method==="GET"));
});

test("apply rejects mismatched branch head before touching Actions", async () => {
  const a = argsFrom(["--branch",branch,"--apply","--confirm-head",oldSha]);
  const urls=[];
  await assert.rejects(execute(a,async(url)=>{urls.push(url);
    return {ok:true,status:200,json:async()=>({commit:{sha:newSha}})};
  }), /Current head changed/);
  assert.equal(urls.length,1);
});
