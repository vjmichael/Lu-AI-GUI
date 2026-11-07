#!/usr/bin/env python3
"""WO8-F installed outcome evidence contract.

Local witness metadata is NEVER equivalent to independently verified native
GUI, kernel, service, or package evidence. It must be reconciled with exact
GitHub run/job logs and immutable external artifacts before acceptance.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import sys

SHA40 = re.compile(r"[a-fA-F0-9]{40}\Z")
SHA64 = re.compile(r"[a-fA-F0-9]{64}\Z")
GATES = {
    "windows-installed-shell": ("ide-wo8-installed-windows.yml","windows-2022"),
    "windows-project-picker": ("ide-wo8-installed-windows.yml","windows-2022"),
    "windows-explicit-save": ("ide-wo8-installed-windows.yml","windows-2022"),
    "windows-restart-persistence": ("ide-wo8-installed-windows.yml","windows-2022"),
    "windows-uninstall": ("ide-wo8-installed-windows.yml","windows-2022"),
    "linux-installed-deb": ("ide-wo8-installed-linux.yml","ubuntu-24.04"),
    "linux-native-window": ("ide-wo8-installed-linux.yml","ubuntu-24.04"),
    "linux-uninstall": ("ide-wo8-installed-linux.yml","ubuntu-24.04"),
    "signed-protected-cargo": ("ide-wo8-signed-protected-e2e.yml","windows-2022"),
    "signed-protected-debug": ("ide-wo8-signed-protected-e2e.yml","windows-2022"),
    "signed-cancel-recovery": ("ide-wo8-signed-protected-e2e.yml","windows-2022"),
}


class EvidenceDenied(ValueError):
    pass


def validate(data: object, exact_head: str) -> dict:
    if not isinstance(exact_head, str) or not SHA40.fullmatch(exact_head):
        raise EvidenceDenied("missing_exact_commit")
    if not isinstance(data, dict) or data.get("schema") != "lu.ide.wo8.installed.v1":
        raise EvidenceDenied("unsupported_wo8_evidence")
    if data.get("sourceSha") != exact_head:
        raise EvidenceDenied("source_head_drift")
    entries = data.get("observations")
    if not isinstance(entries, list) or len(entries) > 48:
        raise EvidenceDenied("invalid_observations")
    seen, jobs = set(), set()
    for row in entries:
        if not isinstance(row, dict):
            raise EvidenceDenied("invalid_native_record")
        gate = row.get("gate")
        if gate not in GATES or gate in seen:
            raise EvidenceDenied("unknown_or_duplicate_gate")
        seen.add(gate)
        workflow, platform = GATES[gate]
        if (row.get("workflow") != workflow or row.get("runnerOs") != platform
                or row.get("sourceSha") != exact_head):
            raise EvidenceDenied("wrong_workflow_os_or_commit")
        if row.get("result") != "passed" or row.get("skipped") is not False:
            raise EvidenceDenied("skipped_or_failed_native_gate")
        for key in ("installerSha256","installedBinarySha256","witnessSha256"):
            value = row.get(key)
            if not isinstance(value, str) or not SHA64.fullmatch(value):
                raise EvidenceDenied("unmeasured_artifact_or_witness")
        run, job = row.get("runId"), row.get("jobId")
        if type(run) is not int or type(job) is not int or run <= 0 or job <= 0:
            raise EvidenceDenied("missing_exact_native_ci_job")
        # Multiple gates from one exact native job are valid, but each
        # stage carries a separately fingerprinted native observation.
        identity=(run,job,gate)
        if identity in jobs:
            raise EvidenceDenied("repeated_gate_witness")
        jobs.add(identity)
        if gate.startswith("signed-") and row.get("packageSigned") is not True:
            raise EvidenceDenied("unsigned_artifact_used_for_protected_release")
        if not gate.startswith("signed-") and row.get("releaseApproved") is True:
            raise EvidenceDenied("unsigned_smoke_claimed_release")
    missing=GATES.keys()-seen
    if missing:
        raise EvidenceDenied("missing_installed_gate:" + ",".join(sorted(missing)))
    return {"sourceSha":exact_head,"coveredGates":len(entries),
            "metadataConsistent":True,
            "independentRunLogInspectionComplete":False,
            "realInstalledAcceptanceGranted":False,
            "releaseAccepted":False}


def main() -> int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--evidence",type=Path,required=True)
    parser.add_argument("--source-sha",required=True)
    args=parser.parse_args()
    try:
        output=validate(json.loads(args.evidence.read_text(encoding="utf-8")),args.source_sha)
    except (OSError,ValueError,TypeError,KeyError,json.JSONDecodeError) as exc:
        message=str(exc) if isinstance(exc,EvidenceDenied) else "invalid_evidence_document"
        print("DENIED: "+message,file=sys.stderr)
        return 1
    print(json.dumps(output,sort_keys=True))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
