#!/usr/bin/env python3
"""Exact source-only B4/B5 current-stack native boundary review.

Historical B4 editor-only and B5 performance-only source checks remain in
their original workflows. This distinct current-stack review acknowledges
ONLY pinned integration changes and cannot replace OS/kernel, P05 or release
acceptance. No environment variable or renderer input can add exceptions.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

SCHEMA = "lu.ide.merge02.reviewed-integrated-native-boundaries.v1"
BASE = "06beb2d8725f1175a1c4eb930e829afd582a6e96"
SCOPE = ("ide/host", "ide/supervisor", "ide/app/src-tauri", "ide/packaging")
HEX = re.compile(r"[a-f0-9]{40}\Z")


class Denied(ValueError):
    pass


def compare(review: dict, observed: dict[str, str]) -> int:
    if review.get("schema") != SCHEMA or review.get("baseSha") != BASE:
        raise Denied("reviewed_current_native_source_baseline_changed")
    if review.get("sourceScope") != list(SCOPE) or (
        review.get("originalHistoricalB4AndB5GuardsRetained") is not True
        or review.get("sourceReviewApprovedForExactListOnly") is not True
    ):
        raise Denied("historical_source_boundary_or_scoped_review_missing")
    for key in ("nativeWindowsAccepted", "nativeLinuxAccepted",
                "installedGuiAccepted", "releaseApproved", "mergeApproved"):
        if review.get(key) is not False:
            raise Denied("source_identity_forged_native_or_release_acceptance")
    rows = review.get("files")
    if not isinstance(rows, list) or len(rows) < 1 or len(rows) > 128:
        raise Denied("reviewed_source_rows_missing_or_unbounded")
    expected = {}
    for entry in rows:
        if not isinstance(entry, dict) or set(entry) != {"path", "sha", "scope"}:
            raise Denied("unexpected_reviewed_source_row")
        path, sha, reason = entry["path"], entry["sha"], entry["scope"]
        if (not isinstance(path, str)
                or not any(path.startswith(prefix + "/") for prefix in SCOPE)
                or (not path.endswith((".rs", ".py", ".json", ".toml", ".ps1"))
                    and path != "ide/packaging/upstream-licenses/README.md")
                or any(part in ("", ".", "..") for part in path.split("/"))
                or not isinstance(sha, str) or HEX.fullmatch(sha) is None
                or not isinstance(reason, str) or not 8 <= len(reason) <= 200
                or path in expected):
            raise Denied("unsafe_or_duplicate_source_exception")
        expected[path] = sha
    if set(observed) != set(expected):
        raise Denied("reviewed_current_native_source_path_set_drift")
    if any(observed[path] != sha for path, sha in expected.items()):
        raise Denied("reviewed_current_native_source_blob_drift")
    return len(expected)


def audit(root: Path, manifest: Path) -> int:
    root = root.resolve(strict=True)
    if manifest.is_symlink():
        raise Denied("linked_boundary_review")
    review = json.loads(manifest.read_text(encoding="utf-8"))
    def git(*args):
        try:
            return subprocess.run(
                ["git", "-C", str(root), *args], check=True, timeout=20,
                capture_output=True, text=True,
            ).stdout.strip()
        except (subprocess.CalledProcessError, OSError, subprocess.TimeoutExpired) as exc:
            raise Denied("reviewed_native_git_source_unavailable") from exc
    if git("rev-parse", "--is-inside-work-tree") != "true":
        raise Denied("source_checkout_not_git")
    if git("merge-base", BASE, "HEAD") != BASE:
        raise Denied("reviewed_integration_base_not_ancestor")
    changed = [p for p in git("diff", "--name-only", BASE, "HEAD", "--", *SCOPE).splitlines() if p]
    observed = {}
    for path in changed:
        source = root / path
        if source.is_symlink() or not source.is_file():
            raise Denied("linked_or_missing_reviewed_native_file")
        committed = git("rev-parse", "HEAD:" + path)
        if committed != git("hash-object", "--", path):
            raise Denied("native_worktree_bytes_not_exact_reviewed_head")
        observed[path] = committed
    return compare(review, observed)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--repo-root", default=".", type=Path)
    args = p.parse_args()
    try:
        count = audit(args.repo_root,
                      args.repo_root / "ide/ci/merge02-reviewed-native-boundaries.json")
    except (Denied, OSError, ValueError, KeyError, TypeError) as exc:
        print("DENIED:" + str(exc), file=sys.stderr)
        return 2
    print(f"IDE_MERGE02_REVIEWED_CURRENT_NATIVE_BLOBS={count};"
          "historical_B4_B5_checks=retained;native_os_acceptance=OPEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
