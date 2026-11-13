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
      !/^    types: \\[edited\\]\\s*$/m.test(source)) return false;
  const body = source.split(/^jobs:\\s*$/m)[1];
  if (!body) return false;
  const jobs = body.split(/^  ([a-z0-9_-]+):\\s*$/gm).slice(1);
  if (jobs.length !== 6) return false; // exactly three admission-scoped jobs
  const expected = new Set([
    "hosted-windows-nsis",
    "hosted-debian-installed-human",
    "hosted-genuine-gguf-original"
  ]);
  const legacy = "github.event.pull_request.number == 219 && github.event.pull_request.draft && github.event.pull_request.head.ref == 'lu/plan07-evidence-driven-task-updates-20261110' && contains(github.event.pull_request.body, format('LU_PLAN07D_INSTALLED_SHA={0}', github.event.pull_request.head.sha)) && !contains(github.event.changes.body.from, format('LU_PLAN07D_INSTALLED_SHA={0}', github.event.pull_request.head.sha))";
  const core = "github.event.pull_request.number == 226 && github.event.pull_request.draft && github.event.pull_request.head.ref == 'lu/core01-source-convergence-no-ide-no-kobold-20261112' && contains(github.event.pull_request.body, format('LU_CORE01E_INSTALLED_SHA={0}', github.event.pull_request.head.sha)) && !contains(github.event.changes.body.from, format('LU_CORE01E_INSTALLED_SHA={0}', github.event.pull_request.head.sha))";
  const exactAdmission = '${{ (' + legacy + ') || (' + core + ') }}';
  for (let i = 0; i < jobs.length; i += 2) {
    if (!expected.delete(jobs[i])) return false;
    const condition = jobs[i+1].match(/^    if: (.+)$/m)?.[1] || "";
    // Prevent permissive OR clauses, wrong PR, branch, or repeated approvals.
    if (condition !== exactAdmission) return false;
  }
  if (expected.size !== 0) return false;
  // Each platform checks the marker once more INSIDE the original process,
  // including independent source-tree verification at the exact checked SHA.
  const win = /REVIEW_MARKER: \\$\\{\\{ github\\.event\\.pull_request\\.number == 226 && 'LU_CORE01E_INSTALLED_SHA' \\|\\| 'LU_PLAN07D_INSTALLED_SHA' \\}\\}/g;
  if ([...source.matchAll(win)].length !== 3) return false;
  if (!source.includes("re.escape(os.environ['REVIEW_MARKER'])") ||
      !source.includes('re.escape(os.environ["REVIEW_MARKER"])') ||
      !source.includes("[regex]::Escape($env:REVIEW_MARKER)")) return false;
  return true;
}

export function inspectOne(filename, source) {
  const keys = eventKeys(source);
  const problems = [];
  const narrowlyAdmitted = isGuardedPlan07D(filename, source, keys);
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
