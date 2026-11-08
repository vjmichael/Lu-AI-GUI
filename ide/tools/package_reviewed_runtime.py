#!/usr/bin/env python3
"""DEV-03.2B.2 offline bundle builder (build-machine tool, not a Lu IDE command).

Never downloads, installs, executes or trusts any staged runtime. Input trees
must come from a separately reviewed supply-chain workflow. The generated
candidate is NOT a release approval; approved digest must be provided during
the reproducible verified build and compiled into the native host.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import zipfile

DOMAIN = b"lu-ide-runtime-tree-v1\0"
MAX_COUNT = 40_000
MAX_TOTAL = 4 * 1024**3
MAX_FILE = 1024**3
MAX_PATH = 1024
MAX_DEPTH = 32
REQUIRED = {
    ("python_pytest", "windows-x64"): (
        "bin/python.exe", "Lib/pytest/__init__.py",
        "LICENSES/python.txt", "LICENSES/pytest.txt"),
    ("python_pytest", "linux-x64"): (
        "bin/python3", "lib/pytest/__init__.py",
        "LICENSES/python.txt", "LICENSES/pytest.txt"),
    ("js_tests", "windows-x64"): ("bin/node.exe", "LICENSES/node.txt"),
    ("js_tests", "linux-x64"): ("bin/node", "LICENSES/node.txt"),
    ("cpp_cmake_build", "windows-x64"): (
        "bin/cmake.exe", "bin/ctest.exe", "bin/ninja.exe", "bin/clang.exe",
        "bin/clang++.exe", "LICENSES/cmake.txt", "LICENSES/ninja.txt",
        "LICENSES/llvm.txt", "drivers/dev03-cmake.cmake"),
    ("cpp_cmake_build", "linux-x64"): (
        "bin/cmake", "bin/ctest", "bin/ninja", "bin/clang", "bin/clang++",
        "LICENSES/cmake.txt", "LICENSES/ninja.txt", "LICENSES/llvm.txt", "drivers/dev03-cmake.cmake"),
}


class Refused(ValueError):
    """No artifact is emitted after a provenance or content mismatch."""


def safe_part(part: str) -> bool:
    if not part or part in {".", ".."} or len(part.encode("utf-8")) > 255:
        return False
    if part.endswith(".") or part.endswith(" "):
        return False
    return not any(ord(c) < 32 or ord(c) == 127 or c in r'\/:|' for c in part)


def stable_files(root: Path) -> tuple[list[tuple[str, Path, int, str]], str, int]:
    root = root.resolve(strict=True)
    if not root.is_dir() or root.is_symlink():
        raise Refused("runtime root is not a regular directory")
    files: list[tuple[str, Path, int, str]] = []
    total = 0
    stack = [(root, 0)]
    while stack:
        current, depth = stack.pop()
        if depth > MAX_DEPTH:
            raise Refused("runtime tree too deep")
        for path in sorted(current.iterdir(), key=lambda p: p.name):
            rel = path.relative_to(root)
            parts = rel.parts
            if not all(safe_part(p) for p in parts) or len(rel.as_posix().encode()) > MAX_PATH:
                raise Refused("unsafe runtime path")
            before = path.lstat()
            if stat.S_ISLNK(before.st_mode):
                raise Refused("runtime symlink forbidden")
            if os.name == "nt" and getattr(before, "st_file_attributes", 0) & 0x400:
                raise Refused("Windows reparse point forbidden")
            if stat.S_ISDIR(before.st_mode):
                stack.append((path, depth + 1))
                continue
            if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
                raise Refused("special file or hard link forbidden")
            if before.st_size > MAX_FILE:
                raise Refused("single file budget exceeded")
            total += before.st_size
            if total > MAX_TOTAL or len(files) >= MAX_COUNT:
                raise Refused("runtime tree budget exceeded")
            sha = hashlib.sha256()
            size = 0
            with path.open("rb") as stream:
                while chunk := stream.read(65536):
                    size += len(chunk)
                    if size > before.st_size:
                        raise Refused("file changed while hashing")
                    sha.update(chunk)
                after_fd = os.fstat(stream.fileno())
            after = path.lstat()
            if (size != before.st_size or after.st_size != before.st_size
                    or after.st_mtime_ns != before.st_mtime_ns
                    or after.st_ino != before.st_ino
                    or after_fd.st_ino != before.st_ino):
                raise Refused("file changed while hashing")
            files.append((rel.as_posix(), path, size, sha.hexdigest()))
    files.sort(key=lambda x: x[0].encode("utf-8"))
    digest = hashlib.sha256(DOMAIN)
    for name, _path, size, sha in files:
        digest.update(name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(size.to_bytes(8, "little"))
        digest.update(sha.encode("ascii"))
        digest.update(b"\0")
    return files, digest.hexdigest(), total


def approved_sha(value: str) -> str:
    if len(value) != 64 or value.lower() != value or set(value) - set("0123456789abcdef"):
        raise Refused("expected pin must be lowercase SHA-256")
    if value == "0" * 64:
        raise Refused("zero pin is never approved")
    return value


def package(root: Path, output: Path, profile: str, platform: str,
            expected: str | None, candidate: bool) -> dict:
    if (profile, platform) not in REQUIRED:
        raise Refused("unreviewed runtime profile/platform")
    if not candidate and not expected:
        raise Refused("approved tree pin required; candidate must be explicit")
    if candidate and expected:
        raise Refused("choose candidate OR pinned verification")
    files, actual, total = stable_files(root)
    names = {name for name, *_ in files}
    missing = set(REQUIRED[(profile, platform)]) - names
    if missing:
        raise Refused("required binaries/licenses missing: " + ", ".join(sorted(missing)))
    if expected is not None and approved_sha(expected) != actual:
        raise Refused("bundle SHA mismatch")
    metadata = {
        "formatVersion": 1, "profileId": profile, "platform": platform,
        "runtimeTreeSha256": actual, "fileCount": len(files),
        "totalBytes": total,
        "approval": "candidate_not_approved" if candidate else "pin_matched_not_os_accepted",
        "runtimeExecutionEnabled": False,
        "installedAppAccepted": False,
        "requiredFiles": list(REQUIRED[(profile, platform)]),
        "digestDomain": "lu-ide-runtime-tree-v1",
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    temp = output.with_suffix(output.suffix + ".partial")
    if temp.exists():
        raise Refused("refusing to overwrite prior partial bundle")
    try:
        with zipfile.ZipFile(temp, "w", compression=zipfile.ZIP_DEFLATED,
                             allowZip64=True, compresslevel=6) as archive:
            manifest = json.dumps(metadata, sort_keys=True, separators=(",", ":")).encode()
            entry = zipfile.ZipInfo("runtime-manifest.json", (1980, 1, 1, 0, 0, 0))
            entry.external_attr = (0o100644 << 16)
            archive.writestr(entry, manifest)
            for name, path, size, before_hash in files:
                entry = zipfile.ZipInfo("runtime/" + name, (1980, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = ((0o100755 if os.access(path, os.X_OK) else 0o100644) << 16)
                with path.open("rb") as file, archive.open(entry, "w") as target:
                    sha = hashlib.sha256()
                    length = 0
                    while chunk := file.read(65536):
                        sha.update(chunk)
                        length += len(chunk)
                        if length > size:
                            raise Refused("runtime source changed during packaging")
                        target.write(chunk)
                    if length != size or sha.hexdigest() != before_hash:
                        raise Refused("runtime source changed during packaging")
        if output.exists():
            raise Refused("refusing to overwrite an existing bundle")
        temp.rename(output)
    finally:
        if temp.exists():
            temp.unlink()
    return metadata


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, choices=sorted({p for p,_ in REQUIRED}))
    parser.add_argument("--platform", required=True, choices=["linux-x64","windows-x64"])
    parser.add_argument("--runtime-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--expected-tree-sha256")
    mode.add_argument("--candidate", action="store_true")
    opts = parser.parse_args()
    try:
        result = package(opts.runtime_root, opts.output, opts.profile, opts.platform,
                         opts.expected_tree_sha256, opts.candidate)
    except (Refused, OSError, zipfile.BadZipFile) as exc:
        print(f"RUNTIME_PACKAGE_DENIED: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
