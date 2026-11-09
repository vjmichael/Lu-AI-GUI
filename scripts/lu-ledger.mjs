#!/usr/bin/env node
// Keep ONLY the active-PR metadata block in docs/LU_DEVELOPMENT_LEDGER.md current.
// The manually reviewed evidence, feature acceptance and release history are never rewritten.
import fs from "node:fs/promises";
import path from "node:path";
import {fileURLToPath} from "node:url";

const START = "<!-- LU_OPEN_PRS_START -->";
const END = "<!-- LU_OPEN_PRS_END -->";
const tick = String.fromCharCode(96);

function usage() {
  throw new Error("usage: node scripts/lu-ledger.mjs (--check|--update) [--file PATH] [--fixture PATH] [--graph PATH]");
}
function valueAfter(args, flag, fallback) {
  const i = args.indexOf(flag);
  if (i < 0) return fallback;
  if (!args[i + 1] || args[i + 1].startsWith("--")) usage();
  return args[i + 1];
}
function escaped(text) {
  return String(text ?? "")
    .replace(/[\r\n\t]+/g, " ")
    .replace(/\\/g, "\\\\")
    .replace(/\|/g, "\\|")
    .replace(/\x60/g, "'")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
function validatePr(pr) {
  if (!Number.isSafeInteger(pr?.number) || pr.number <= 0
    || !/^[a-f0-9]{40}$/.test(pr?.head?.sha ?? "")
    || typeof pr?.base?.ref !== "string"
    || typeof pr?.head?.ref !== "string"
    || typeof pr?.title !== "string") {
    throw new Error("GitHub response contains malformed PR metadata");
  }
  return pr;
}
function render(prs, repo) {
  const rows = prs.map(validatePr).sort((a, b) => b.number - a.number)
    .map(pr => [
      "[#" + pr.number + "](https://github.com/" + repo + "/pull/" + pr.number + ")",
      escaped(pr.title),
      tick + escaped(pr.head.ref) + tick,
      "[" + tick + pr.head.sha.slice(0, 12) + tick + "](https://github.com/" + repo + "/commit/" + pr.head.sha + ")",
      tick + escaped(pr.base.ref) + tick,
      pr.draft ? "Draft" : "Open"
    ].join(" | "))
    .map(row => "| " + row + " |");
  return [
    START,
    "_This section is a GitHub metadata snapshot, not proof that an implementation is integrated. Refresh with " + tick + "node scripts/lu-ledger.mjs --update" + tick + ". Open PR count: **" + prs.length + "**._",
    "",
    "| PR | Proposal / scope | Head branch | Exact head | PR base | Review state |",
    "| --- | --- | --- | --- | --- | --- |",
    ...rows,
    END
  ].join("\n");
}
function workstream(ref) {
  if (ref.startsWith("ide/")) return "ide";
  if (ref.startsWith("lu/memory-") || ref.startsWith("lu/ansf-") || ref.startsWith("lu/m2-") || ref.startsWith("lu/m3-") || ref.startsWith("lu/m4-") || ref.startsWith("lu/m5-")) return "memory";
  if (ref.startsWith("lu/agent") || ref.startsWith("lu/e1-")) return "agent-e1";
  if (ref.startsWith("lu/tools-") || ref.startsWith("lu/wave") || ref.startsWith("lu/v1-observe")) return "tools";
  return "core-other";
}
function graph(prs, repo) {
  const heads = new Map(prs.map(pr => [pr.head.ref, pr.number]));
  return {
    schema: 1,
    repository: repo,
    evidence: "open GitHub PR metadata only; no compilation, integration or acceptance implied",
    openCount: prs.length,
    pullRequests: prs.map(validatePr).sort((a, b) => a.number - b.number).map(pr => ({
      number: pr.number,
      title: pr.title,
      draft: Boolean(pr.draft),
      workstream: workstream(pr.head.ref),
      head: {ref: pr.head.ref, sha: pr.head.sha},
      base: {ref: pr.base.ref, sha: pr.base.sha ?? null},
      parentOpenPr: heads.get(pr.base.ref) ?? null,
      baseClassification: pr.base.ref === "main" ? "main" : heads.has(pr.base.ref) ? "open-pr-head" : "other-branch"
    }))
  };
}
async function getGithubPrs(repo) {
  const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;
  const headers = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "lu-dev-ledger/1",
    "X-GitHub-Api-Version": "2022-11-28"
  };
  if (token) headers.Authorization = "Bearer " + token;
  const all = [];
  for (let page = 1; page <= 10; page++) {
    const url = "https://api.github.com/repos/" + repo
      + "/pulls?state=open&per_page=100&page=" + page;
    const response = await fetch(url, {headers, signal: AbortSignal.timeout(20000)});
    if (!response.ok) throw new Error("GitHub PR metadata request failed: HTTP " + response.status);
    const batch = await response.json();
    if (!Array.isArray(batch)) throw new Error("GitHub PR response was not an array");
    all.push(...batch);
    if (batch.length < 100) return all;
  }
  throw new Error("More than 1000 open PRs; refusing incomplete inventory");
}
export async function run(argv = process.argv.slice(2)) {
  const update = argv.includes("--update");
  const check = argv.includes("--check");
  if (update === check) usage();
  const repo = process.env.GITHUB_REPOSITORY || "vjmichael/Lu-AI-GUI";
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error("Invalid repository slug");
  const file = path.resolve(valueAfter(argv, "--file", "docs/LU_DEVELOPMENT_LEDGER.md"));
  const fixture = valueAfter(argv, "--fixture", null);
  const graphFile = valueAfter(argv, "--graph", null);
  const prs = fixture ? JSON.parse(await fs.readFile(fixture, "utf8")) : await getGithubPrs(repo);
  if (!Array.isArray(prs)) throw new Error("PR fixture must be an array");
  if (new Set(prs.map(p => p.number)).size !== prs.length) throw new Error("Duplicate PR numbers");
  const graphText = graphFile ? JSON.stringify(graph(prs, repo), null, 2) + "\n" : null;
  const original = await fs.readFile(file, "utf8");
  const first = original.indexOf(START);
  const last = original.indexOf(END, first + START.length);
  if (first < 0 || last < 0 || original.indexOf(START, first + START.length) !== -1
    || original.indexOf(END, last + END.length) !== -1) {
    throw new Error("Ledger has missing or duplicate generated-region markers");
  }
  const generated = render(prs, repo);
  const next = original.slice(0, first) + generated + original.slice(last + END.length);
  const priorGraph = graphFile ? await fs.readFile(graphFile, "utf8").catch(error => {
    if (error.code === "ENOENT") return null;
    throw error;
  }) : null;
  if (next === original && (!graphFile || priorGraph === graphText)) {
    console.log("Lu ledger PR inventory is current (" + prs.length + " open PRs).");
    return;
  }
  if (check) {
    console.error("Lu ledger PR inventory is out of date; run node scripts/lu-ledger.mjs --update");
    process.exitCode = 1;
    return;
  }
  await fs.writeFile(file, next, "utf8");
  if (graphFile) await fs.writeFile(graphFile, graphText, "utf8");
  console.log("Updated Lu ledger PR metadata (" + prs.length + " open PRs); manually curated status untouched.");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run().catch(error => { console.error(error.message); process.exitCode = 1; });
}
