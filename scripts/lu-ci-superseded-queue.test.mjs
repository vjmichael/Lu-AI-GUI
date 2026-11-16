import test from "node:test";
import assert from "node:assert/strict";
import { selectSuperseded, argsFrom, execute } from "./lu-ci-superseded-queue.mjs";
const newSha = "a".repeat(40), oldSha = "b".repeat(40);
const branch = "lu/memory-loop03b-example";
const timestamp = (minutesAgo) => new Date(Date.now()-minutesAgo*60_000).toISOString();
const mk = (id, overrides = {}) => ({
  id, name: "Lu tests", workflow_id: 42, status: "queued",
  event: "pull_request", head_branch: branch, head_sha: oldSha,
  created_at: timestamp(60), conclusion: null,
  pull_requests: [{number: 110}], ...overrides
});
const successor = (id = 99, overrides = {}) => mk(id, {
  head_sha:newSha, created_at: timestamp(4), ...overrides
});

test("selects only old queued same-workflow original-PR runs behind a current head", () => {
  const out = selectSuperseded([
    mk(1), mk(2,{head_sha:newSha}),mk(3,{event:"workflow_dispatch"}),
    mk(4,{status:"in_progress"}),mk(5,{head_branch:"lu/other"}),
    mk(6,{head_sha:"invalid"}),mk(1),successor(),
  ],branch,newSha);
  assert.deepEqual(out.map((r)=>r.id),[1]);
  assert.equal(out[0].verified_successor_id,99);
});

test("historical 30-minute and matching workflow/PR successor fences", () => {
  const conditions = [
    mk(10,{created_at:timestamp(5)}),
    mk(11,{workflow_id:43}),
    mk(12,{pull_requests:[{number:777}]}),
    mk(13,{status:"waiting"}),
    mk(14,{status:"in_progress"}),
    mk(15,{event:"push"}),
  ];
  assert.deepEqual(selectSuperseded([...conditions,successor()],branch,newSha),[]);
  assert.deepEqual(selectSuperseded([
    mk(21),successor(88,{status:"completed",conclusion:"failure"})
  ],branch,newSha),[]);
  assert.deepEqual(selectSuperseded([
    mk(22),successor(89,{status:"completed",conclusion:"cancelled"})
  ],branch,newSha),[]);
  assert.deepEqual(selectSuperseded([mk(30),successor(90,{
    pull_requests:[{number:900}]
  })],branch,newSha),[]);
});

test("explicit authorization, branch and head lease are mandatory to apply", () => {
  assert.throws(()=>argsFrom(["--branch",branch,"--apply"]));
  assert.throws(()=>argsFrom(["--branch","main"]));
  assert.throws(()=>argsFrom(["--branch",branch,"--limit","500"]));
  assert.equal(argsFrom(["--branch",branch]).apply,false);
  assert.equal(argsFrom(["--branch",branch,"--apply","--confirm-head",newSha]).apply,true);
});

test("dry run does not cancel and reports exact verified successor", async () => {
  const paths = [];
  const fetcher = async (url,init={}) => {
    paths.push({url,method:init.method||"GET"});
    return {ok:true,status:200,json:async()=>url.includes("/branches/") ?
      {commit:{sha:newSha}}:{workflow_runs:[mk(10),successor(),mk(12,{event:"workflow_dispatch"})]}};
  };
  const report = await execute(argsFrom(["--branch",branch]),fetcher);
  assert.deepEqual(report.selected.map((x)=>x.id),[10]);
  assert.equal(report.selected[0].successorId,99);
  assert.equal(report.mode,"DRY_RUN");
  assert.ok(paths.every((p)=>p.method==="GET"));
});

test("apply rejects changed branch before any run cancellation", async () => {
  const a = argsFrom(["--branch",branch,"--apply","--confirm-head",oldSha]);
  const urls=[];
  await assert.rejects(execute(a,async(url)=>{urls.push(url);
    return {ok:true,status:200,json:async()=>({commit:{sha:newSha}})};
  }), /Current head changed/);
  assert.equal(urls.length,1);
});

test("apply rechecks original and successor before sending a cancellation", async () => {
  const opts=argsFrom(["--branch",branch,"--apply","--confirm-head",newSha]);
  const methods=[];
  const fetcher=async(url,init={})=>{
    methods.push(init.method||"GET");
    let value;
    if(url.includes("/branches/")) value={commit:{sha:newSha}};
    else if(url.endsWith("/actions/runs/10")) value=mk(10);
    else if(url.endsWith("/actions/runs/99")) value=successor(99,{status:"completed",conclusion:"failure"});
    else value={workflow_runs:[mk(10),successor(99)]};
    return {ok:true,status:200,json:async()=>value};
  };
  await assert.rejects(execute(opts,fetcher),/successor identity, age or status changed/);
  assert.ok(methods.every((x)=>x==="GET"));
});

test("apply cancels only a still-queued exact run with a live verified successor", async () => {
  const opts=argsFrom(["--branch",branch,"--apply","--confirm-head",newSha]);
  const cancels=[];
  const fetcher=async(url,init={})=>{
    if(init.method==="POST"){
      cancels.push(url);
      return {ok:true,status:202,json:async()=>({})};
    }
    let value;
    if(url.includes("/branches/")) value={commit:{sha:newSha}};
    else if(url.endsWith("/actions/runs/10")) value=mk(10);
    else if(url.endsWith("/actions/runs/99")) value=successor();
    else value={workflow_runs:[mk(10),successor()]};
    return {ok:true,status:200,json:async()=>value};
  };
  const report=await execute(opts,fetcher);
  assert.equal(report.cancellationRequests[0].accepted,true);
  assert.equal(cancels.length,1);
  assert.match(cancels[0],/\/actions\/runs\/10\/cancel$/);
});
