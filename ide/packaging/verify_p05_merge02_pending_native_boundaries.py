#!/usr/bin/env python3
"""Read-only pending P05 discrepancy audit: NEVER an acceptance replacement.

Intentionally returns nonzero for seven independently UNAUTHORIZED workflow
changes. The original P05 verifier and its 38 immutable refs remain the
sole historical-preservation gate until separate review/authorization.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

OLD_BLOB = "5299b1ca6981f341d2855f549e567b8c2747dc13"
SOURCE_HEAD = "bc4fd78685c342675e4a80556970e934125a0e20"
EXPECTED = {
    ".github/workflows/ide-d7-windows-worker.yml": ("518635e58bbac5b249bfeadba7e60c90b9d0010c", "6832a33bdab3539d85805ccd43a9aa479758d08d"),
    ".github/workflows/ide-d7b-native-acceptance.yml": ("f70955319c621cbd08cd2a7a6f7eac77caffc3a1", "c247635e41a4c325a76cc5223239f890898c92c1"),
    ".github/workflows/ide-d7d-protected-service.yml": ("ce56f987cb056c01ef7be58318d70c9e136aafad", "35a0a506186694caaa6ff72d8bfdeec694cf0235"),
    ".github/workflows/ide-supervisor-core.yml": ("e0d26f80713cdf498c3bfdf82645498dddf34e16", "11203a4e5f8cfeecee9effa00c88c981492be527"),
    ".github/workflows/ide-wo6-native-os-acceptance.yml": ("d7627899b3a6cafc7b383f00abbb2b9e7107fe2d", "de7cae8464649481ff46f148341bd575aca045da"),
    ".github/workflows/ide-wo8-installed-windows.yml": ("8fadc3f1829e62123b9da6a5e6bbd49a3f3ec36f", "95138087b419a4b1f9baa052e717c34fc069ae44"),
    ".github/workflows/ide-wo8-signed-protected-e2e.yml": ("08e2ab350b9e0c9e88958a40a3eeca59a3a66df5", "3cbaf9efc3836bfbe3a978cdd5511daac04f2380"),
}

class Denied(ValueError):
    pass


def classify_rows(approved: dict, observed: dict, pending: dict) -> dict:
    if len(approved) != 38 or set(approved) != set(observed):
        raise Denied("p05_missing_original_38_native_workflows")
    if set(pending) != set(EXPECTED):
        raise Denied("p05_pending_source_scope_changed")
    mismatched = {}
    for path, original in approved.items():
        if not isinstance(path, str) or not path.startswith(".github/workflows/ide-"):
            raise Denied("p05_unknown_protected_path")
        if original != observed[path]:
            mismatched[path] = (original, observed[path])
    if mismatched != EXPECTED:
        raise Denied("p05_unreviewed_historical_source_drift")
    if pending != EXPECTED:
        raise Denied("p05_mismatch_manifest_forged_or_outdated")
    return {
        "originalWorkflows": 38,
        "byteEqualWorkflows": 31,
        "unauthorizedChangedWorkflows": 7,
        "p05HistoricallyAccepted": False,
        "publicationAuthorized": False,
    }


def original_blob_id(data: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def git_head_blob(root: Path, rel: str) -> str:
    try:
        p = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--verify", "HEAD:" + rel],
            capture_output=True, text=True, check=True, timeout=15,
        )
    except (subprocess.SubprocessError, OSError) as err:
        raise Denied("p05_unreadable_source_head") from err
    sha = p.stdout.strip()
    if not re.fullmatch(r"[a-f0-9]{40}", sha):
        raise Denied("p05_invalid_git_blob_id")
    actual = root / rel
    if not actual.is_file() or actual.is_symlink():
        raise Denied("p05_missing_or_linked_workflow")
    if original_blob_id(actual.read_bytes()) != sha:
        raise Denied("p05_working_copy_does_not_equal_head")
    return sha


def report(root: Path) -> dict:
    root = root.resolve(strict=True)
    p05 = root / "ide/packaging/p05-workflow-coverage.json"
    if original_blob_id(p05.read_bytes()) != OLD_BLOB:
        raise Denied("p05_immutable_original_manifest_changed")
    source = json.loads(p05.read_text(encoding="utf-8"))
    review = json.loads(
        (root / "ide/packaging/p05-merge02-pending-native-boundaries.json").read_text(
            encoding="utf-8"
        )
    )
    if source.get("sourceCandidateSha") != SOURCE_HEAD:
        raise Denied("p05_historical_origin_changed")
    if review.get("schema") != "lu.ide.p05.merge02-unresolved-workflow-boundaries.v1":
        raise Denied("p05_review_schema_changed")
    if review.get("immutableHistoricalManifestBlob") != OLD_BLOB:
        raise Denied("p05_original_review_pin_changed")
    for key in (
        "originalP05AuditMayBeClaimedPassed",
        "permissionToPrunePushTriggers",
        "independentlyAuthorizedReplacement",
        "independentlyAcceptedNativeEquivalence",
        "installedOrNativeAcceptance",
        "signedOrReleaseApproved",
    ):
        if review.get(key) is not False:
            raise Denied("p05_unapproved_source_promotion:" + key)
    approval = {x["path"]: x["blobSha"] for x in source["requiredWorkflows"]}
    observed = {p: git_head_blob(root, p) for p in approval}
    pending = {x["path"]: (x["approvedHistoricalBlob"], x["currentUnacceptedBlob"])
               for x in review["workflows"]}
    if len(review["workflows"]) != 7 or any(
        x.get("independentReplacementApproved") is not False or
        x.get("historicalSourceRestorationAccepted") is not False or
        x.get("retainOriginalChecks") is not True
        for x in review["workflows"]
    ):
        raise Denied("p05_replacement_falsely_authorized")
    return classify_rows(approval, observed, pending)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = report(args.repo_root)
    except (Denied, ValueError, TypeError, KeyError, OSError) as exc:
        print("DENIED:" + str(exc), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    print("DENIED:P05 seven historical workflow replacements remain unauthorized", file=sys.stderr)
    # This diagnostic MUST NOT become an acceptance pass.
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
