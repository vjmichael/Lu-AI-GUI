#!/usr/bin/env python3
"""PLAN-07D independent original-ANSF installed-app decision witness.

Read-only; requires the original PLAN-06 two-project fixture and additionally
validates immutable PLAN-07C prepared/terminal review records. This cannot
claim UI interaction by itself. A UIA/AT-SPI runner must supply that evidence.
"""
import argparse
import hashlib
import json
import pathlib
import sqlite3
import subprocess
import sys
import tempfile

PHASES = {"approved_for_review", "refused", "cancelled", "interrupted", "stale_rejected"}
EXPECTED_SCHEMA = "lu-project-task-human-update-review-v1"

def sha(data):
    return hashlib.sha256(data).hexdigest()

def fail(reason):
    raise RuntimeError("PLAN07D_ANSF_REFUSED: " + reason)

def witness(profile, seed, expected):
    db = profile / "ansf" / "library.db"
    if not db.is_file() or db.is_symlink():
        fail("missing/unsafe original SQLite database")
    with tempfile.TemporaryDirectory(prefix="plan07d-readonly-") as tmp:
        # First reuse the independent PLAN-06 original-source, board,
        # preserved-user-memory and no-execution oracle unchanged.
        original = pathlib.Path(tmp) / "plan06-original.json"
        subprocess.run([sys.executable, "scripts/plan06-readonly-ansf-witness.py",
                        "--profile", str(profile), "--seed", str(seed),
                        "--output", str(original)], check=True)
        prior = json.loads(original.read_text(encoding="utf-8"))
    conn = sqlite3.connect(db.as_uri() + "?mode=ro", uri=True, timeout=3)
    conn.execute("PRAGMA query_only=ON")
    try:
        if conn.execute("PRAGMA integrity_check").fetchone() != ("ok",):
            fail("SQLite integrity check failed")
        tables = {row[0] for row in conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table'")}
        if "ansf_project_task_update_review_events" not in tables:
            if expected:
                fail("no PLAN-07C review journal after requested UI decisions")
            return {"schema": "lu-plan07d-independent-ansf-witness-v1",
                    "original": prior, "decisions": [], "phases": [],
                    "noExecutionAuthority": True}
        rows = conn.execute("""
            SELECT review_id, sequence, event_sha256, previous_sha256,
                   register_id, event_json, no_completion_authority,
                   no_execution_authority
            FROM ansf_project_task_update_review_events
            ORDER BY rowid
        """).fetchall()
        by_id = {}
        original_ids = {p["registerId"] for p in prior["projects"]}
        for rid, seq, digest, previous, register, raw, no_complete, no_exec in rows:
            if not isinstance(raw, bytes) or len(raw) > 8192:
                fail("original event bytes missing or oversized")
            if no_complete != 1 or no_exec != 1 or register not in original_ids:
                fail("event has unauthorized authority or foreign project")
            event = json.loads(raw)
            if event.get("schema") != EXPECTED_SCHEMA or event.get("reviewId") != rid:
                fail("event source schema or review identity invalid")
            if event.get("sha256") != digest or event.get("sequence") != seq or (
                event.get("previousSha256") != previous or
                event.get("registerId") != register):
                fail("row-to-original-byte mismatch")
            unsigned = dict(event)
            unsigned["sha256"] = ""
            serialized = json.dumps(unsigned, ensure_ascii=False,
                                    separators=(",", ":")).encode("utf-8")
            if sha(serialized) != digest:
                fail("original event digest mismatch")
            if event.get("noCompletionAuthority") is not True or (
                event.get("noExecutionAuthority") is not True):
                fail("model or human acquired completion/execution authority")
            if not isinstance(event.get("operatorNote"), str) or not (
                5 <= len(event["operatorNote"]) <= 250):
                fail("operator note missing or invalid")
            entries = by_id.setdefault(rid, [])
            if not rid.startswith("lu-plan07c-review-") or seq != len(entries) + 1:
                fail("event replay, gap or reorder")
            if previous != (entries[-1]["sha256"] if entries else None):
                fail("event predecessor chain broken")
            if seq == 1:
                if event.get("phase") != "prepared" or event.get("humanConfirmed") is not False:
                    fail("original prepared event invalid")
            elif seq == 2:
                if event.get("phase") not in PHASES:
                    fail("unrecognized terminal decision")
                if event.get("humanConfirmed") != (
                        event["phase"] == "approved_for_review"):
                    fail("terminal approval flag invalid")
                identity = ["registerId", "expectedRegisterSha256", "previewSha256",
                            "originalGraphSha256", "originalStatusSha256", "taskId",
                            "expectedTaskVersion", "originalRequirementId",
                            "modelSuggestionSha256", "modelAction",
                            "operatorNote", "hostInstance"]
                if any(event.get(k) != entries[0].get(k) for k in identity):
                    fail("terminal decision changed prepared original")
            else:
                fail("more than one terminal decision")
            entries.append(event)
        decisions = [{"reviewId": rid, "phase": events[-1]["phase"],
                      "firstSha256": events[0]["sha256"],
                      "latestSha256": events[-1]["sha256"],
                      "registerId": events[0]["registerId"],
                      "taskId": events[0]["taskId"],
                      "sequence": len(events),
                      "modelSuggestionSha256": events[0]["modelSuggestionSha256"]}
                     for rid, events in by_id.items()]
        phases = [item["phase"] for item in decisions if item["sequence"] == 2]
        for p in expected:
            if phases.count(p) < expected.count(p):
                fail("required original decision phase absent: " + p)
        # Review consent is never a PLAN-05 register revision or work completion.
        return {"schema": "lu-plan07d-independent-ansf-witness-v1",
                "original": prior, "decisions": decisions, "phases": phases,
                "noExecutionAuthority": True}
    finally:
        conn.close()

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--profile", type=pathlib.Path, required=True)
    p.add_argument("--seed", type=pathlib.Path, required=True)
    p.add_argument("--output", type=pathlib.Path, required=True)
    p.add_argument("--require-phase", action="append", default=[],
                   choices=sorted(PHASES))
    args = p.parse_args()
    if args.output.exists():
        fail("refuse overwriting acceptance evidence")
    result = witness(args.profile, args.seed, args.require_phase)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n",
                           encoding="utf-8")
    print("PLAN07D_ORIGINAL_ANSF_READONLY_PASS reviews=" +
          str(len(result["decisions"])) + " terminal=" +
          str(len(result["phases"])))

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(2)
