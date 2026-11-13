#!/usr/bin/env node
/** Group-scoped CI policy: heavy Lu Core workflows do NOT run on every PR or source-lock update. */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const AUTOMATIC = new Set(["lu-repo01-full-source.yml"]);

export function eventKeys(source) {
  const matches = [...source.matchAll(/^on:\s*$/gm)];
  if (matches.length !== 1) throw Error("Expected exactly one root-level on: block");
  const from = matches[0].index + matches[0][0].length;
  const rest = source.slice(from);
  const next = rest.search(/^[A-Za-z][A-Za-z0-9_-]*:\s*(?:#.*)?$/m);
  if (next < 0) throw Error("No root-level event terminator");
  const block = rest.slice(0, next);
  return [...block.matchAll(/^  ([a-z][a-z0-9_-]*):/gm)].map((m) => m[1]);
}


/**
 * PLAN-07D predates the opt-in workflow_dispatch convention. Keep its
 * operator-reviewed PR #219 transition alive, but never allow its restricted
 * [edited] trigger to be generalized to other PRs, branches, or SHA markers.
 * This exception is NOT permission to run arbitrary task-group CI per push.
 */
export function isGuardedPlan07D(filename, source, keys) {
  if (filename !== "lu-plan07d-installed.yml" ||
      keys.length !== 1 || keys[0] !== "pull_request" ||
      !/^    types: \[edited\]\s*$/m.test(source)) return false;
  const body = source.split(/^jobs:\s*$/m)[1];
  if (!body) return false;
  const jobs = body.split(/^  ([a-z0-9_-]+):\s*$/gm).slice(1);
  if (jobs.length !== 6) return false; // exactly 3 job-name/body pairs
  const expected = new Set([
    "hosted-windows-nsis",
    "hosted-debian-installed-human",
    "hosted-genuine-gguf-original"
  ]);
  for (let i = 0; i < jobs.length; i += 2) {
    if (!expected.delete(jobs[i])) return false;
    const condition = jobs[i+1].match(/^    if: (.+)$/m)?.[1] || "";
    if (!condition.includes("github.event.pull_request.number == 219") ||
        !condition.includes("github.event.pull_request.draft") ||
        !condition.includes("github.event.pull_request.head.ref == 'lu/plan07-evidence-driven-task-updates-20261110'") ||
        !condition.includes("contains(github.event.pull_request.body, format('LU_PLAN07D_INSTALLED_SHA={0}', github.event.pull_request.head.sha))") ||
        !condition.includes("!contains(github.event.changes.body.from, format('LU_PLAN07D_INSTALLED_SHA={0}', github.event.pull_request.head.sha))")) return false;
  }
  return expected.size === 0;
}

/**
 * Main's authoritative metadata ledger is deliberately scheduled and main-push
 * verified. Its jobs cannot run the heavyweight Core matrix and refresh only
 * docs/LU_DEVELOPMENT_LEDGER.md from checked-out main. This narrowly scoped
 * exception does not admit other scheduled/push workflows.
 */
export function isGuardedMainLedger(filename, source, keys) {
  if (filename !== "lu-development-ledger.yml" ||
      keys.join(",") !== "push,workflow_dispatch,schedule") return false;
  if (!/^on:\s*\n  push:\s*\n    branches: \[main\]\s*\n    paths:\s*\n      - '\.github\/workflows\/lu-development-ledger\.yml'\s*\n      - 'scripts\/lu-ledger\.mjs'\s*\n      - 'scripts\/lu-ledger\.test\.mjs'\s*\n  workflow_dispatch:\s*\n  schedule:\s*\n    - cron: '17 13 \* \* \*'/m.test(source)) return false;
  if ([...source.matchAll(/^  ([a-z][a-z0-9-]*):\s*$/gm)]
      .map(m => m[1]).filter(x => x === "refresh" || x === "verify-on-source-change")
      .join(",") !== "verify-on-source-change,refresh") return false;
  if (!/verify-on-source-change:\s*\n    if: github\.event_name == 'push'/.test(source) ||
      !/refresh:\s*\n    if: github\.repository == 'vjmichael\/Lu-AI-GUI' && github\.event_name != 'push'/.test(source) ||
      !/ref: main/.test(source) ||
      !/git add -- docs\/LU_DEVELOPMENT_LEDGER\.md/.test(source) ||
      !/git push origin HEAD:main/.test(source)) return false;
  const jobs = source.slice(source.indexOf("\njobs:\n"));
  return !/^  (?!verify-on-source-change:|refresh:)[a-z][a-z0-9_-]*:\s*$/m.test(jobs);
}

export function inspectOne(filename, source) {
  const keys = eventKeys(source);
  const problems = [];
  const narrowlyAdmitted = isGuardedPlan07D(filename, source, keys) || isGuardedMainLedger(filename, source, keys);
  if (!keys.includes("workflow_dispatch") && !narrowlyAdmitted) problems.push("NO_GROUP_DISPATCH");
  if (keys.filter((x) => x === "workflow_dispatch").length !== (narrowlyAdmitted ? 0 : 1)) problems.push("DUPLICATE_DISPATCH");
  if (!AUTOMATIC.has(filename) && keys.some((x) => [
    "push", "pull_request", "pull_request_target", "merge_group", "repository_dispatch"
  ].includes(x)) && !narrowlyAdmitted) problems.push("HEAVY_WORKFLOW_AUTOMATIC_FANOUT");
  if (AUTOMATIC.has(filename) && !keys.includes("pull_request"))
    problems.push("BASELINE_PR_CHECK_MISSING");
  if (!/\njobs:\s*\n/.test(source)) problems.push("NO_JOBS");
  return { filename, events: keys, problems };
}

export function inspectDirectory(directory) {
  const filenames = fs.readdirSync(directory).filter((x) => /^lu-.*\.ya?ml$/.test(x)).sort();
  const entries = filenames.map((filename) => inspectOne(filename, fs.readFileSync(path.join(directory, filename), "utf8")));
  const failures = entries.filter((x) => x.problems.length);
  const automatic = entries.filter((x) => AUTOMATIC.has(x.filename));
  const deferred = entries.filter((x) => !AUTOMATIC.has(x.filename));
  return { schema: "lu-task-group-ci-policy-v1", total: entries.length,
    automatic: automatic.map((x) => x.filename), taskGroupOnly: deferred.length,
    failures, entries };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const directory = path.resolve(process.argv[2] || ".github/workflows");
  const report = inspectDirectory(directory);
  console.log(JSON.stringify({ schema: report.schema, total: report.total,
    automatic: report.automatic, taskGroupOnly: report.taskGroupOnly,
    failures: report.failures }, null, 2));
  if (report.failures.length || report.total < 60) process.exitCode = 1;
}
