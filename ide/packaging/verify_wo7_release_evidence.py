#!/usr/bin/env python3
"""WO7-E exact-head release evidence consistency gate (NOT OS attestation).

Each claimed acceptance must identify an independent CI run and OS witness.
This is an offline integrity/coverage check; independently inspect referenced
GitHub runs and native receipts before promoting any release in human review.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

from verify_wo7_nsis_release import Denied

SHA40 = re.compile(r"[a-fA-F0-9]{40}\Z")
SHA64 = re.compile(r"[a-fA-F0-9]{64}\Z")
REQUIRED = {
    "native-windows-cargo",
    "native-windows-dap",
    "native-linux-dap",
    "signed-installer",
    "protected-scm-install",
    "protected-run",
    "protected-test",
    "protected-debug",
    "host-crash-recovery",
    "service-crash-recovery",
    "source-tamper-denial",
    "installed-uninstall",
    "license-and-import-closure",
}
WORKFLOWS = {
    "native-windows-cargo": "ide-d7d-protected-service.yml",
    "native-windows-dap": "ide-wo6-native-os-acceptance.yml",
    "native-linux-dap": "ide-wo6-native-os-acceptance.yml",
}
MAX_ROWS = 64


def audit(data: object, exact_head: str) -> dict:
    if not isinstance(exact_head, str) or not SHA40.fullmatch(exact_head):
        raise Denied("invalid_exact_head")
    if not isinstance(data, dict) or data.get("schema") != "lu.ide.wo7.evidence.v1":
        raise Denied("unknown_evidence_schema")
    if data.get("sourceSha") != exact_head or data.get("cleanSource") is not True:
        raise Denied("dirty_or_wrong_head")
    rows = data.get("acceptance")
    if not isinstance(rows, list) or len(rows) > MAX_ROWS:
        raise Denied("acceptance_records_invalid")
    seen, jobs = set(), set()
    for row in rows:
        if not isinstance(row, dict):
            raise Denied("acceptance_record_not_object")
        name = row.get("gate")
        if name not in REQUIRED or name in seen:
            raise Denied("unknown_or_duplicate_gate")
        seen.add(name)
        if row.get("sourceSha") != exact_head or row.get("conclusion") != "success" or row.get("os") not in {"windows-2022", "windows-2025", "ubuntu-24.04"}:
            raise Denied("native_gate_status_or_head_wrong")
        run_id, job_id = row.get("runId"), row.get("jobId")
        if (type(run_id) is not int or run_id <= 0 or
                type(job_id) is not int or job_id <= 0 or
                (run_id, job_id) in jobs):
            raise Denied("missing_or_reused_exact_ci_job")
        jobs.add((run_id, job_id))
        if row.get("workflow") != WORKFLOWS.get(name, row.get("workflow")):
            raise Denied("native_workflow_path_mismatch")
        if not isinstance(row.get("workflow"), str) or not re.fullmatch(r"ide-[a-z0-9-]+\.yml", row["workflow"]):
            raise Denied("unknown_workflow_provenance")
        if not isinstance(row.get("witnessSha256"), str) or not SHA64.fullmatch(row["witnessSha256"]):
            raise Denied("missing_native_receipt_digest")
        if row.get("skipped") is not False or row.get("containmentVerified") is not True:
            raise Denied("unverified_or_skipped_security_gate")
    missing = REQUIRED - seen
    if missing:
        raise Denied("missing_acceptance_gates:" + ",".join(sorted(missing)))
    return {"sourceSha": exact_head, "gateCount": len(rows),
            "metadataComplete": True,
            "githubAndNativeEvidenceIndependentlyFetched": False,
            "releaseAccepted": False}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--exact-head", required=True)
    args = parser.parse_args()
    try:
        result = audit(json.loads(args.evidence.read_text(encoding="utf-8")), args.exact_head)
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_evidence"), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
