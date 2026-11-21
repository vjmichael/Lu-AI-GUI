import test from "node:test";
import assert from "node:assert/strict";
import { inspectOne, inspectDirectory, isCanonicalMainLedgerWorkflow } from "./lu-ci-group-policy.mjs";
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
test("canonical main ledger refresh is admitted ONLY at the exact reviewed main Git blob", () => {
  const filename = "lu-development-ledger.yml";
  const exact = fs.readFileSync(".github/workflows/" + filename, "utf8");
  assert.deepEqual(inspectOne(filename, exact).problems, []);
  assert.equal(isCanonicalMainLedgerWorkflow(filename, exact,
    ["push", "workflow_dispatch", "schedule"]), true);
  // Preserving main is not permission for an arbitrary new write-capable
  // scheduled/push workflow. Every mutation of this verified blob fails.
  for (const altered of [
    exact.replace("contents: write", "contents: read"),
    exact.replace("node scripts/lu-ledger.mjs --update",
      "node scripts/lu-ledger.mjs --update && echo changed"),
    exact + "\n# unreviewed source drift\n"
  ]) {
    assert.ok(inspectOne(filename, altered).problems.includes(
      "HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
  }
  assert.ok(inspectOne("lu-unauthorized-ledger.yml", exact).problems.includes(
    "HEAVY_WORKFLOW_AUTOMATIC_FANOUT"));
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

test("CORE-MERGE-C4 remains manual/reusable and is admitted only by original exact PR-head marker", () => {
  const source = fs.readFileSync(".github/workflows/lu-core-merge-c4-grouped.yml", "utf8");
  const primary = fs.readFileSync(".github/workflows/lu-repo01-full-source.yml", "utf8");
  assert.deepEqual(inspectOne("lu-core-merge-c4-grouped.yml", source).problems, []);
  assert.match(source, /^on:\n  workflow_dispatch:\n  workflow_call:/m);
  assert.doesNotMatch(source, /^  (?:push|pull_request|pull_request_target):/m);
  const gate = primary.slice(primary.lastIndexOf("\n  core-merge-c4-one-sha:"));
  assert.match(gate, /needs: verify-source/);
  assert.match(gate, /number == 235/);
  assert.match(gate, /head\.ref == 'lu\/p0-next-typed-criterion-review-20261113'/);
  assert.match(gate, /github\.event\.action == 'edited'/);
  assert.match(gate, /LU_CORE_C4_GROUP_SHA=\{0\}/);
  assert.match(gate, /!contains\(github\.event\.changes\.body\.from/);
  assert.match(gate, /uses: \.\/\.github\/workflows\/lu-core-merge-c4-grouped\.yml/);
});

test("original AGENT-01L positive proof is routed only to configured native Linux and a skipped proof fails full C4", () => {
  const grouped=fs.readFileSync(".github/workflows/lu-core-merge-c4-grouped.yml","utf8");
  const standalone=fs.readFileSync(".github/workflows/lu-agent01l-linux-cgroup-v2.yml","utf8");
  const original=fs.readFileSync("benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py","utf8");
  const g=grouped.split("  linux-real-cgroup:")[1]?.split("\n  linux-original-root-owned-git:")[0];
  const s=standalone.split("  linux-kernel-crash-and-escaped-descendant:")[1];
  const final=grouped.split("  exact-sha-consolidated-reconciliation:")[1];
  assert.ok(g && s && final,"original separated kernel acceptance jobs must exist");
  for(const block of [g,s]){
    assert.match(block,/runs-on: \[self-hosted, linux, x64, lu-cgroup-v2-delegated\]/);
    assert.doesNotMatch(block,/runs-on: ubuntu-24\.04|continue-on-error: true/);
    assert.match(block,/agent01l_cgroup_escape_crash_acceptance\.py/);
    assert.match(block,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
  }
  assert.match(g,/vars\.LU_CGROUP_POSITIVE_RUNNER_READY == 'true'/);
  assert.match(final,/- linux-real-cgroup/);
  assert.match(final,/LINUX: \$\{\{ needs\.linux-real-cgroup\.result \}\}/);
  assert.match(final,/test "\$LINUX" = success/);
  assert.match(final,/LINUX_DELEGATED_CGROUP_POSITIVE_OPEN=1/);
  assert.match(original,/AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1/);
  assert.match(original,/return 78/);
  assert.match(original,/os\.kill\(host\.pid,signal\.SIGKILL\)/);
  assert.match(original,/os\.pidfd_open\(escaped_pid\)/);
});

test("native SEC06C fixture adapter in exact-host gate cannot downgrade positive Linux result",()=>{
  const native=fs.readFileSync(".github/workflows/lu-core-merge-c4-grouped.yml","utf8");
  const original=fs.readFileSync(".github/workflows/lu-agent01l-linux-cgroup-v2.yml","utf8");
  for(const src of [native,original]){
    assert.match(src,/lu-sec06c-agent01l-fixture-launcher\.py/);
    assert.match(src,/lu-sec06c-agent01l-fixture-manager\.py/);
    assert.match(src,/LU_AGENT01L_REQUIRE_OS_OWNED_FIXTURE: '1'/);
    assert.match(src,/sudo -n -l \/usr\/libexec\/lu\/lu-sec06c-launch-agent01l-fixture/);
    assert.match(src,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
    assert.doesNotMatch(src,/continue-on-error: true/);
  }
  assert.match(native,/test "\$LINUX" = success/);
  assert.match(native,/runs-on: \[self-hosted, linux, x64, lu-cgroup-v2-delegated\]/);
});

test("AGENT-01L original native workflow asserts all current source cases before physical test",()=>{
  const workflow=fs.readFileSync(".github/workflows/lu-agent01l-linux-cgroup-v2.yml","utf8");
  const modern=fs.readFileSync("lu-desktop/tests/agent01l-linux-cgroup-v2.test.mjs","utf8");
  const legacy=fs.readFileSync("lu-desktop/tests/agent01l-os-custody.test.mjs","utf8");
  const casesModern=(modern.match(/^\s*test\(/gm)||[]).length;
  const casesLegacy=(legacy.match(/^\s*test\(/gm)||[]).length;
  assert.ok(casesModern>=14 && casesLegacy>=10,"current mandatory native source contracts");
  assert.match(workflow,new RegExp("# pass "+casesModern+"(?=' agent01l-cgroup-evidence/linux-cgroup-contract\\.log)"));
  assert.match(workflow,new RegExp("# pass "+casesLegacy+"(?=' agent01l-cgroup-evidence/agent01l-os-source\\.log)"));
  assert.doesNotMatch(workflow,/continue-on-error: true/);
  assert.match(workflow,/LU_AGENT01L_REQUIRE_OS_OWNED_FIXTURE: '1'/);
  assert.match(workflow,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
  assert.match(workflow,/agent01l_linux_real_git_cgroup_custody_after_restart/);
});

test("hosted-positive never softens original SEC-06C and AGENT-01L kernel tests",()=>{
  const src=fs.readFileSync(".github/workflows/lu-repo01-full-source.yml","utf8");
  const marker="LU_AGENT01L_HOSTED_CGROUP_SHA={0}";
  assert.match(src,/  linux-agent01l-hosted-positive:/);
  assert.match(src,/runs-on: ubuntu-24\.04/);
  assert.match(src,/github\.event\.pull_request\.number == 235/);
  assert.match(src,/github\.event\.action == 'edited'/);
  assert.ok(src.split(marker).length >= 3);
  assert.match(src,/install-lu-agent01l-native-fixture\.sh/);
  assert.match(src,/LU_AGENT01L_REQUIRE_OS_OWNED_FIXTURE: '1'/);
  assert.match(src,/benchmarks\/e1\/agent01l_cgroup_escape_crash_acceptance\.py/);
  assert.match(src,/agent01l_linux_real_git_cgroup_custody_after_restart/);
  assert.match(src,/AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1/);
  assert.match(src,/sec06c_owned_git_host_sigkill_recovery_original_journal/);
  const positive=src.split("  linux-agent01l-hosted-positive:")[1];
  assert.doesNotMatch(positive,/continue-on-error: true/);
  assert.doesNotMatch(positive,/\|\| true/);
  assert.doesNotMatch(positive,/set \+e/);
  assert.match(positive,/if: \$\{\{ !cancelled\(\) \}\}/);
});


test("CI-POLICY-01-R1: all nine retired triggers keep native jobs and reject reinsertion", () => {
  const records = [
    ["lu-cp02-grouped-native.yml",["source-and-frontend","native"],true],
    ["lu-cp03-grouped-native.yml",["source-and-frontend","native"],true],
    ["lu-cp04b3-native-provision.yml",["browser-candidate","native-provider-routing","engine-and-model"],false],
    ["lu-cp05e-closeout.yml",["independent-original-browser-byte-review","installed-repaired-windows-gguf"],false],
    ["lu-cp05e-grouped-native.yml",["linux-agent01l-hosted-positive","core01e-sec06c-os-owned-probes","genuine-local-qwen","windows-native-witness","windows-nsis-engine-and-current-lu","windows-headed-browser-candidate","current-browser-byte-audit-not-approval"],false],
    ["lu-cp06er2-consolidated.yml",["exact-source","native","dual-gguf","browser-custody","core01e-sec06-native","core01e-sec06c-os-owned-git","core01e-sec06c-os-owned-probes","core01e-sec06c-os-owned-native","core01e-sec06f-original-receipts"],false],
    ["lu-cp06er3-native-web.yml",["native-web"],false],
    ["lu-cp12br7r1-windows-collaboration.yml",["windows-native-protocol","windows-desktop-source-check","r4-r2-source-lock","r4-r2-frontend-ui","r4-r2-native-core","r4-r2-desktop-windows"],false],
    ["lu-dev-offline-cache.yml",["collect-exact-offline-inputs"],false]
  ];
  for(const [file,jobs,reusable] of records) {
    const src=fs.readFileSync(".github/workflows/"+file,"utf8").replace(/\r\n?/g,"\n");
    const check=inspectOne(file,src);
    assert.deepEqual(check.problems,[],file);
    assert.deepEqual(check.events,reusable?["workflow_dispatch","workflow_call"]:["workflow_dispatch"],file);
    for(const job of jobs)assert.match(src,new RegExp("^  "+job+":","m"),file+" "+job);
    const unsafe=src.replace(/^on:\n  workflow_dispatch:/m,"on:\n  push:\n  workflow_dispatch:");
    assert.ok(inspectOne(file,unsafe).problems.includes("HEAVY_WORKFLOW_AUTOMATIC_FANOUT"),file);
  }
  const src=fs.readFileSync(".github/workflows/lu-cp12br7r1-windows-collaboration.yml","utf8");
  assert.match(src,/needs: \[windows-native-protocol, windows-desktop-source-check\]/);
  assert.match(src,/github\.ref == 'refs\/heads\/lu\/cp10-consolidated-ux-20261120'/);
  assert.match(src,/needs\.windows-native-protocol\.result == 'success'/);
  assert.match(src,/needs\.windows-desktop-source-check\.result == 'success'/);
  assert.doesNotMatch(src,/github\.event\.head_commit\.message/);
  assert.match(src,/--features windows_dx_eval/);
  assert.match(src,/Default-off Windows transport regression failed/);
  assert.match(src,/generation_queue::tests/);
  assert.match(src,/turn_coordinator::tests/);
  const cp6=fs.readFileSync(".github/workflows/lu-cp06er2-consolidated.yml","utf8");
  assert.match(cp6,/github\.ref == 'refs\/heads\/lu\/cp01-attachment-transaction-20261116'/);
});
