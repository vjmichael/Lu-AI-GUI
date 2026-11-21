#!/usr/bin/env python3
"""Select ONLY an immutable, independently pinned P06 checkout graph.

The ordinary P06 verifier remains default-deny; this CI-only adapter selects
the verified original, IDE05, or IDE-MERGE02 source lock on an exact Git HEAD.
No remote Git, hooks, install authority, or release approval is granted.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from verify_p06_candidate_lock import (
    IDE05_SUCCESSOR,
    IDE_MERGE02_SUCCESSOR,
    LockDenied,
    check,
)

CARGO_PATH = "ide/app/src-tauri/Cargo.lock"
NPM_PATH = "ide/app/package-lock.json"
ORIGINAL_CARGO_BLOB = "87d0045a4be1c78651f080300aba71c14dbe1f30"
REVIEWED_NPM_BLOB = "dfe4412bfacfc22ad1149f5213f4a26aa49b4067"


def blob_sha1(data: bytes) -> str:
    """Git SHA-1 blob ID of exact bytes, without any checkout filters."""
    return hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()


def select_scoped_review(
    head_cargo: str, source_cargo: str, head_npm: str, source_npm: str
) -> str:
    """Fail closed on dirty/unreviewed graph bytes before invoking P06."""
    if head_cargo != source_cargo or head_npm != source_npm:
        raise LockDenied("p06_checked_out_lock_bytes_differ_from_head")
    if head_npm != REVIEWED_NPM_BLOB:
        raise LockDenied("p06_unreviewed_npm_blob")
    allowed = {
        ORIGINAL_CARGO_BLOB: "original",
        IDE05_SUCCESSOR["newCargoBlob"]: "ide05",
        IDE_MERGE02_SUCCESSOR["newCargoBlob"]: "ide-merge02",
    }
    if head_cargo not in allowed:
        raise LockDenied("p06_unreviewed_cargo_blob")
    return allowed[head_cargo]


def _head_blob(root: Path, relative_path: str) -> str:
    try:
        process = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--verify", "HEAD:" + relative_path],
            check=True, capture_output=True, text=True, timeout=15,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise LockDenied("p06_git_head_blob_unverifiable") from exc
    digest = process.stdout.strip().lower()
    if len(digest) != 40 or not all(c in "0123456789abcdef" for c in digest):
        raise LockDenied("p06_git_head_blob_invalid")
    return digest


def verify_checkout(root: Path, platform: str) -> dict:
    root = root.resolve(strict=True)
    # Exact checked-in blob identities must match actual bytes, not only
    # a source-controlled review JSON or a claimed event SHA.
    head_cargo = _head_blob(root, CARGO_PATH)
    head_npm = _head_blob(root, NPM_PATH)
    cargo = blob_sha1((root / CARGO_PATH).read_bytes())
    npm = blob_sha1((root / NPM_PATH).read_bytes())
    scope = select_scoped_review(head_cargo, cargo, head_npm, npm)
    if scope == "original":
        accepted = check(root, platform)
    elif scope == "ide05":
        accepted = check(
            root, platform,
            ide05_successor=json.loads(
                (root / "ide/packaging/ide05-reviewed-lock-successor.json").read_text(
                    encoding="utf-8"
                )
            ),
        )
    else:
        accepted = check(
            root, platform,
            ide_merge02_successor=json.loads(
                (root / "ide/packaging/ide-merge02-reviewed-lock-successor.json").read_text(
                    encoding="utf-8"
                )
            ),
        )
    if accepted.get("releaseApproved") is not False or accepted.get("signedOrLegallyApproved") is not False:
        raise LockDenied("p06_checkout_must_not_authorize_release")
    accepted["reviewedScope"] = scope
    return accepted


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument(
        "--platform", choices=("windows-2022", "ubuntu-24.04"), required=True
    )
    args = parser.parse_args()
    try:
        result = verify_checkout(args.repo_root, args.platform)
    except (OSError, ValueError, TypeError, KeyError) as exc:
        print("DENIED: " + str(exc), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
