import test from "node:test";
import assert from "node:assert/strict";
import { inspectOne, inspectDirectory } from "./lu-ci-group-policy.mjs";
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

test("CORE-01E installed gate requires independent PR226 one-use exact-SHA marker", () => {
  const src = fs.readFileSync(".github/workflows/lu-plan07d-installed.yml", "utf8");
  assert.deepEqual(inspectOne("lu-plan07d-installed.yml", src).problems, []);
  for (const unsafe of [
    src.replace("number == 226", "number == 999"),
    src.replace("LU_CORE01E_INSTALLED_SHA={0}", "LU_CORE01E_NATIVE_GATE_SHA={0}"),
    src.replace("!contains(github.event.changes.body.from, format('LU_CORE01E_INSTALLED_SHA", "contains(github.event.changes.body.from, format('LU_CORE01E_INSTALLED_SHA"),
    src.replace("head.ref == 'lu/core01-source-convergence-no-ide-no-kobold-20261112'", "head.ref == 'main'"),
    src.replace(") || (github.event.pull_request.number == 226", ") || (true) || (github.event.pull_request.number == 226"),
    src.replace("REVIEW_MARKER: ${{ github.event.pull_request.number == 226", "REVIEW_MARKER: ${{ github.event.pull_request.number == 999")
  ]) {
    assert.ok(inspectOne("lu-plan07d-installed.yml", unsafe).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
  }
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
