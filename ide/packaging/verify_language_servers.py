#!/usr/bin/env python3
"""Fail-closed Lu IDE language-server redistribution audit.

CI may inspect the *planned* manifest without shipping binaries.
Release mode refuses any unpinned candidate, missing license/notice copy,
unreviewed transitive dependency, modified binary, or network download.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import sys

SCHEMA = "lu.ide.lsp.bundle.v1"
PLATFORMS = {"windows-x86_64", "linux-x86_64"}
HEX = re.compile(r"^[a-f0-9]{64}$")


def safe_file(root: pathlib.Path, relative: str) -> pathlib.Path:
    if not relative or relative.startswith(("/", "\\")):
        raise ValueError("absolute/empty path")
    if "\\" in relative or any(part in {"", ".", ".."} for part in relative.split("/")):
        raise ValueError("path escape")
    item = pathlib.PurePosixPath(relative)
    if root.is_symlink():
        raise ValueError("linked resource root")
    result = root
    for part in item.parts:
        result = result / part
        if result.is_symlink():
            raise ValueError("symlink in original resource path")
    if not result.is_file():
        raise ValueError("missing original bundle asset")
    if not result.resolve().is_relative_to(root.resolve()):
        raise ValueError("bundle path escaped root")
    return result




def reviewed_native_resources(row: dict, root: pathlib.Path, platform: str):
    """Release-only extra ORIGINAL resources in fixed worker-relative layout.

    A binary-only clangd release is NOT a complete native payload. Independent
    reviewer populates this immutable manifest after full upstream archive and
    license/dependency inventory. The test-only original archive receipt is
    not a substitute for signoff.
    """
    if platform not in PLATFORMS:
        raise ValueError("unknown native resource platform")
    declared = row.get("nativeResourceFilesByPlatform")
    approved = row.get("nativeResourceManifestReviewed")
    if row.get("category") != "native":
        if declared or approved:
            raise ValueError("node LSP cannot impersonate native resource manifest")
        return []
    if not approved or not isinstance(declared, dict) or set(declared) != PLATFORMS:
        raise ValueError("complete independently reviewed native resource manifest required")
    entries = declared[platform]
    if not isinstance(entries, dict) or len(entries) > 490 or (
        row.get("id") == "clangd" and not entries
    ):
        raise ValueError("clangd requires reviewed original LLVM resources and file quota")
    result = []
    seen = {"bin/" + ("clangd.exe" if platform.startswith("windows") else "clangd")
            if row.get("id") == "clangd" else
            "bin/" + row["id"] + (".exe" if platform.startswith("windows") else "")}
    total = 0
    for relative, item in entries.items():
        if not isinstance(relative,str) or not relative or len(relative)>1024 or (
            relative.startswith(("/", "\\")) or "\\" in relative
        ):
            raise ValueError("untrusted native resource destination")
        parts=relative.split("/")
        if (len(parts)>18 or any(not part or part in (".","..") or
            len(part)>200 or not re.fullmatch(r"[A-Za-z0-9._+-]+",part)
            for part in parts) or parts[0].casefold()=="notices"):
            raise ValueError("unsafe native original resource layout")
        folded=relative.casefold()
        if folded in seen:
            raise ValueError("duplicate/case-aliased original native resource")
        seen.add(folded)
        if not isinstance(item,dict) or set(item)!={"path","sha256"} or (
            not isinstance(item["sha256"],str) or
            not HEX.fullmatch(item["sha256"])
        ):
            raise ValueError("resource missing original source and exact SHA")
        file=safe_file(root,item["path"])
        if file.stat().st_size>256*1024*1024:
            raise ValueError("original resource exceeds protected Windows per-file quota")
        digest=hashlib.sha256()
        with file.open("rb") as stream:
            while part:=stream.read(1024*1024):
                digest.update(part)
        if digest.hexdigest()!=item["sha256"]:
            raise ValueError("original resource SHA mismatch")
        total+=file.stat().st_size
        if total>512*1024*1024:
            raise ValueError("resource set exceeds stricter protected Windows total quota")
        result.append((relative,file,item["sha256"]))
    return sorted(result)


def verify(manifest: pathlib.Path, root: pathlib.Path, release: bool = False) -> list[str]:
    data = json.loads(manifest.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA or data.get("autoInstallOnProjectOpen") is not False \
            or data.get("allowUnpinnedExecutables") is not False:
        raise ValueError("unsafe manifest policy")
    rows = data.get("servers")
    if not isinstance(rows, list) or not 1 <= len(rows) <= 20:
        raise ValueError("bad server catalog")
    if release and data.get("releaseApproved") is not True:
        raise ValueError("release has not received reviewed approval")
    seen: set[str] = set()
    included: list[str] = []
    for row in rows:
        ident = row.get("id")
        if not isinstance(ident, str) or not re.fullmatch(r"[a-z0-9-]{2,64}", ident) \
                or ident in seen:
            raise ValueError("invalid/duplicate LSP ID")
        seen.add(ident)
        mode = row.get("mode")
        if mode not in {"planned", "deferred", "bundled"}:
            raise ValueError("unrecognized bundle state")
        if row.get("category") not in {"native", "node"}:
            raise ValueError("invalid runtime category")
        if not isinstance(row.get("licenseExpression"), str) \
                or not row["licenseExpression"]:
            raise ValueError("license missing")
        if not str(row.get("noticeUpstream", "")).startswith("https://"):
            raise ValueError("source origin missing")
        if mode != "bundled":
            if row.get("assetByPlatform") or row.get("sha256ByPlatform"):
                raise ValueError("unapproved executable bytes cannot be staged")
            continue
        if not row.get("copyrightReviewed") or not row.get("dependenciesAudited") \
                or not row.get("releaseVersion"):
            raise ValueError(f"{ident}: release provenance/notice not approved")
        paths = row.get("assetByPlatform")
        digests = row.get("sha256ByPlatform")
        notices = row.get("licenseNoticeFiles")
        if not isinstance(paths, dict) or not isinstance(digests, dict) \
                or set(paths) != PLATFORMS or set(digests) != PLATFORMS \
                or not isinstance(notices, list) or not notices:
            raise ValueError(f"{ident}: complete platform asset pins and notices required")
        for platform in sorted(PLATFORMS):
            checksum = digests[platform]
            if not isinstance(checksum, str) or not HEX.fullmatch(checksum):
                raise ValueError(f"{ident}: unpinned {platform} binary")
            target = safe_file(root, paths[platform])
            if target.stat().st_size > 1024 * 1024 * 1024:
                raise ValueError(f"{ident}: unreasonably large bundle executable")
            digest = hashlib.sha256()
            with target.open("rb") as stream:
                while block := stream.read(1024 * 1024):
                    digest.update(block)
            if digest.hexdigest() != checksum:
                raise ValueError(f"{ident}: {platform} binary digest mismatch")
        # An independent approved release cannot silently discard all
        # original clangd DLLs/Clang resources. Check BOTH OS inventories.
        for native_platform in sorted(PLATFORMS):
            reviewed_native_resources(row,root,native_platform)
        for notice in notices:
            candidate = safe_file(root, notice)
            if candidate.stat().st_size > 1024 * 1024 or candidate.stat().st_size == 0:
                raise ValueError(f"{ident}: missing or oversized notice text")
        included.append(ident)
    if release and not included:
        raise ValueError("no signed/approved language server candidates to ship")
    return included


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=pathlib.Path,
                        default=pathlib.Path(__file__).parent / "language-servers.manifest.json")
    parser.add_argument("--bundle-root", type=pathlib.Path, default=pathlib.Path(__file__).parent)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    try:
        included = verify(args.manifest, args.bundle_root, args.release)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f"DENIED: language-server redistribution: {exc}", file=sys.stderr)
        return 1
    print(f"PASS: manifest safe; actual bundled providers={included}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
