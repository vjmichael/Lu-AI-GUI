#!/usr/bin/env python3
"""Prepare a fail-closed *review packet* for Lu IDE's native LSP binaries.

This never approves or stages a release. Source is an operator-provided
offline bundle; checksummed binary/notice records are merely evidence for
a later independent redistributor/legal/signature review.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import sys

SCHEMA = "lu.ide.e2.lsp-offline-intake.v1"
IDS = {
    "clangd": {"windows-x86_64": "clangd.exe", "linux-x86_64": "clangd"},
    "rust-analyzer": {
        "windows-x86_64": "rust-analyzer.exe",
        "linux-x86_64": "rust-analyzer",
    },
    "ruff": {"windows-x86_64": "ruff.exe", "linux-x86_64": "ruff"},
}
MAX_BINARY_BYTES = 1024 * 1024 * 1024
MAX_NOTICE_BYTES = 1024 * 1024


def safe_file(root: Path, relative: str, maximum: int) -> Path:
    if not isinstance(relative, str) or not relative or len(relative) > 1024 or (
        "\\" in relative or ":" in relative or relative.startswith("/")
    ):
        raise ValueError("invalid candidate relative asset path")
    parts = PurePosixPath(relative).parts
    if not parts or any(
        p in (".", "..", "") or len(p) > 240 or
        not re.fullmatch(r"[A-Za-z0-9._+-]+", p) for p in parts
    ):
        raise ValueError("candidate relative asset path unsafe")
    path = root
    for item in parts:
        path = path / item
        if path.is_symlink():
            raise ValueError("candidate asset uses a symlink")
    if not path.is_file() or not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("candidate asset missing or escapes root")
    length = path.stat().st_size
    if length < 1 or length > maximum:
        raise ValueError("candidate asset has unacceptable size")
    return path


def digest(path: Path) -> str:
    total = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            total.update(chunk)
    return total.hexdigest()


def review_packet(bundle_root: Path, proposal: dict) -> dict:
    if not bundle_root.is_dir() or bundle_root.is_symlink():
        raise ValueError("offline candidate asset root must be a real directory")
    if not isinstance(proposal, dict) or proposal.get("schema") != SCHEMA:
        raise ValueError("wrong offline candidate proposal schema")
    items = proposal.get("servers")
    if not isinstance(items, list) or len(items) != 3:
        raise ValueError("exactly three native providers required")
    received = {}
    for row in items:
        if not isinstance(row, dict) or set(row) != {
            "id", "releaseVersion", "assetByPlatform", "licenseNoticeFiles"
        }:
            raise ValueError("unknown or missing provider proposal fields")
        ident = row["id"]
        if ident not in IDS or ident in received:
            raise ValueError("unapproved, duplicate or missing provider ID")
        version = row["releaseVersion"]
        if not isinstance(version, str) or not re.fullmatch(
            r"[A-Za-z0-9._+-]{1,90}", version
        ):
            raise ValueError("provider version must be an exact upstream ID")
        paths = row["assetByPlatform"]
        notices = row["licenseNoticeFiles"]
        if not isinstance(paths, dict) or set(paths) != set(IDS[ident]):
            raise ValueError("both native OS binary assets are required")
        if not isinstance(notices, list) or not 1 <= len(notices) <= 32 or (
            len(notices) != len(set(notices))
        ):
            raise ValueError("original upstream license notice inventory required")
        observed = {}
        for platform, filename in IDS[ident].items():
            asset = paths[platform]
            if not isinstance(asset, str) or PurePosixPath(asset).name != filename:
                raise ValueError("unreviewed executable asset filename")
            src = safe_file(bundle_root, asset, MAX_BINARY_BYTES)
            observed[platform] = {
                "relativePath": asset,
                "sha256": digest(src),
                "bytes": src.stat().st_size,
            }
        notice_records = []
        for notice in notices:
            path = safe_file(bundle_root, notice, MAX_NOTICE_BYTES)
            notice_records.append({
                "relativePath": notice, "sha256": digest(path),
                "bytes": path.stat().st_size,
            })
        received[ident] = {
            "versionClaim": version,
            "platforms": observed,
            "originalNoticeCandidates": notice_records,
            "copyrightReviewed": False,
            "dependenciesAudited": False,
            "licenseApproved": False,
            "signatureApproved": False,
        }
    if set(received) != set(IDS):
        raise ValueError("the Rust, clangd, Ruff provider bundle is incomplete")
    return {
        "schema": "lu.ide.e2.lsp-approval-review.v1",
        "releaseApproved": False,
        "autoInstallOnProjectOpen": False,
        "allowUnpinnedExecutables": False,
        "osContainmentVerified": False,
        "distributorAuthorizationConfirmed": False,
        "servers": received,
        "nextAction": "Independent origin, dependency, copyright, license and signature review; do not stage binaries from this evidence alone",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bundle-root", type=Path, required=True)
    parser.add_argument("--candidate-spec", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    options = parser.parse_args()
    try:
        if options.out.exists() or options.out.is_symlink():
            raise ValueError("output must not already exist")
        proposal = json.loads(options.candidate_spec.read_text(encoding="utf-8"))
        result = review_packet(options.bundle_root, proposal)
        options.out.parent.mkdir(parents=True, exist_ok=True)
        with options.out.open("x", encoding="utf-8") as writer:
            json.dump(result, writer, indent=2, sort_keys=True)
            writer.write("\n")
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        print("DENIED: LSP review intake: " + str(error), file=sys.stderr)
        return 2
    print("LSP_CANDIDATE_REVIEW_PACKET_CREATED_NO_RELEASE_AUTHORITY=PASS")
    print("LSP_RELEASE_APPROVED=NO")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
