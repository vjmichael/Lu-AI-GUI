#!/usr/bin/env node
// Refreshable and deterministic Core/IDE open-PR topology. Does not merge, authorize or execute projects.
import fs from "node:fs/promises";
import {pathToFileURL} from "node:url";
import path from "node:path";

export const buildSnapshot = function buildSnapshot(raw,options={}){
 const sourceBranch=options.sourceBranch||"lu/memory-loop03e-crash-publication-20261109";
 const repository=options.repository||"vjmichael/Lu-AI-GUI";
 if(!Array.isArray(raw)||!raw.length||raw.length>1000)throw Error("Incomplete or excessive open-PR inventory");
 const seenIds=new Set(),seenRefs=new Set();
 const nodes=raw.map(p=>{
   if(!Number.isSafeInteger(p.number)||p.number<=0||seenIds.has(p.number)||
       typeof p.title!=="string"||p.title.length>512||
       !/^[a-f0-9]{40}$/.test(p.head?.sha||"")||
       typeof p.head?.ref!=="string"||p.head.ref.length>240||
       typeof p.base?.ref!=="string"||p.base.ref.length>240||
       seenRefs.has(p.head.ref))throw Error("Invalid or duplicate PR metadata");
   seenIds.add(p.number);seenRefs.add(p.head.ref);
   return {number:p.number,title:p.title,headBranch:p.head.ref,headSha:p.head.sha,
     baseBranch:p.base.ref,baseSha:p.base.sha||null,draft:!!p.draft,
     workstream:p.head.ref.startsWith("ide/")?"ide":"core"};
 }).sort((a,b)=>a.number-b.number);
 const lookup=new Map(nodes.map(x=>[x.headBranch,x]));
 const tip=lookup.get(sourceBranch);
 if(!tip||tip.workstream!=="core")throw Error("Core convergence tip is absent");
 const chain=[],visited=new Set();let current=tip;
 while(current){
   if(visited.has(current.headBranch))throw Error("Cycle in open PR base refs");
   visited.add(current.headBranch);chain.push(current);
   current=lookup.get(current.baseBranch);
 }
 const core=nodes.filter(n=>n.workstream==="core"),ide=nodes.filter(n=>n.workstream==="ide");
 const side=core.filter(n=>!visited.has(n.headBranch));
 const compared=options.comparisons||null;
 if(compared){
   if(compared.coreHeadSha!==tip.headSha||compared.repository!==repository)throw Error("Ancestry witness belongs to a different exact head");
   if(Object.keys(compared.behindByPr).length!==side.length)throw Error("Missing independent side-branch comparisons");
 }
 const external=side.map(n=>{
   let v=compared?.behindByPr[String(n.number)];
   if(compared && (!Number.isSafeInteger(v)||v<0))throw Error("Missing/invalid Git comparison for PR #"+n.number);
   return {number:n.number,headBranch:n.headBranch,headSha:n.headSha,
     baseBranch:n.baseBranch,gitRelation:v===undefined?"unverified":v===0?"already_ancestor":"diverged",
     commitsNotInCoreHead:v??null};
 }).sort((a,b)=>a.number-b.number);
 return {
   schema:"lu-core-pr-convergence-v1",repository,
   evidence:"Live open PR metadata and recorded Git comparisons only; NOT evidence of merge, compilation, authority or installed release",
   observedDate:options.observedDate||"2026-11-09",
   targetCoreBranch:sourceBranch,targetCoreHeadSha:tip.headSha,
   summary:{
     openPrs:nodes.length,corePrs:core.length,idePrs:ide.length,
     coreOpenBaseChain:chain.length,coreOutsideBaseChain:external.length,
     outsideAlreadyGitAncestor:external.filter(n=>n.gitRelation==="already_ancestor").length,
     outsideDiverged:external.filter(n=>n.gitRelation==="diverged").length,
     outsideUnverified:external.filter(n=>n.gitRelation==="unverified").length
   },
   coreBaseChain:chain.map(n=>({number:n.number,headBranch:n.headBranch,headSha:n.headSha,baseBranch:n.baseBranch})),
   coreChainTailBase:chain.at(-1).baseBranch,
   outsideCoreBaseChain:external,
   pullRequests:nodes
 };
};

async function main(){
  const args=process.argv.slice(2);
  const option=name=>{const k=args.indexOf(name);return k>=0?args[k+1]:null};
  const updating=args.includes("--update"),checking=args.includes("--check");
  if(updating===checking)throw Error("Choose exactly one of --update or --check");
  const output=option("--output")||"docs/LU_CORE_OPEN_PR_STACK_20261109.json";
  const fixture=option("--fixture"),relationFile=option("--relations");
  let prs=[];
  if(fixture){
    const read=JSON.parse(await fs.readFile(fixture,"utf8"));
    prs=Array.isArray(read)?read:read.pullRequests?.map(p=>({
      number:p.number,title:p.title,draft:p.draft,
      head:{ref:p.headBranch,sha:p.headSha},
      base:{ref:p.baseBranch,sha:p.baseSha}
    }));
  }else{
    const token=process.env.GITHUB_TOKEN||process.env.GH_TOKEN;
    for(let page=1;page<=20;page++){
      const headers={"Accept":"application/vnd.github+json","User-Agent":"Lu-Core-Stack-Graph"};
      if(token)headers.Authorization="Bearer "+token;
      const response=await fetch("https://api.github.com/repos/vjmichael/Lu-AI-GUI/pulls?state=open&per_page=100&page="+page,{
        headers,signal:AbortSignal.timeout(20000)
      });
      if(!response.ok)throw Error("GitHub PR API failed "+response.status);
      const group=await response.json();
      if(!Array.isArray(group))throw Error("Malformed GitHub PR page");
      prs.push(...group);
      if(group.length<100)break;
      if(page===20)throw Error("Pagination budget reached; inventory cannot be called complete");
    }
  }
  const comparisons=relationFile?JSON.parse(await fs.readFile(relationFile,"utf8")):null;
  const result=buildSnapshot(prs,{comparisons,observedDate:option("--date")||"2026-11-09"});
  const encoded=JSON.stringify(result,null,2)+"\n";
  if(checking){
    const previous=await fs.readFile(output,"utf8");
    if(previous!==encoded)throw Error("PR stack snapshot stale or source head changed; refresh and review");
    console.log("CORE_STACK_GRAPH_MATCH total="+result.summary.openPrs+" core="+result.summary.corePrs+" ide="+result.summary.idePrs);
  }else{
    await fs.writeFile(output,encoded,"utf8");
    console.log("CORE_STACK_GRAPH_UPDATED total="+result.summary.openPrs+" core="+result.summary.corePrs+" ide="+result.summary.idePrs);
  }
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  main().catch(e=>{console.error("CORE_STACK_GRAPH_REJECTED: "+e.message);process.exitCode=1});
}
