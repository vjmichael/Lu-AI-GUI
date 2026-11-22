#!/usr/bin/env node
/** Group-scoped CI policy: heavy Lu Core workflows do NOT run on every PR or source-lock update. */
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

export const AUTOMATIC = new Set(["lu-repo01-full-source.yml"]);

// Only the exact canonical metadata-only workflow already on main may retain
// its existing scheduled/push maintenance. This is a fixed Git blob identity
// exception, NEVER a filename-wide exception to task-group fanout safety.
const CANONICAL_MAIN_LEDGER_GIT_BLOB = "35c5de7ab6bb4e4b50db76f785064809f342396b";
export function isCanonicalMainLedgerWorkflow(filename, source, keys) {
  if (filename !== "lu-development-ledger.yml" ||
      keys.join(",") !== "push,workflow_dispatch,schedule") return false;
  const contents = Buffer.from(source, "utf8");
  const digest = createHash("sha1").update(`blob ${contents.length}\0`).update(contents).digest("hex");
  return digest === CANONICAL_MAIN_LEDGER_GIT_BLOB;
}


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
  const win = /REVIEW_MARKER: \$\{\{ github\.event\.pull_request\.number == 226 && 'LU_CORE01E_INSTALLED_SHA' \|\| 'LU_PLAN07D_INSTALLED_SHA' \}\}/g;
  if ([...source.matchAll(win)].length !== 3) return false;
  if (!source.includes("re.escape(os.environ['REVIEW_MARKER'])") ||
      !source.includes('re.escape(os.environ["REVIEW_MARKER"])') ||
      !source.includes("[regex]::Escape($env:REVIEW_MARKER)")) return false;
  return true;
}

/** Only the two exact CP-10 release-qualification branches may trigger heavy CI on a finalized source lock. */
export function isScopedLockedReleaseWorkflow(filename, source, keys) {
  const scopes={
    'lu-core-c6-hosted-dual-nsis.yml':{branch:'lu/cp10-consolidated-ux-20261120',group:'lu-c6-dual-nsis',browser:false},
    'lu-core-c62b-reviewed-browser-nsis.yml':{branch:'lu/cp10-c62b-reviewed-browser-20261121',group:'lu-c62b-reviewed-browser',browser:true}
  };
  const scope=scopes[filename];
  if(!scope||keys.join(',')!=='push,workflow_dispatch,workflow_call')return false;
  const evt=source.split('\non:\n')[1]?.split('\npermissions:\n')[0]||'';
  const expected='  push:\n    branches:\n      - '+scope.branch+'\n    paths:\n      - source-tree.lock.json\n  workflow_dispatch:\n  workflow_call:\n';
  if(!evt.includes(expected))return false;
  const other=evt.replace(expected,'').replace(/^\s*#.*$/gm,'').trim();
  if(other)return false;
  const dollar=String.fromCharCode(36);
  if(!source.includes('group: '+scope.group+'-'+dollar+'{{ github.ref }}')||
     !source.includes('cancel-in-progress: false')||
     !source.includes('ref: '+dollar+'{{ github.sha }}')||
     !source.includes('node scripts/verify-source-tree.mjs')||
     !source.includes('permissions:\n  contents: read')||
     source.includes('continue-on-error: true'))return false;
  if(scope.browser)return ["LU_REQUIRE_BROWSER: '1'",'run-id: 43976953361',
    '859ae6365aabaad2ada9362bb556243ede414f3de65fa3667ea0c3e5da38af0b',
    'promote-reviewed-browser-candidate.mjs','PASS_INSTALLED_UNIFIED_TABS',
    'scripts/m3-windows-nsis-cycle.ps1'].every(part=>source.includes(part));
  return ["LU_REQUIRE_BROWSER: '0'",'PASS_INSTALLED_UNIFIED_TABS',
    'scripts/m3-windows-nsis-cycle.ps1'].every(part=>source.includes(part));
}
export function inspectOne(filename, source) {
  const keys = eventKeys(source);
  const problems = [];
  const narrowlyAdmitted = isGuardedPlan07D(filename, source, keys);
  const canonicalMainLedger = isCanonicalMainLedgerWorkflow(filename, source, keys);
  const scopedRelease = isScopedLockedReleaseWorkflow(filename, source, keys);
  if (!keys.includes("workflow_dispatch") && !narrowlyAdmitted) problems.push("NO_GROUP_DISPATCH");
  if (keys.filter((x) => x === "workflow_dispatch").length !== (narrowlyAdmitted ? 0 : 1)) problems.push("DUPLICATE_DISPATCH");
  if (!AUTOMATIC.has(filename) && keys.some((x) => [
    "push", "pull_request", "pull_request_target", "merge_group", "repository_dispatch"
  ].includes(x)) && !narrowlyAdmitted && !canonicalMainLedger && !scopedRelease) problems.push("HEAVY_WORKFLOW_AUTOMATIC_FANOUT");
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
