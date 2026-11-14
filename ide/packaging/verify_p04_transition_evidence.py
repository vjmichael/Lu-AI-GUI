#!/usr/bin/env python3
"""P04 release transition metadata gate, NOT independent OS or signing proof.

No JSON document, including complete fixture data, authorizes a release.
An independent reviewer must fetch the linked exact GitHub runs/artifacts,
inspect native OS receipts, signed binaries, licensing, and both versions.
"""
import argparse
import json
from pathlib import Path
import re
import sys

SHA40 = re.compile(r"^[a-fA-F0-9]{40}$")
SHA64 = re.compile(r"^[a-fA-F0-9]{64}$")
REQUIRED = {
    "windows-signed-reinstall": ("windows-2022", "ide-p04-signed-reinstall.yml", False),
    "windows-different-version-upgrade": ("windows-2022", "ide-p04-upgrade-native.yml", True),
    "windows-failed-upgrade-rollback": ("windows-2022", "ide-p04-upgrade-native.yml", True),
    "linux-signed-deb-reinstall": ("ubuntu-24.04", "ide-p04-linux-upgrade.yml", False),
    "linux-failed-upgrade-rollback": ("ubuntu-24.04", "ide-p04-linux-upgrade.yml", True),
}

class Denied(ValueError):
    pass

def sha64(value):
    return isinstance(value, str) and SHA64.fullmatch(value) is not None

def evaluate(document, exact_head):
    if not isinstance(exact_head, str) or not SHA40.fullmatch(exact_head):
        raise Denied("invalid_exact_head")
    if not isinstance(document, dict) or document.get("schema") != "lu.ide.p04.transition.v1":
        raise Denied("unsupported_p04_evidence_schema")
    if document.get("sourceSha") != exact_head or document.get("releaseApproved") is not False:
        raise Denied("untrusted_source_or_unauthorized_release_promotion")
    records = document.get("observations")
    if not isinstance(records, list) or len(records) > len(REQUIRED):
        raise Denied("invalid_transition_observations")
    seen = set()
    for row in records:
        if not isinstance(row, dict):
            raise Denied("non_object_transition_record")
        gate = row.get("gate")
        if gate not in REQUIRED or gate in seen:
            raise Denied("unknown_or_duplicate_transition")
        seen.add(gate)
        os_name, workflow, changing = REQUIRED[gate]
        if (row.get("sourceSha") != exact_head or row.get("runnerOs") != os_name
                or row.get("workflow") != workflow or row.get("result") != "passed"
                or row.get("skipped") is not False):
            raise Denied("wrong_native_head_os_workflow_or_outcome")
        if (type(row.get("runId")) is not int or row["runId"] <= 0
                or type(row.get("jobId")) is not int or row["jobId"] <= 0):
            raise Denied("missing_real_native_job")
        for field in ("witnessSha256", "oldInstallerSha256", "newInstallerSha256",
                      "catalogBeforeSha256", "catalogAfterSha256",
                      "sourceBeforeSha256", "sourceAfterSha256"):
            if not sha64(row.get(field)):
                raise Denied("missing_transition_artifact_or_state_digest")
        if (row["catalogBeforeSha256"].lower() != row["catalogAfterSha256"].lower()
                or row["sourceBeforeSha256"].lower() != row["sourceAfterSha256"].lower()):
            raise Denied("user_catalog_or_source_changed")
        if ((row["oldInstallerSha256"].lower() != row["newInstallerSha256"].lower()) != changing):
            raise Denied("same_version_misreported_as_upgrade_or_wrong_reinstall_identity")
        for key in ("beforeVersion", "afterVersion"):
            if not isinstance(row.get(key), str) or not row[key]:
                raise Denied("unidentified_installed_versions")
        if changing:
            if not isinstance(row.get("attemptedVersion"), str) or (
                    not row["attemptedVersion"] or row["attemptedVersion"] == row["beforeVersion"]):
                raise Denied("upgrade_attempt_must_change_version")
            if "rollback" in gate:
                if row["afterVersion"] != row["beforeVersion"]:
                    raise Denied("rollback_did_not_restore_old_version")
                if row.get("restoredInstallerSha256") != row["oldInstallerSha256"]:
                    raise Denied("rollback_did_not_restore_original_signed_candidate")
            elif row["afterVersion"] != row["attemptedVersion"]:
                raise Denied("upgrade_result_version_not_observed")
        elif row["afterVersion"] != row["beforeVersion"]:
            raise Denied("reinstall_changed_version")
        if row.get("signedAndReviewed") is not True or row.get("processTreeEmpty") is not True:
            raise Denied("unverified_package_or_os_cleanup")
        if row.get("noAutomaticReplay") is not True or row.get("protectedServiceRemoved") is not True:
            raise Denied("unreconciled_worker_or_service")
        if "rollback" in gate:
            if row.get("failedUpgradeObserved") is not True or \
                    row.get("previousVersionRestored") is not True:
                raise Denied("rollback_not_actually_observed")
        elif row.get("failedUpgradeObserved") is True:
            raise Denied("unexpected_failure_in_successful_transition")
    missing = set(REQUIRED).difference(seen)
    if missing:
        raise Denied("missing_native_transitions:" + ",".join(sorted(missing)))
    return {
        "sourceSha": exact_head,
        "metadataCoverageComplete": True,
        "independentlyFetchedNativeEvidence": False,
        "signedReleaseApproved": False,
        "actualUpgradeAndRollbackIndependentlyAccepted": False,
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    try:
        result = evaluate(json.loads(args.evidence.read_text(encoding="utf-8")), args.source_sha)
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        code = str(exc) if isinstance(exc, Denied) else "invalid_transition_evidence"
        print("DENIED: " + code, file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
