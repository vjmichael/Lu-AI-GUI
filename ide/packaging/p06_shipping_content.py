#!/usr/bin/env python3
"""P06-R2 read-only hash inventory for a deliberately supplied payload tree.

Inventory is NOT proof that files actually ship: a native installed/package
witness must independently identify the correct exact artifact and root.
This script NEVER authorizes distribution, signs files, runs executables,
reads paths outside the chosen tree, or asserts legal permission.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

SCHEMA = "lu.ide.p06.shipping-content-inventory.v1"
SHA40 = re.compile(r"[0-9a-f]{40}\Z")
SHA64 = re.compile(r"[0-9a-f]{64}\Z")
MAX_ITEMS = 20000
MAX_BYTES = 4 * 1024 * 1024 * 1024
MAX_FILE = 1024 * 1024 * 1024
MAX_DEPTH = 24
MAX_REL = 512
LEGAL_DIRS = frozenset({
    "third_party_licenses", "verification-notices",
    "third-party-notices", "licenses", "license",
})
BINARY_SUFFIXES = frozenset({
    ".exe", ".dll", ".so", ".dylib", ".node", ".pyd", ".a",
})


class Denied(ValueError):
    pass


def exact_sha(value):
    if not isinstance(value, str) or not SHA40.fullmatch(value):
        raise Denied("invalid_exact_source_sha")
    return value


def _lstat(path):
    try:
        info = path.lstat()
    except OSError as exc:
        raise Denied("unreadable_tree_entry") from exc
    if stat.S_ISLNK(info.st_mode) or (
        getattr(info, "st_file_attributes", 0)
        & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)
    ):
        raise Denied("symlink_or_reparse_entry_denied")
    return info


def _safe_relative(rel):
    if not isinstance(rel, str) or not rel or len(rel) > MAX_REL or (
        "\\" in rel or "\x00" in rel or
        any(ord(c) < 32 or ord(c) == 127 for c in rel)
    ):
        raise Denied("invalid_payload_relative_path")
    parts = rel.split("/")
    if any(not part or part in (".", "..") for part in parts):
        raise Denied("invalid_payload_relative_path")
    if len(parts) > MAX_DEPTH:
        raise Denied("excessive_payload_depth")
    return rel


def _hash_regular(path, expected):
    # Empty files are valid package members and must remain in the inventory:
    # ignoring them would miss deletions, additions and packaging drift.
    if not stat.S_ISREG(expected.st_mode) or expected.st_size < 0 or (
        expected.st_size > MAX_FILE
    ):
        raise Denied("unsafe_payload_file")
    # O_NOFOLLOW handles leaf symlinks on Linux; lstat + reparse check is
    # additionally required for Windows. Neither substitutes for a secured
    # OS-created immutable installation snapshot under hostile concurrency.
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)
    try:
        fd = os.open(path, flags)
        with os.fdopen(fd, "rb") as stream:
            start = os.fstat(stream.fileno())
            if (not stat.S_ISREG(start.st_mode) or
                start.st_size != expected.st_size or
                (start.st_dev, start.st_ino) !=
                (expected.st_dev, expected.st_ino)):
                raise Denied("payload_mutated_before_read")
            digest = hashlib.sha256()
            size = 0
            magic = b""
            while True:
                chunk = stream.read(1024 * 1024)
                if not chunk:
                    break
                if not magic:
                    magic = chunk[:4]
                size += len(chunk)
                if size > MAX_FILE:
                    raise Denied("payload_file_too_large")
                digest.update(chunk)
            end = os.fstat(stream.fileno())
    except OSError as exc:
        raise Denied("payload_read_denied") from exc
    if (size != expected.st_size or
        (start.st_size, start.st_mtime_ns, start.st_dev, start.st_ino) !=
        (end.st_size, end.st_mtime_ns, end.st_dev, end.st_ino)):
        raise Denied("payload_modified_during_hash")
    return digest.hexdigest(), size, magic


def _category(rel, magic, info):
    parts = rel.casefold().split("/")
    if any(part in LEGAL_DIRS for part in parts[:-1]) or (
        parts[-1].startswith(("license", "copying", "notice"))
    ):
        return "legal_evidence_candidate"
    name = parts[-1]
    if (Path(name).suffix in BINARY_SUFFIXES or
        ".so." in name or magic in (b"\x7fELF", b"\0asm") or
        magic[:2] == b"MZ" or
        (info.st_mode & 0o111 and not name.endswith((".sh", ".py")))):
        return "native_binary_candidate"
    return "other_payload_file"


def capture(root, source_sha, platform, tree_kind):
    source_sha = exact_sha(source_sha)
    if platform not in ("windows-2022", "ubuntu-24.04", "ubuntu-26.04"):
        raise Denied("unsupported_native_witness_platform")
    if tree_kind not in ("installed_candidate", "unpacked_package_candidate"):
        raise Denied("unsupported_payload_observation")
    base = Path(root).absolute()
    if not stat.S_ISDIR(_lstat(base).st_mode):
        raise Denied("payload_root_not_directory")
    records = []
    folded = set()
    total = 0
    for current, dirs, files in os.walk(base, topdown=True, followlinks=False):
        prefix = Path(current).relative_to(base)
        _lstat(Path(current))
        for name in sorted(dirs):
            p = Path(current) / name
            rel = _safe_relative((prefix / name).as_posix())
            info = _lstat(p)
            if not stat.S_ISDIR(info.st_mode):
                raise Denied("unsafe_payload_directory")
            k = rel.casefold()
            if k in folded:
                raise Denied("cross_platform_case_collision")
            folded.add(k)
        for name in sorted(files):
            p = Path(current) / name
            rel = _safe_relative((prefix / name).as_posix())
            k = rel.casefold()
            if k in folded:
                raise Denied("cross_platform_case_collision")
            folded.add(k)
            info = _lstat(p)
            digest, size, magic = _hash_regular(p, info)
            total += size
            if total > MAX_BYTES or len(records) >= MAX_ITEMS:
                raise Denied("payload_tree_limits_exceeded")
            records.append({
                "path": rel, "sha256": digest,
                "bytes": size, "category": _category(rel, magic, info),
            })
    if not records:
        raise Denied("empty_payload_tree")
    records.sort(key=lambda row: row["path"].encode("utf-8"))
    canonical = json.dumps(records, sort_keys=True,
                           separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    counts = {
        kind: sum(x["category"] == kind for x in records)
        for kind in ("native_binary_candidate", "legal_evidence_candidate",
                     "other_payload_file")
    }
    return {
        "schema": SCHEMA, "sourceSha": source_sha,
        "platformClaim": platform, "treeKindClaim": tree_kind,
        "treeTrust": "unattested_input_tree_native_witness_required",
        "hashMethod": "sha256_canonical_relative_file_manifest_v1",
        "contentTreeSha256": hashlib.sha256(canonical).hexdigest(),
        "fileCount": len(records), "totalBytes": total,
        "categories": counts, "files": records,
        "nativeInstalledVerified": False, "packageOriginVerified": False,
        "dependencyShipmentVerified": False,
        "licenseLegalReviewApproved": False,
        "publisherReviewDeferredUntilFinalShipSet": True,
        "signingIdentityReviewed": False,
        "protectedExecutionAccepted": False, "releaseApproved": False,
    }


def verify(root, claimed, source_sha, platform, tree_kind):
    if not isinstance(claimed, dict) or claimed.get("schema") != SCHEMA:
        raise Denied("invalid_shipment_snapshot_schema")
    actual = capture(root, source_sha, platform, tree_kind)
    if claimed != actual:
        raise Denied("shipment_snapshot_differs_from_actual_tree")
    return {
        "contentTreeSha256": actual["contentTreeSha256"],
        "fileCount": actual["fileCount"], "bytesRechecked": actual["totalBytes"],
        "sourceSha": actual["sourceSha"], "sourceTreeMatches": True,
        "nativeInstalledVerified": False, "releaseApproved": False,
        "licenseLegalReviewApproved": False,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", type=Path, required=True)
    ap.add_argument("--source-sha", required=True)
    ap.add_argument("--platform", required=True,
                    choices=["windows-2022", "ubuntu-24.04", "ubuntu-26.04"])
    ap.add_argument("--tree-kind", required=True,
                    choices=["installed_candidate", "unpacked_package_candidate"])
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--verify", action="store_true")
    args = ap.parse_args()
    try:
        base = args.root.absolute().resolve(strict=True)
        out = args.output.absolute().resolve(strict=False)
        if out == base or base in out.parents:
            raise Denied("receipt_cannot_be_inside_observed_payload")
        if args.verify:
            claim = json.loads(args.output.read_text(encoding="utf-8"))
            result = verify(base, claim, args.source_sha, args.platform, args.tree_kind)
            print(json.dumps(result, sort_keys=True))
        else:
            if args.output.exists() or args.output.is_symlink():
                raise Denied("receipt_must_be_new")
            result = capture(base, args.source_sha, args.platform, args.tree_kind)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            with args.output.open("x", encoding="utf-8") as stream:
                json.dump(result, stream, sort_keys=True, indent=2)
                stream.write("\n")
            print("P06_R2_PAYLOAD_HASH_SNAPSHOT=CREATED_NOT_INSTALLED_ATTESTATION")
            print("P06_R2_NATIVE_AND_LEGAL_RELEASE=NOT_AUTHORIZED")
    except (OSError, ValueError, TypeError, KeyError) as exc:
        print("P06_R2_SHIPMENT_EVIDENCE=DENIED:" + (
            str(exc) if isinstance(exc, Denied) else "invalid_observation"
        ), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
