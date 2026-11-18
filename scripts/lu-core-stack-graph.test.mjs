import test from "node:test";
import assert from "node:assert/strict";
import {buildSnapshot} from "./lu-core-stack-graph.mjs";
const sha=n=>String(n).repeat(40);
const pr=(n,ref,base)=>({number:n,title:"PR #"+n,head:{ref,sha:sha(n%10)},base:{ref:base,sha:sha(1)},draft:true});
test("separates IDE from Core and walks only the declared open-PR base chain",()=>{
 const input=[pr(4,"ide/first","main"),pr(3,"lu/side","main"),pr(2,"lu/core-tip","lu/base"),pr(1,"lu/base","main")];
 const view=buildSnapshot(input,{sourceBranch:"lu/core-tip"});
 assert.deepEqual(view.coreBaseChain.map(n=>n.number),[2,1]);
 assert.equal(view.summary.corePrs,3);assert.equal(view.summary.idePrs,1);
 assert.deepEqual(view.outsideCoreBaseChain.map(n=>n.number),[3]);
});
test("exact Git-ancestry evidence cannot be reused after source head changed",()=>{
 const input=[pr(1,"lu/core-tip","main"),pr(2,"lu/side","main")];
 const comparisons={repository:"vjmichael/Lu-AI-GUI",coreHeadSha:sha(9),behindByPr:{"2":0}};
 assert.throws(()=>buildSnapshot(input,{sourceBranch:"lu/core-tip",comparisons}),/different exact head/);
 comparisons.coreHeadSha=sha(1);
 const view=buildSnapshot(input,{sourceBranch:"lu/core-tip",comparisons});
 assert.equal(view.summary.outsideAlreadyGitAncestor,1);
 comparisons.behindByPr={};
 assert.throws(()=>buildSnapshot(input,{sourceBranch:"lu/core-tip",comparisons}),/Missing independent/);
});
test("duplicate PR identities, malformed SHA and cycles refuse publication",()=>{
 const tip=pr(1,"lu/core-tip","lu/other");const second=pr(2,"lu/other","lu/core-tip");
 assert.throws(()=>buildSnapshot([tip,second],{sourceBranch:"lu/core-tip"}),/Cycle/);
 assert.throws(()=>buildSnapshot([tip,tip],{sourceBranch:"lu/core-tip"}),/duplicate/);
 assert.throws(()=>buildSnapshot([pr(2,"lu/core-tip","main"),{...tip,head:{ref:"lu/other",sha:"fake"}}],{sourceBranch:"lu/core-tip"}),/Invalid/);
});
