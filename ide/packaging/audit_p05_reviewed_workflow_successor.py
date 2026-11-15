#!/usr/bin/env python3
"""Source-only successor witness for P05's seven divergent IDE workflows.

This does NOT replace the historical immutable 38-blob P05 checker, approve
push-trigger reduction, certify OS runtime/security or authorize release.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

HISTORICAL_INVENTORY_GIT_BLOB = "5299b1ca6981f341d2855f549e567b8c2747dc13"
# Each independently source-compared path keeps BOTH original and successor
# exact Git blob identities. All other 31 must equal their original P05 pins.
REVIEWED = {
    ".github/workflows/ide-d7-windows-worker.yml": (
        "518635e58bbac5b249bfeadba7e60c90b9d0010c",
        "6832a33bdab3539d85805ccd43a9aa479758d08d",
        "push_trigger_removed_requires_authorization"),
    ".github/workflows/ide-d7b-native-acceptance.yml": (
        "f70955319c621cbd08cd2a7a6f7eac77caffc3a1",
        "c247635e41a4c325a76cc5223239f890898c92c1",
        "new_protected_cargo_native_proof"),
    ".github/workflows/ide-d7d-protected-service.yml": (
        "ce56f987cb056c01ef7be58318d70c9e136aafad",
        "35a0a506186694caaa6ff72d8bfdeec694cf0235",
        "dap_feature_and_native_scm_proof"),
    ".github/workflows/ide-supervisor-core.yml": (
        "e0d26f80713cdf498c3bfdf82645498dddf34e16",
        "11203a4e5f8cfeecee9effa00c88c981492be527",
        "push_trigger_removed_requires_authorization"),
    ".github/workflows/ide-wo6-native-os-acceptance.yml": (
        "d7627899b3a6cafc7b383f00abbb2b9e7107fe2d",
        "de7cae8464649481ff46f148341bd575aca045da",
        "windows_runner_and_protected_dap_evidence_changed"),
    ".github/workflows/ide-wo8-installed-windows.yml": (
        "8fadc3f1829e62123b9da6a5e6bbd49a3f3ec36f",
        "95138087b419a4b1f9baa052e717c34fc069ae44",
        "installed_win_p06_scoped_lock_and_new_uxa_checks"),
    ".github/workflows/ide-wo8-signed-protected-e2e.yml": (
        "08e2ab350b9e0c9e88958a40a3eeca59a3a66df5",
        "3cbaf9efc3836bfbe3a978cdd5511daac04f2380",
        "signed_manual_native_lldb_evidence_expanded"),
}
SHA = re.compile(r"^[0-9a-f]{40}$")


class Denied(ValueError):
    pass


def classify(inventory, actual_blobs):
    if (inventory.get("schema") != "lu.ide.p05.workflow-coverage.v1"
            or inventory.get("observedWorkflowCount") != 38
            or inventory.get("nativeReleaseSecurityClosure") != "pending"
            or inventory.get("releaseApproved") is not False):
        raise Denied("historical_p05_inventory_scope_changed")
    entries = inventory.get("requiredWorkflows")
    if not isinstance(entries, list) or len(entries) != 38:
        raise Denied("historical_p05_gate_count_changed")
    seen = set()
    changed = []
    for item in entries:
        if not isinstance(item, dict):
            raise Denied("invalid_historical_p05_row")
        path = item.get("path")
        old = item.get("blobSha")
        if (not isinstance(path, str) or path in seen
                or not path.startswith(".github/workflows/ide-")
                or not path.endswith(".yml")
                or not isinstance(old, str) or not SHA.fullmatch(old)
                or item.get("replacementAccepted") is not False
                or item.get("prTriggerPruningAuthorized") is not False):
            raise Denied("historical_p05_pin_or_policy_changed")
        seen.add(path)
        approved = REVIEWED.get(path)
        if approved and old != approved[0]:
            raise Denied("reviewed_successor_original_blob_not_immutable:" + path)
        expected = approved[1] if approved else old
        if actual_blobs.get(path) != expected:
            raise Denied("unreviewed_successor_workflow_drift:" + path)
        if approved:
            changed.append(path)
    if (set(actual_blobs) != seen or len(changed) != 7
            or set(changed) != set(REVIEWED)):
        raise Denied("successor_workflow_coverage_incomplete")
    return {
        "schema": "lu.ide.p05.reviewed-current-workflow-source-only.v1",
        "historicalPinnedWorkflows": 38,
        "identicalHistoricalWorkflows": 31,
        "historicallyDivergentWorkflows": 7,
        "reviewedCurrentGitSourceIdentitiesVerified": True,
        "historicalP05ImmutableCheckoutPass": False,
        "originalHistoricalDenialPreserved": True,
        "pushTriggerReductionAuthorizationRequired": [
            path for path, row in REVIEWED.items()
            if row[2] == "push_trigger_removed_requires_authorization"
        ],
        "originalNativeAcceptanceClosed": False,
        "originalSecurityRuntimeAccepted": False,
        "signedInstalledReleaseAccepted": False,
        "legalAndLicenseReviewApproved": False,
        "mainMergeAuthorized": False,
        "successorHasAuthorityToOverrideHistoricalP05": False,
        "releaseApproved": False,
    }


def git_blob(root, path):
    proc = subprocess.run(["git", "ls-tree", "HEAD", "--", path],
                          cwd=root, text=True, capture_output=True,
                          timeout=15, check=False)
    if proc.returncode:
        raise Denied("successor_git_tree_unreadable")
    fields = proc.stdout.strip().split("\t", 1)
    if len(fields) != 2 or fields[1] != path:
        raise Denied("successor_required_path_missing_or_tree_changed:" + path)
    words = fields[0].split()
    if len(words) != 3 or words[0] != "100644" or words[1] != "blob":
        raise Denied("successor_non_regular_git_blob:" + path)
    return words[2]


def checkout(root):
    root = Path(root).resolve(strict=True)
    if not (root / ".git").exists():
        raise Denied("successor_real_git_checkout_required")
    item = "ide/packaging/p05-workflow-coverage.json"
    if git_blob(root, item) != HISTORICAL_INVENTORY_GIT_BLOB:
        raise Denied("historical_p05_inventory_modified")
    inventory = json.loads((root / item).read_text(encoding="utf-8"))
    blobs = {}
    for row in inventory["requiredWorkflows"]:
        path = row["path"]
        file = root / path
        if not file.is_file() or file.is_symlink():
            raise Denied("successor_workflow_missing_or_linked:" + path)
        blob = git_blob(root, path)
        # Compare actual checked-out bytes through Git's configured filters:
        # only citing the HEAD blob is not enough to attest a working copy.
        result = subprocess.run(["git", "hash-object", "--", path], cwd=root,
                                text=True, capture_output=True,
                                timeout=15, check=False)
        if result.returncode or result.stdout.strip() != blob:
            raise Denied("successor_workflow_worktree_differs_from_head:" + path)
        blobs[path] = blob
    return classify(inventory, blobs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = checkout(args.repo_root)
    except (Denied, OSError, ValueError, TypeError, KeyError,
            subprocess.TimeoutExpired) as exc:
        print("P05_CURRENT_WORKFLOW_SOURCE=DENIED:" + str(exc), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
