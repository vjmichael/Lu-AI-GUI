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
import base64
import io
import csv
import zipfile


def digest_tree(root: Path) -> tuple[str, int]:
    # Keep the Python staging preflight aligned with the native Rust verifier.
    # An unsafe root could previously produce a fingerprint that native code
    # would correctly refuse, hiding the actual packaging error.
    root_meta = root.lstat()
    if (not stat.S_ISDIR(root_meta.st_mode)
            or root_meta.st_uid != 0
            or root_meta.st_mode & 0o022
            or root_meta.st_nlink == 0
            or root.resolve(strict=True) != root):
        raise ValueError("untrusted native runtime root")
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



def stage_fixed_ci_wheel(wheelhouse: Path) -> str:
    """Deterministic synthetic wheel; CI source evidence only, NOT an upstream release."""
    wheelhouse.mkdir(mode=0o755)
    prefix = "ide05_demo-1.0.0.dist-info"
    members = {
        "ide05_demo/__init__.py": b"VALUE = 42\n",
        prefix + "/WHEEL": (
            b"Wheel-Version: 1.0\nGenerator: lu-ide-ci-fixed\n"
            b"Root-Is-Purelib: true\nTag: py3-none-any\n"
        ),
        prefix + "/METADATA": (
            b"Metadata-Version: 2.4\nName: ide05-demo\nVersion: 1.0.0\n"
            b"License-Expression: MIT\nLicense-File: LICENSE.txt\n"
        ),
        prefix + "/licenses/LICENSE.txt": (
            b"MIT License\nCI-only first-party synthetic fixture; no upstream claim.\n"
        ),
    }
    records = []
    for relative, content in sorted(members.items()):
        sha = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode("ascii")
        records.append([relative, "sha256=" + sha, str(len(content))])
    records.append([prefix + "/RECORD", "", ""])
    record_bytes = io.StringIO(newline="")
    csv.writer(record_bytes, lineterminator="\n").writerows(records)
    members[prefix + "/RECORD"] = record_bytes.getvalue().encode("utf-8")
    archive = wheelhouse / "ide05_demo-1.0.0-py3-none-any.whl"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED) as output:
        for relative, content in sorted(members.items()):
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = (0o100644 << 16)
            output.writestr(info, content)
    archive.chmod(0o644)
    return hashlib.sha256(archive.read_bytes()).hexdigest()

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
    # Python's implicit parents=True directory creation uses a different mode
    # from the final component. The Rust native verifier rejects a writable
    # runtime *root*, not just writable files underneath it.
    root.mkdir(parents=True, mode=0o755)
    root.chmod(0o755)
    (root / "bin").mkdir(mode=0o755)
    (root / "LICENSES").mkdir(mode=0o755)
    if root.lstat().st_uid != 0 or (root.lstat().st_mode & 0o022):
        raise PermissionError("CI native runtime root must be root-owned and nonwritable")
    for source, destination, executable in (
        (args.uv_exe, root / "bin/uv", True),
        (args.python_exe, root / "bin/python3", True),
        (args.uv_license, root / "LICENSES/uv.txt", False),
        (args.python_license, root / "LICENSES/python.txt", False),
    ):
        shutil.copyfile(require_regular_file(Path(source)), destination)
        destination.chmod(0o755 if executable else 0o644)
    # This wheel is CI-constructed deterministic first-party test data. Its
    # entire byte tree is covered by the same root-owned native runtime pin.
    wheel_sha = stage_fixed_ci_wheel(root / "wheelhouse")
    # No project-provided archives, PATH tools or preauthorized grants.
    # Real upstream license/source validation is still a separate gate.
    sha, count = digest_tree(root)
    print("IDE055_RUNTIME_SCOPE=CI_ONLY_SELF_MEASURED_NOT_RELEASE_APPROVED")
    print("IDE055_RUNTIME_FILES=" + str(count))
    print("LU_IDE05_LINUX_UV_SHA256=" + sha)
    print("LU_IDE05_CI_WHEEL_SHA256=" + wheel_sha)
    print(json.dumps({
        "schema": "ide055-ci-runtime-candidate-v1",
        "bundleSha256": sha,
        "ciWheelSha256": wheel_sha,
        "fileCount": count,
        "releaseApproved": False,
        "humanApproved": False,
        "licenseProvenanceApproved": False,
    }, separators=(",", ":")))


if __name__ == "__main__":
    main()
