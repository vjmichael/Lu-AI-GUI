import test from "node:test";
import assert from "node:assert/strict";
import { inspectOne, inspectDirectory } from "./lu-ci-group-policy.mjs";

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
