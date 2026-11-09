#!/usr/bin/env python3
"""CI-only *self-measured* root-owned uv/Python bundle for native negative test.
NOT a reviewed upstream signature, license clearance, installer/release pin,
or human approval. Accepts exactly fixed source executables, no shell/PATH
tool selection from within a protected worker.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import struct


def digest_tree(root: Path) -> tuple[str, int]:
    hasher = hashlib.sha256(b"lu-ide-runtime-tree-v1\0")
    entries = []
    for file in root.rglob("*"):
        meta = file.lstat()
        if stat.S_ISLNK(meta.st_mode) or (
            not stat.S_ISREG(meta.st_mode) and not stat.S_ISDIR(meta.st_mode)
        ):
            raise ValueError(f"untrusted bundle entry: {file}")
        if meta.st_uid != 0 or meta.st_mode & 0o022:
            raise ValueError(f"bundle entry must be root-owned nonwritable: {file}")
        if meta.st_nlink != 1 and stat.S_ISREG(meta.st_mode):
            raise ValueError(f"hardlinked runtime file: {file}")
        if file.is_file():
            if meta.st_size > 1024**3:
                raise ValueError("oversize runtime file")
            rel = file.relative_to(root).as_posix()
            if len(rel.encode()) > 1024:
                raise ValueError("unsafe runtime path")
            sha = hashlib.sha256(file.read_bytes()).hexdigest()
            entries.append((rel, meta.st_size, sha))
    if len(entries) > 40000 or sum(row[1] for row in entries) > 4*1024**3:
        raise ValueError("runtime exceeds supervisor scan cap")
    for rel, size, sha in sorted(entries):
        hasher.update(rel.encode())
        hasher.update(b"\0")
        hasher.update(struct.pack("<Q", size))
        hasher.update(sha.encode())
        hasher.update(b"\0")
    return hasher.hexdigest(), len(entries)


def require_regular_file(path: Path) -> Path:
    # The source may be a symlink under a CI-managed CPython release; copy
    # the actual bytes, never retain that alias in the root-owned bundle.
    source = path.resolve(strict=True)
    if not source.is_file() or not stat.S_ISREG(source.stat().st_mode):
        raise ValueError(f"source executable/file is not regular: {path}")
    return source


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True)
    parser.add_argument("--uv-exe", required=True)
    parser.add_argument("--python-exe", required=True)
    parser.add_argument("--uv-license", required=True)
    parser.add_argument("--python-license", required=True)
    args = parser.parse_args()
    root = Path(args.runtime_root)
    if os.geteuid() != 0:
        raise PermissionError("root-owned CI runtime staging required")
    if root != Path("/opt/lu-ide/runtimes/uv_sync_offline"):
        raise ValueError("only fixed installer-style sandbox runtime path")
    if root.exists():
        raise FileExistsError("refuse to overwrite a preexisting runtime pin")
    (root / "bin").mkdir(parents=True, mode=0o755)
    (root / "LICENSES").mkdir(mode=0o755)
    for source, destination, executable in (
        (args.uv_exe, root / "bin/uv", True),
        (args.python_exe, root / "bin/python3", True),
        (args.uv_license, root / "LICENSES/uv.txt", False),
        (args.python_license, root / "LICENSES/python.txt", False),
    ):
        shutil.copyfile(require_regular_file(Path(source)), destination)
        destination.chmod(0o755 if executable else 0o644)
    # No unexpected OS package, user-controlled wheelhouse or cached grants
    # enter this pin. Original licensing remains a separate external review.
    sha, count = digest_tree(root)
    print("IDE055_RUNTIME_SCOPE=CI_ONLY_SELF_MEASURED_NOT_RELEASE_APPROVED")
    print("IDE055_RUNTIME_FILES=" + str(count))
    print("LU_IDE05_LINUX_UV_SHA256=" + sha)
    print(json.dumps({
        "schema": "ide055-ci-runtime-candidate-v1",
        "bundleSha256": sha,
        "fileCount": count,
        "releaseApproved": False,
        "humanApproved": False,
        "licenseProvenanceApproved": False,
    }, separators=(",", ":")))


if __name__ == "__main__":
    main()
