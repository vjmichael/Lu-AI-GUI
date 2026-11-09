import {test} from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {spawnSync} from "node:child_process";
import {fileURLToPath} from "node:url";

const tool = fileURLToPath(new URL("./lu-ledger.mjs", import.meta.url));
const repo = "vjmichael/Lu-AI-GUI";

function invoke(flags) {
  return spawnSync(process.execPath, [tool, ...flags], {
    encoding: "utf8",
    env: {...process.env, GITHUB_REPOSITORY: repo}
  });
}
function pr(sha, number = 1) {
  return {
    number, title: "File | Reader <untrusted>", draft: true,
    head: {sha, ref: "lu/file-reader"}, base: {ref: "main"}
  };
}
test("update is idempotent; check rejects changed heads without overwriting manually reviewed notes", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lu-ledger-"));
  try {
    const file = path.join(root, "ledger.md");
    const fixture = path.join(root, "prs.json");
    const graph = path.join(root, "graph.json");
    fs.writeFileSync(file, "# Hand-curated notes\n\nKEEP THIS APPROVAL WORDING\n\n"
      + "<!-- LU_OPEN_PRS_START -->\nOld table\n<!-- LU_OPEN_PRS_END -->\n\n"
      + "## Human status\nNot tested\n");
    fs.writeFileSync(fixture, JSON.stringify([pr("a".repeat(40))]));
    const flags = ["--file", file, "--fixture", fixture, "--graph", graph];
    assert.equal(invoke(["--update", ...flags]).status, 0);
    const saved = fs.readFileSync(file, "utf8");
    assert.match(saved, /KEEP THIS APPROVAL WORDING/);
    assert.match(saved, /Human status\nNot tested/);
    assert.match(saved, /lu\/file-reader/);
    assert.ok(saved.includes("File \\| Reader &lt;untrusted&gt;"));
    const snapshot = JSON.parse(fs.readFileSync(graph, "utf8"));
    assert.equal(snapshot.openCount, 1);
    assert.deepEqual(snapshot.pullRequests[0].head, {ref: "lu/file-reader", sha: "a".repeat(40)});
    assert.equal(snapshot.pullRequests[0].baseClassification, "main");
    assert.equal(invoke(["--check", ...flags]).status, 0);
    assert.equal(invoke(["--update", ...flags]).status, 0);
    assert.equal(fs.readFileSync(file, "utf8"), saved);
    fs.writeFileSync(fixture, JSON.stringify([pr("b".repeat(40))]));
    assert.equal(invoke(["--check", ...flags]).status, 1);
    assert.equal(fs.readFileSync(file, "utf8"), saved, "check is read-only");
    assert.equal(JSON.parse(fs.readFileSync(graph, "utf8")).pullRequests[0].head.sha, "a".repeat(40));
    assert.equal(invoke(["--update", ...flags]).status, 0);
    assert.match(fs.readFileSync(file, "utf8"), /bbbbbbbbbbbb/);
    assert.equal(JSON.parse(fs.readFileSync(graph, "utf8")).pullRequests[0].head.sha, "b".repeat(40));
  } finally {
    fs.rmSync(root, {recursive: true, force: true});
  }
});
test("refuses duplicate or malformed markers and metadata", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lu-ledger-bad-"));
  try {
    const file = path.join(root, "ledger.md");
    const fixture = path.join(root, "prs.json");
    fs.writeFileSync(file, "<!-- LU_OPEN_PRS_START -->\n<!-- LU_OPEN_PRS_START -->\n<!-- LU_OPEN_PRS_END -->");
    fs.writeFileSync(fixture, JSON.stringify([pr("a".repeat(40))]));
    assert.equal(invoke(["--update", "--file", file, "--fixture", fixture]).status, 1);
    fs.writeFileSync(file, "<!-- LU_OPEN_PRS_START -->\nOld\n<!-- LU_OPEN_PRS_END -->");
    fs.writeFileSync(fixture, JSON.stringify([pr("not-sha")]));
    assert.equal(invoke(["--update", "--file", file, "--fixture", fixture]).status, 1);
    assert.match(fs.readFileSync(file, "utf8"), /Old/);
  } finally {
    fs.rmSync(root, {recursive: true, force: true});
  }
});
