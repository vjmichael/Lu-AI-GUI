#!/usr/bin/env python3
"""R4 source-only bridge: preserve original MERGE02C/P06 and admit exact R2 IPC locks.

Historical checks execute in a disposable Git worktree at the frozen original
PR head, never against the altered current lock. The R2 verifier independently
reconstructs the only approved current host/desktop dependency changes.
No installer, credential, worker, human consent or release approval is granted.
"""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from verify_ide_merge02c_native_locks import PINS, verify_bytes
from verify_ide_precore_lock_successor import check as verify_ipc_successor

HISTORICAL_SHA = "f1f0ed4bc1e740d2a9f85615b988b779a46651db"
SUPERVISOR = "ide/supervisor/Cargo.lock"
PLATFORMS = ("windows-2022", "ubuntu-24.04")


class Denied(ValueError):
    pass


def invoke(argv, timeout=180):
    try:
        proc = subprocess.run(
            [str(part) for part in argv],
            check=False, text=True, capture_output=True, timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise Denied("pinned_git_or_historical_verifier_unavailable") from exc
    if proc.returncode:
        raise Denied("reviewed_command_failed: " + " ".join(str(x) for x in argv[:3])
                     + " / " + (proc.stderr or proc.stdout)[-700:].strip())
    return proc.stdout.strip()


def git(root, *args):
    return invoke(["git", "-C", root, *args])


def verify_current(root):
    """Require exact reviewed current lock bytes and tracked Git identity."""
    head = git(root, "rev-parse", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        raise Denied("current_checkout_sha_invalid")
    if git(root, "merge-base", HISTORICAL_SHA, "HEAD") != HISTORICAL_SHA:
        raise Denied("historical_integrated_parent_not_ancestor")
    verify_ipc_successor(root)
    attrs = (root / ".gitattributes").read_text(encoding="utf-8").splitlines()
    for path in ("ide/host/Cargo.lock", SUPERVISOR, "ide/app/src-tauri/Cargo.lock"):
        source = root / path
        if not source.is_file() or source.is_symlink():
            raise Denied("reviewed_lock_missing_or_linked")
        if path + " text eol=lf" not in attrs:
            raise Denied("reviewed_lock_line_endings_not_pinned")
        if git(root, "rev-parse", "HEAD:" + path) != git(root, "hash-object", "--", path):
            raise Denied("current_lock_worktree_does_not_match_exact_commit")
    original_blob, digest, length, count = PINS[SUPERVISOR]
    data = (root / SUPERVISOR).read_bytes()
    verify_bytes(data, digest, length, count)
    if git(root, "rev-parse", "HEAD:" + SUPERVISOR) != original_blob:
        raise Denied("unreviewed_supervisor_lock_successor")
    return head


def verify_historical(root, platform):
    """Run both *unchanged* historical verifiers at their original Git SHA."""
    if platform is not None and platform not in PLATFORMS:
        raise Denied("unreviewed_platform")
    temp_root = os.environ.get("RUNNER_TEMP")
    with tempfile.TemporaryDirectory(prefix="ide-precore-r4-", dir=temp_root) as directory:
        hist = Path(directory) / "historical"
        attached = False
        try:
            git(root, "worktree", "add", "--detach", str(hist), HISTORICAL_SHA)
            attached = True
            if git(hist, "rev-parse", "HEAD") != HISTORICAL_SHA:
                raise Denied("historical_worktree_wrong_exact_sha")
            invoke([sys.executable,
                    hist / "ide/packaging/verify_ide_merge02c_native_locks.py",
                    "--repo-root", hist])
            if platform is not None:
                invoke([sys.executable,
                        hist / "ide/packaging/verify_p06_candidate_lock.py",
                        "--repo-root", hist, "--platform", platform,
                        "--ide-merge02-successor"])
        finally:
            if attached:
                git(root, "worktree", "remove", "--force", str(hist))


def check(root, platform=None):
    root = Path(root).resolve(strict=True)
    if platform is not None and platform not in PLATFORMS:
        raise Denied("unreviewed_platform")
    head = verify_current(root)
    verify_historical(root, platform)
    return head


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--repo-root", type=Path, required=True)
    p.add_argument("--platform", choices=PLATFORMS)
    args = p.parse_args()
    try:
        head = check(args.repo_root, args.platform)
    except (Denied, ValueError, OSError, subprocess.CalledProcessError) as exc:
        print("DENIED: " + str(exc), file=sys.stderr)
        return 2
    print("IDE_PRECORE_R4_ORIGINAL_MERGE02C=PASS_HISTORICAL_EXACT_SHA")
    if args.platform:
        print("IDE_PRECORE_R4_ORIGINAL_P06=PASS_HISTORICAL_" + args.platform)
    print("IDE_PRECORE_R4_IPC_R2_LOCK_SUCCESSOR=PASS_EXACT_SOURCE_" + head)
    print("IDE_PRECORE_R4_SIGNED_INSTALLED_AND_RELEASE=NOT_APPROVED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
