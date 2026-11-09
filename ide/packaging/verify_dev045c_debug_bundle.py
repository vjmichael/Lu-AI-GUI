#!/usr/bin/env python3
"""DEV-04.5C: independently audit a candidate's *installed* project LLDB bytes.

The pinned lock originates in a separately reviewed signed WO7 candidate.
This checks candidate integrity ONLY. The native host must still prove its own
compile-time tree pin, OS containment, and fresh consent. No release approval.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys

SCHEMA = "lu.ide.dev045c.installed-project-debug-bundle.v1"
SCOPE = "technical-installed-test-only"
BUNDLE = Path("resources/debug-adapters/lldb-dap")
MAX_FILES = 512
MAX_FILE_BYTES = 512 * 1024 * 1024
MAX_TOTAL_BYTES = 1024 * 1024 * 1024
SHA40 = re.compile(r"[0-9a-f]{40}\Z")
SHA64 = re.compile(r"[0-9a-f]{64}\Z")


class Denied(ValueError):
    """Never turn a malformed candidate into installed execution authority."""


def sha256(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as reader:
        for block in iter(lambda: reader.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def safe_relative(raw: object) -> str:
    if not isinstance(raw, str) or len(raw) > 240 or "\\" in raw or ":" in raw:
        raise Denied("invalid_candidate_relative_path")
    parts = raw.split("/")
    if (not parts or any(part in ("", ".", "..") for part in parts)
            or any(part.strip(" .") != part for part in parts)
            or any(any(ord(char) < 32 for char in part) for part in parts)):
        raise Denied("unsafe_candidate_path")
    path = PurePosixPath(raw)
    if path.is_absolute() or path.as_posix() != raw:
        raise Denied("noncanonical_candidate_path")
    return raw


def normal_file(path: Path, outer: Path) -> os.stat_result:
    """Reject symlinks, reparse points and linked executables at every level."""
    if not path.is_relative_to(outer):
        raise Denied("asset_outside_installed_root")
    cursor = path
    while True:
        try:
            meta = cursor.lstat()
        except OSError as exc:
            raise Denied("missing_installed_asset") from exc
        if (stat.S_ISLNK(meta.st_mode)
                or getattr(meta, "st_file_attributes", 0)
                & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
            raise Denied("linked_or_reparsed_installed_asset")
        if cursor == outer:
            break
        cursor = cursor.parent
    leaf = path.stat()
    if not stat.S_ISREG(leaf.st_mode) or leaf.st_nlink != 1:
        raise Denied("nonregular_or_hardlinked_installed_asset")
    if not 0 < leaf.st_size <= MAX_FILE_BYTES:
        raise Denied("unbounded_installed_asset")
    return leaf


def audit(root: Path, lock: dict, source_sha: str, *, platform: str) -> dict:
    if (not SHA40.fullmatch(source_sha)
            or not isinstance(lock, dict)
            or lock.get("schema") != SCHEMA
            or lock.get("sourceSha") != source_sha
            or lock.get("scope") != SCOPE
            or lock.get("releaseApproved") is not False
            or lock.get("redistributionApproved") is not False
            or lock.get("userSelectedExecutable") is not False
            or lock.get("platform") != platform
            or platform not in ("windows-x86_64", "linux-x86_64")):
        raise Denied("wrong_source_scope_or_unapproved_candidate")
    if not root.is_absolute() or not root.is_dir() or root.is_symlink():
        raise Denied("not_an_installed_absolute_root")
    bundle = root / BUNDLE
    if not bundle.is_dir() or bundle.is_symlink():
        raise Denied("debugger_bundle_missing_or_linked")
    executable = ("bin/lldb-dap.exe" if platform == "windows-x86_64"
                  else "bin/lldb-dap")
    debuggee = ("debuggee/wo6-selftest.exe" if platform == "windows-x86_64"
                else "debuggee/wo6-selftest")
    rows = lock.get("files")
    if not isinstance(rows, list) or not 2 <= len(rows) <= MAX_FILES:
        raise Denied("invalid_debugger_inventory")
    seen: set[str] = set()
    total = 0
    expected: set[str] = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "bytes"}:
            raise Denied("invalid_file_record")
        rel = safe_relative(row["path"])
        key = rel.casefold()
        if key in seen:
            raise Denied("duplicate_or_case_colliding_asset")
        seen.add(key)
        expected.add(rel)
        if (not isinstance(row["sha256"], str)
                or SHA64.fullmatch(row["sha256"]) is None
                or type(row["bytes"]) is not int
                or not 0 < row["bytes"] <= MAX_FILE_BYTES):
            raise Denied("missing_exact_candidate_file_pin")
        allowed = rel in (executable, debuggee)
        if platform == "windows-x86_64":
            allowed |= rel.startswith("bin/") and rel.endswith(".dll") and rel.count("/") == 1
        else:
            allowed |= (rel.startswith("bin/") and rel.endswith(".so")
                        and rel.count("/") == 1)
        if not allowed:
            raise Denied("unreviewed_debugger_payload_type")
        path = bundle.joinpath(*rel.split("/"))
        observed = normal_file(path, root)
        if observed.st_size != row["bytes"] or sha256(path) != row["sha256"]:
            raise Denied("installed_candidate_file_changed")
        total += observed.st_size
        if total > MAX_TOTAL_BYTES:
            raise Denied("debugger_total_bytes_unbounded")
    if executable not in expected or debuggee not in expected:
        raise Denied("real_adapter_and_fixed_probe_required")
    actual: set[str] = set()
    for directory, dirs, files in os.walk(bundle, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            if path.is_symlink() or (getattr(path.lstat(), "st_file_attributes", 0)
                                     & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
                raise Denied("linked_or_reparsed_bundle_member")
        for name in files:
            rel = (Path(directory) / name).relative_to(bundle).as_posix()
            actual.add(rel)
            if len(actual) > MAX_FILES:
                raise Denied("unbounded_bundle_inventory")
    if actual != expected:
        raise Denied("extra_or_missing_debugger_runtime_files")
    # An upstream notice is a *separate*, independently hash-pinned file;
    # the candidate cannot satisfy it with a string saying "licenses reviewed".
    notice = lock.get("notice")
    if not isinstance(notice, dict) or set(notice) != {"path", "sha256", "bytes"}:
        raise Denied("debugger_original_notice_not_pinned")
    notice_rel = safe_relative(notice["path"])
    if not notice_rel.startswith("THIRD_PARTY_LICENSES/"):
        raise Denied("debugger_notice_outside_licenses")
    if (not isinstance(notice["sha256"], str)
            or SHA64.fullmatch(notice["sha256"]) is None
            or type(notice["bytes"]) is not int
            or not 0 < notice["bytes"] <= MAX_FILE_BYTES):
        raise Denied("invalid_debugger_notice_pin")
    notice_file = root.joinpath(*notice_rel.split("/"))
    notice_size = normal_file(notice_file, root).st_size
    if notice_size != notice["bytes"] or sha256(notice_file) != notice["sha256"]:
        raise Denied("changed_or_missing_debugger_notice")
    return {
        "schema": "lu.ide.dev045c.bundle-integrity-observation.v1",
        "sourceSha": source_sha, "platform": platform,
        "filesVerified": len(expected), "bytesVerified": total,
        "noticeVerified": True, "candidateIntegrityVerified": True,
        "nativeHostTreePinVerified": False, "osContainmentVerified": False,
        "signedCandidateVerifiedSeparately": False,
        "installedProjectDebuggerAccepted": False,
        "releaseApproved": False
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--installed-root", type=Path, required=True)
    parser.add_argument("--lock", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--platform", choices=["windows-x86_64", "linux-x86_64"], required=True)
    a = parser.parse_args()
    try:
        observed = audit(a.installed_root, json.loads(a.lock.read_text(encoding="utf-8")),
                         a.source_sha, platform=a.platform)
    except (Denied, OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        reason = str(exc) if isinstance(exc, Denied) else "invalid_installed_debugger_candidate"
        print("DENIED: DEV045C_" + reason, file=sys.stderr)
        return 1
    print(json.dumps(observed, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
