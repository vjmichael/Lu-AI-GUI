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
REVIEWED_SUPERVISOR_BLOB = "d3f1af3f66f29d8c7d08cae826131de27b9dfe7c"
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


def reproduce_supervisor_lock(original):
    """Only rustix/zeroize links in the existing 171-package original graph."""
    _, digest, length, count = PINS[SUPERVISOR]
    verify_bytes(original, digest, length, count)
    marker = b'[[package]]\nname = "lu-ide-workspace"\n'
    if original.count(marker) != 1:
        raise Denied("original_supervisor_workspace_package_not_unique")
    prefix, tail = original.split(marker, 1)
    if b'\n[[package]]' not in tail:
        raise Denied("original_supervisor_following_package_missing")
    local, remainder = tail.split(b'\n[[package]]', 1)
    rand_anchor = b' "rand",\n "serde",'
    end_anchor = b' "ureq",\n "zip",'
    if (local.count(rand_anchor) != 1 or local.count(end_anchor) != 1
            or b' "rustix",\n' in local or b' "zeroize",\n' in local):
        raise Denied("unreviewed_original_supervisor_package_layout")
    updated = local.replace(rand_anchor, b' "rand",\n "rustix",\n "serde",', 1)
    updated = updated.replace(end_anchor, b' "ureq",\n "zeroize",\n "zip",', 1)
    if len(updated) != len(local) + 23:
        raise Denied("unexpected_supervisor_lock_delta")
    return prefix + marker + updated + b'\n[[package]]' + remainder


def verify_supervisor_successor(root, current):
    """Verify original Git blob/hash and exact successor bytes without Cargo update."""
    original_blob, digest, size, packages = PINS[SUPERVISOR]
    if git(root, "rev-parse", HISTORICAL_SHA + ":" + SUPERVISOR) != original_blob:
        raise Denied("original_supervisor_lock_blob_mismatch")
    try:
        base = subprocess.check_output(
            ["git", "-C", str(root), "show", HISTORICAL_SHA + ":" + SUPERVISOR],
            timeout=20,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        raise Denied("original_supervisor_lock_unavailable") from exc
    verify_bytes(base, digest, size, packages)
    exact = reproduce_supervisor_lock(base)
    if current != exact:
        raise Denied("supervisor_lock_successor_bytes_unreviewed")
    if git(root, "rev-parse", "HEAD:" + SUPERVISOR) != REVIEWED_SUPERVISOR_BLOB:
        raise Denied("supervisor_lock_successor_blob_unreviewed")
    return True


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
    data = (root / SUPERVISOR).read_bytes()
    verify_supervisor_successor(root, data)
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
