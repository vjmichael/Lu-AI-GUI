#!/usr/bin/env python3
"""Exact immutable standalone IDE Cargo lock verification, never release approval."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib

PINS = {
    "ide/host/Cargo.lock": (
        "90e52a812a834786d828fefb86d244732a882dd7",
        "00de403d7aeb63cd476f7694d52af1db1ff25a87fd2460a782ad2301edf893ef",
        39190, 166
    ),
    "ide/supervisor/Cargo.lock": (
        "cf3ea845e89c7b8242a75d71c9dd5288b5580595",
        "1fcc77eb3790e34b5815f69df2adb3948034d1944e12e4ddc904861038e1edfb",
        40607, 171
    ),
}
EXPECTED_ORIGIN = "f4e8a1a587c4aadd3d7293896324ceb8fa299738"

class Denied(ValueError):
    pass

def verify_bytes(data, sha256, size, package_count):
    if len(data) != size or hashlib.sha256(data).hexdigest() != sha256:
        raise Denied("native_lock_bytes_not_reviewed")
    if b"\r" in data or not data.endswith(b"\n"):
        raise Denied("native_lock_line_endings_not_pinned")
    try:
        graph = tomllib.loads(data.decode("utf-8"))
    except (UnicodeError, ValueError) as exc:
        raise Denied("native_lock_toml_invalid") from exc
    if graph.get("version") != 4 or len(graph.get("package", [])) != package_count:
        raise Denied("native_lock_graph_count_unreviewed")
    return True

def git(*argv, root):
    result = subprocess.run(["git", *argv], cwd=root, text=True,
                            capture_output=True, check=False, timeout=20)
    if result.returncode:
        raise Denied("native_lock_not_in_exact_git_head")
    return result.stdout.strip()

def verify(root):
    source = root.resolve(strict=True)
    try:
        review = json.loads((source / "ide/packaging/ide-merge02c-native-cargo-lock-receipts.json")
                            .read_text(encoding="utf-8"))
        attrs = (source / ".gitattributes").read_text(encoding="utf-8")
    except (OSError, ValueError) as exc:
        raise Denied("native_lock_review_not_readable") from exc
    if (review.get("schema") != "lu.ide.merge02c.native-lock-source-evidence.v1"
        or review.get("originalSourceSha") != EXPECTED_ORIGIN
        or review.get("sourceOnly") is not True
        or review.get("originatingRun") != 43743660974
        or review.get("originatingJobs") != {"linux":132902109472,"windows":132902109711}
        or any(review.get(key) is not False for key in (
            "releaseApproved", "legalReviewed", "physicalOsAccepted", "mainMergeAuthorized"
        ))):
        raise Denied("native_lock_review_or_release_scope_untrusted")
    expected = [{"path":path, "gitBlobSha":values[0], "sha256":values[1],
                 "bytes":values[2], "packages":values[3]}
                for path, values in PINS.items()]
    if review.get("locks") != expected:
        raise Denied("native_lock_self_authorized_pin_change")
    for rel, (blob_sha, sha256, size, package_count) in PINS.items():
        path = source / rel
        if not path.is_file() or path.is_symlink():
            raise Denied("native_lock_file_missing_or_linked")
        if rel + " text eol=lf" not in attrs.splitlines():
            raise Denied("native_lock_windows_lf_not_pinned")
        verify_bytes(path.read_bytes(), sha256, size, package_count)
        if git("rev-parse", "HEAD:" + rel, root=source) != blob_sha:
            raise Denied("native_lock_not_committed_at_reviewed_blob")
        if git("hash-object", rel, root=source) != blob_sha:
            raise Denied("native_lock_worktree_differs_from_head")
    return True

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    args = parser.parse_args()
    try:
        verify(args.repo_root)
    except (OSError, Denied, subprocess.TimeoutExpired) as exc:
        print("DENIED: " + str(exc), file=sys.stderr)
        return 2
    print("IDE_MERGE02C_TRACKED_CARGO_LOCK_BYTES=VERIFIED_SOURCE_ONLY_NOT_RELEASE")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
