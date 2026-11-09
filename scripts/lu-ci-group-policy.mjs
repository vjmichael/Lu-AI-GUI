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

export function inspectOne(filename, source) {
  const keys = eventKeys(source);
  const problems = [];
  if (!keys.includes("workflow_dispatch")) problems.push("NO_GROUP_DISPATCH");
  if (keys.filter((x) => x === "workflow_dispatch").length !== 1) problems.push("DUPLICATE_DISPATCH");
  if (!AUTOMATIC.has(filename) && keys.some((x) => [
    "push", "pull_request", "pull_request_target", "merge_group", "repository_dispatch"
  ].includes(x))) problems.push("HEAVY_WORKFLOW_AUTOMATIC_FANOUT");
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
