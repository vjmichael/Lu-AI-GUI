import test from "node:test";
import assert from "node:assert/strict";
import { inspectOne, inspectDirectory, isGuardedMainLedger } from "./lu-ci-group-policy.mjs";
import fs from "node:fs";

const manual = "name: One\non:\n  workflow_dispatch:\npermissions:\n  contents: read\njobs:\n  test:\n    runs-on: ubuntu-latest\n";
const auto = "name: One\non:\n  push:\n  pull_request:\n  workflow_dispatch:\npermissions:\n  contents: read\njobs:\n  test:\n    runs-on: ubuntu-latest\n";

test("task group manual workflow cannot start itself per commit or PR", () => {
  assert.deepEqual(inspectOne("lu-m4-verified-memory.yml", manual).problems, []);
  assert.ok(inspectOne("lu-m4-verified-memory.yml", auto).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
});
test("keep only baseline source and desktop checks automatic", () => {
  assert.deepEqual(inspectOne("lu-repo01-full-source.yml", auto).problems, []);
  assert.deepEqual(inspectOne("lu-ci.yml", manual).problems, []);
});
test("broken events, duplicate dispatch or missing jobs fail policy", () => {
  assert.throws(() => inspectOne("lu-any.yml", "name: Broken\njobs:\n"));
  assert.ok(inspectOne("lu-any.yml", manual.replace("  workflow_dispatch:\n", "  workflow_dispatch:\n  workflow_dispatch:\n")).problems.includes("DUPLICATE_DISPATCH"));
  assert.ok(inspectOne("lu-any.yml", manual.replace("jobs:\n", "no-jobs:\n")).problems.includes("NO_JOBS"));
});
test("actual checked-in Lu workflow suite uses task-group batching", () => {
  const report = inspectDirectory(".github/workflows");
  assert.ok(report.total >= 68);
  assert.equal(report.automatic.length, 1);
  assert.equal(report.taskGroupOnly, report.total - 1);
  assert.deepEqual(report.failures, []);
});

test("PLAN-07D exception requires all three exact PR219/branch/SHA guards", () => {
  const src = fs.readFileSync(".github/workflows/lu-plan07d-installed.yml", "utf8");
  assert.deepEqual(inspectOne("lu-plan07d-installed.yml", src).problems, []);
  for (const unsafe of [
    src.replace("types: [edited]", "types: [opened, edited]"),
    src.replace("number == 219", "number == 999"),
    src.replace("head.ref == 'lu/plan07-evidence-driven-task-updates-20261110'", "head.ref == 'main'"),
    src.replace("!contains(github.event.changes.body.from", "contains(github.event.changes.body.from"),
    src.replace("hosted-genuine-gguf-original:", "generic-unchecked-job:")
  ]) {
    assert.ok(inspectOne("lu-plan07d-installed.yml", unsafe).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
  }
});

test("existing main-only development ledger is narrowly admitted, never a general heavy CI exception", () => {
  const src = fs.readFileSync(".github/workflows/lu-development-ledger.yml", "utf8");
  assert.equal(isGuardedMainLedger("lu-development-ledger.yml", src, ["push","workflow_dispatch","schedule"]), true);
  assert.deepEqual(inspectOne("lu-development-ledger.yml", src).problems, []);
  for (const unsafe of [
    src.replace("branches: [main]", "branches: [main, dev]"),
    src.replace("    - cron: '17 13 * * *'", "    - cron: '* * * * *'"),
    src.replace("if: github.event_name == 'push'", "if: always()"),
    src.replace("ref: main", "ref: ${{ github.sha }}"),
    src.replace("git add -- docs/LU_DEVELOPMENT_LEDGER.md", "git add -- ."),
    src.replace("git push origin HEAD:main", "git push --force origin HEAD:main"),
    src.replace("\n  refresh:\n", "\n  rogue-heavy:\n    runs-on: ubuntu-latest\n  refresh:\n")
  ]) {
    assert.ok(inspectOne("lu-development-ledger.yml", unsafe).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
  }
  assert.ok(inspectOne("lu-unknown-heavy.yml", src).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
});
