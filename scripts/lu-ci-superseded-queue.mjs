#!/usr/bin/env node
/** Safe GitHub Actions queue cleanup: old-SHA automatic QUEUED runs in ONE explicit Lu branch. */
import { fileURLToPath } from "node:url";
import path from "node:path";

export function selectSuperseded(runs, branch, headSha, limit = 25) {
  if (!/^lu\/[a-zA-Z0-9._/-]+$/.test(branch) || !/^[0-9a-f]{40}$/.test(headSha))
    throw Error("Require a specific Lu branch and an exact 40-character head SHA");
  const selected = runs.filter((r) =>
    r && r.head_branch === branch &&
    r.status === "queued" &&
    (r.event === "push" || r.event === "pull_request") &&
    /^[0-9a-f]{40}$/.test(r.head_sha || "") &&
    r.head_sha !== headSha &&
    Number.isSafeInteger(r.id) && r.id > 0
  );
  const unique = new Map();
  for (const run of selected) unique.set(run.id, run);
  return [...unique.values()].sort((a, b) => a.id - b.id).slice(0, limit);
}

export function argsFrom(argv) {
  const a = { repo: "vjmichael/Lu-AI-GUI", branch: null, apply: false,
    confirmHead: null, limit: 25 };
  for (let i = 0; i < argv.length; i++) {
    const value = argv[i];
    if (value === "--repo" || value === "--branch" ||
        value === "--confirm-head" || value === "--limit") {
      if (!argv[i + 1]) throw Error("Missing value for " + value);
      const v = argv[++i];
      if (value === "--repo") a.repo = v;
      if (value === "--branch") a.branch = v;
      if (value === "--confirm-head") a.confirmHead = v;
      if (value === "--limit") a.limit = Number(v);
    } else if (value === "--apply") a.apply = true;
    else throw Error("Unsupported argument " + value);
  }
  if (a.repo !== "vjmichael/Lu-AI-GUI") throw Error("Only this Lu repository is supported");
  if (!a.branch || !/^lu\/[A-Za-z0-9._/-]+$/.test(a.branch) ||
      a.branch.includes("..") || a.branch.includes("//"))
    throw Error("Explicit safe lu/ branch required");
  if (!Number.isInteger(a.limit) || a.limit < 1 || a.limit > 50)
    throw Error("Limit must be 1..50");
  if (a.apply && !/^[0-9a-f]{40}$/.test(a.confirmHead || ""))
    throw Error("--apply requires --confirm-head exact SHA");
  return a;
}

export async function execute(opts, fetcher) {
  const base = "https://api.github.com/repos/" + opts.repo;
  const api = async (url, init = {}) => {
    const response = await fetcher(url, init);
    if (!response.ok) throw Error("GitHub status " + response.status + " for " + url.split("?")[0]);
    return response.status === 204 ? {} : await response.json().catch(() => ({}));
  };
  const current = await api(base + "/branches/" + encodeURIComponent(opts.branch));
  const head = current?.commit?.sha;
  if (!/^[0-9a-f]{40}$/.test(head || "")) throw Error("Cannot attest current branch head");
  if (opts.apply && opts.confirmHead !== head) throw Error("Current head changed; refuse cancellation");
  const runs = [];
  for (let page = 1; page <= 10; page++) {
    const query = "?branch=" + encodeURIComponent(opts.branch) +
      "&status=queued&per_page=100&page=" + page;
    const list = await api(base + "/actions/runs" + query);
    const pageRuns = list.workflow_runs || [];
    if (!Array.isArray(pageRuns)) throw Error("Malformed run list");
    runs.push(...pageRuns);
    if (pageRuns.length < 100) break;
  }
  const selected = selectSuperseded(runs, opts.branch, head, opts.limit);
  const report = { schema: "lu-superseded-queue-plan-v1", branch: opts.branch,
    currentHeadSha: head, mode: opts.apply ? "APPLY" : "DRY_RUN",
    selected: selected.map((r) => ({ id: r.id, oldSha: r.head_sha, workflow: r.name })),
    cancellationRequests: [] };
  if (opts.apply) {
    for (const r of selected) {
      const freshBranch = await api(base + "/branches/" + encodeURIComponent(opts.branch));
      if (freshBranch?.commit?.sha !== head) throw Error("Branch changed while processing; stop");
      const fresh = await api(base + "/actions/runs/" + r.id);
      if (fresh.status !== "queued" || fresh.head_sha !== r.head_sha ||
          fresh.head_branch !== opts.branch ||
          !["push", "pull_request"].includes(fresh.event))
        throw Error("Queued run changed identity/status; stop");
      const response = await fetcher(base + "/actions/runs/" + r.id + "/cancel", { method: "POST" });
      report.cancellationRequests.push({ id: r.id, httpStatus: response.status,
        accepted: response.status === 202 });
      if (response.status !== 202) throw Error("Cancellation rejected for run " + r.id);
    }
  }
  return report;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const opts = argsFrom(process.argv.slice(2));
    const token = process.env.GITHUB_TOKEN;
    if (!token) throw Error("GITHUB_TOKEN with Actions read/write is required");
    const report = await execute(opts, (url, init = {}) => fetch(url, {
      ...init,
      headers: { Accept: "application/vnd.github+json",
        Authorization: "Bearer " + token, "X-GitHub-Api-Version": "2022-11-28" }
    }));
    console.log(JSON.stringify(report, null, 2));
  } catch (error) {
    console.error("CI queue cleanup refused: " + String(error.message || error));
    process.exitCode = 1;
  }
}
