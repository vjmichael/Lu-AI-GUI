#!/usr/bin/env python3
"""WO9-F installed native evidence contract, never an automatic release signer.

The source-owned JSON is a set of claims: even a syntactically complete
document CANNOT prove the external GitHub CI job, Authenticode identity, signed
installer, OS worker isolation, or installed binary. Those require independent
human/OS verification. CLI returns metadata consistency only.
"""
from __future__ import annotations
import argparse
import json
import re
from pathlib import Path

SHA40 = re.compile(r"[0-9a-f]{40}\Z")
SHA64 = re.compile(r"[0-9a-f]{64}\Z")
GATES = {
    "native-windows-git": ("ide-wo9-advanced-workflows.yml", "windows-2022"),
    "native-linux-git": ("ide-wo9-advanced-workflows.yml", "ubuntu-24.04"),
    "native-dag-restart-windows": ("ide-wo9-advanced-workflows.yml", "windows-2022"),
    "native-dag-restart-linux": ("ide-wo9-advanced-workflows.yml", "ubuntu-24.04"),
    "installed-windows-portfolio": ("ide-wo9-installed-native.yml", "windows-2022"),
    "installed-windows-reviewed-git": ("ide-wo9-installed-native.yml", "windows-2022"),
    "installed-linux-zip": ("ide-wo9-installed-native.yml", "ubuntu-24.04"),
    "installed-linux-uninstall": ("ide-wo9-installed-native.yml", "ubuntu-24.04"),
    "signed-windows-installed-protected-graph": ("ide-wo9-signed-protected-acceptance.yml", "windows-2022"),
    "signed-linux-installed-protected-graph": ("ide-wo9-signed-protected-acceptance.yml", "ubuntu-24.04"),
}


def check(data: object, exact_sha: str) -> dict:
    if not SHA40.fullmatch(exact_sha):
        raise ValueError("WO9_INVALID_COMMIT")
    if not isinstance(data, dict) or data.get("schema") != "lu.ide.wo9.native-release.v1":
        raise ValueError("WO9_UNSUPPORTED_EVIDENCE")
    if data.get("sourceSha") != exact_sha:
        raise ValueError("WO9_EVIDENCE_HEAD_CHANGED")
    records = data.get("observations")
    if not isinstance(records, list) or len(records) > 32:
        raise ValueError("WO9_INVALID_RECORD_COUNT")
    seen: set[str] = set()
    for row in records:
        if not isinstance(row, dict):
            raise ValueError("WO9_INVALID_RECORD")
        key = row.get("gate")
        if key not in GATES or key in seen:
            raise ValueError("WO9_UNKNOWN_OR_DUPLICATED_GATE")
        seen.add(key)
        workflow, os_name = GATES[key]
        if row.get("workflow") != workflow or row.get("runnerOs") != os_name:
            raise ValueError("WO9_CROSS_OS_OR_WORKFLOW_RECEIPT")
        if row.get("sourceSha") != exact_sha or row.get("conclusion") != "success":
            raise ValueError("WO9_STALE_OR_UNSUCCESSFUL_RESULT")
        if row.get("skipped") is not False:
            raise ValueError("WO9_SKIPPED_RESULT_DENIED")
        if type(row.get("runId")) is not int or type(row.get("jobId")) is not int:
            raise ValueError("WO9_NATIVE_RUN_ID_UNAVAILABLE")
        if row["runId"] <= 0 or row["jobId"] <= 0:
            raise ValueError("WO9_NATIVE_RUN_ID_INVALID")
        for field in ("installerSha256", "installedDesktopSha256", "nativeWitnessSha256"):
            value = row.get(field)
            if not isinstance(value, str) or not SHA64.fullmatch(value):
                raise ValueError("WO9_MISSING_ARTIFACT_DIGEST")
        if key.startswith("signed-"):
            if row.get("authenticodeVerified") is not True:
                raise ValueError("WO9_SIGNED_NATIVE_PACKAGE_NOT_VERIFIED")
            if row.get("protectedSupervisorVerified") is not True:
                raise ValueError("WO9_PROTECTED_OS_RECEIPT_NOT_VERIFIED")
        elif row.get("releaseApproved") is True:
            raise ValueError("WO9_UNSIGNED_SMOKE_MISLABELED_RELEASE")
    missing = sorted(GATES.keys() - seen)
    if missing:
        raise ValueError("WO9_MISSING_NATIVE_GATES:" + ",".join(missing))
    return {
        "sourceSha": exact_sha,
        "gateCount": len(seen),
        "metadataConsistent": True,
        "externalNativeWitnessesIndependentlyVerified": False,
        "signedInstallerAccepted": False,
        "releaseApproved": False,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence", type=Path, required=True)
    ap.add_argument("--source-sha", required=True)
    args = ap.parse_args()
    try:
        result = check(json.loads(args.evidence.read_text(encoding="utf-8")), args.source_sha)
    except (ValueError, OSError) as exc:
        print("DENIED:" + str(exc))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
