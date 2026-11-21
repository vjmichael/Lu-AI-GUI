#!/usr/bin/env python3
"""Stage review-approved LSP resources for a separate elevated Lu IDE NSIS build.

Only stages byte-for-byte verified release binaries and their license notices.
No downloads, shell execution, GitHub release lookup or auto-install occurs.
The separately protected Windows SCM service must be authenticode-signed and
installed by a reviewed elevated installer step; merely bundling it does not
create the service or authorize a language-server process.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

from verify_language_servers import safe_file, verify, reviewed_native_resources


def digest_file(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            result.update(block)
    return result.hexdigest()


def windows_lsp_tree_digest(root: Path) -> str:
    """Match Windows cap-std D7B digest_tree (path, size, hex SHA, NUL).

    Only derived from an exclusively staged reviewed resource tree. This is
    not a user-supplied path authority, service grant or signing receipt.
    """
    total = hashlib.sha256()
    def visit(folder: Path) -> None:
        for child in sorted(folder.iterdir(), key=lambda path: path.name):
            if child.is_symlink():
                raise ValueError("symlink in staged Windows language-server tree")
            if child.is_dir():
                visit(child)
            elif child.is_file():
                relative = child.relative_to(root).as_posix()
                total.update(relative.encode("utf-8"))
                total.update(b"\x00")
                total.update(child.stat().st_size.to_bytes(8, "little"))
                total.update(digest_file(child).encode("ascii"))
                total.update(b"\x00")
            else:
                raise ValueError("unsafe staged native Windows bundle entry")
    visit(root)
    return total.hexdigest()


def signed_service(path: Path) -> bool:
    """A release Windows service needs OS Authenticode validation, no fixture bypass."""
    if os.name != "nt":
        return False
    try:
        completed = subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command",
             "(Get-AuthenticodeSignature -LiteralPath $args[0]).Status",
             str(path)],
            capture_output=True, text=True, timeout=20, check=True,
        )
        return completed.stdout.strip() == "Valid"
    except (OSError, subprocess.SubprocessError):
        return False


def stage(
    manifest_path: Path,
    bundle_root: Path,
    service_path: Path,
    service_sha256: str,
    destination: Path,
    *,
    verify_service_signature: bool = True,
) -> dict:
    if not re.fullmatch(r"[a-fA-F0-9]{64}", service_sha256):
        raise ValueError("reviewed service SHA-256 required")
    included = verify(manifest_path, bundle_root, release=True)
    if not included:
        raise ValueError("no approved language servers")
    if destination.exists() or destination.is_symlink():
        raise ValueError("destination must not already exist")
    if service_path.is_symlink() or not service_path.is_file():
        raise ValueError("missing or linked protected service executable")
    if digest_file(service_path).lower() != service_sha256.lower():
        raise ValueError("reviewed service executable hash mismatch")
    if verify_service_signature and not signed_service(service_path):
        raise ValueError("service Authenticode signature not independently valid")
    rows = {item["id"]: item for item in json.loads(
        manifest_path.read_text(encoding="utf-8")
    )["servers"]}
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".lsp-stage-", dir=destination.parent) as temp:
        tmp = Path(temp)
        stage_root = tmp / "resources"
        pin_record = {"schema": "lu.ide.e2.staged-release.v1",
                      "serviceSha256": service_sha256.lower(),
                      "servers": {}}
        for ident in included:
            row = rows[ident]
            # Windows first; Linux assets are independently verified by the
            # manifest verifier but not shipped into an NSIS installation.
            platform = "windows-x86_64"
            src = safe_file(bundle_root, row["assetByPlatform"][platform])
            name = {
                "clangd": "clangd.exe",
                "rust-analyzer": "rust-analyzer.exe",
                "ruff": "ruff.exe",
                "pyright": "pyright-langserver.exe",
            }.get(ident)
            if name is None:
                raise ValueError(f"unsupported executable-bearing LSP: {ident}")
            target = stage_root / "language-servers" / ident / "bin" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, target)
            expected = row["sha256ByPlatform"][platform].lower()
            if digest_file(target) != expected:
                raise ValueError("resource changed after byte-for-byte staging")
            # Preserve every independently approved original native DLL and
            # Clang resource under its exact fixed worker-relative location.
            native_resources=reviewed_native_resources(row,bundle_root,platform)
            for relative,original,expected_resource in native_resources:
                asset=target.parent.parent.joinpath(*relative.split("/"))
                asset.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(original,asset)
                if digest_file(asset)!=expected_resource:
                    raise ValueError("original native resource mutated during staging")
            notice_dir = target.parent.parent / "NOTICES"
            notice_dir.mkdir(parents=True, exist_ok=True)
            for index, path in enumerate(row["licenseNoticeFiles"]):
                notice_src = safe_file(bundle_root, path)
                notice_dst = notice_dir / f"{index:03d}-{notice_src.name}"
                shutil.copyfile(notice_src, notice_dst)
                if digest_file(notice_src) != digest_file(notice_dst):
                    raise ValueError("license notice changed during staging")
            pin_record["servers"][ident] = {
                "binarySha256": expected,
                "version": row["releaseVersion"],
                "license": row["licenseExpression"],
                "notices": len(row["licenseNoticeFiles"]),
                "originalNativeResourceFiles":len(native_resources),
                "bundleSha256": windows_lsp_tree_digest(target.parent.parent),
                "rustEnvPin": {
                    "clangd": "LU_IDE_E2_CLANGD_BUNDLE_SHA256",
                    "rust-analyzer": "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256",
                    "ruff": "LU_IDE_E2_RUFF_BUNDLE_SHA256",
                    "pyright": "LU_IDE_E2_PYRIGHT_BUNDLE_SHA256",
                }[ident],
                # Rust MUST independently remeasure the entire installed
                # tree before it consumes the one-shot protected grant.
            }
        service_target = stage_root / "recovery" / "windows_recovery_service.exe"
        service_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(service_path, service_target)
        if digest_file(service_target) != service_sha256.lower():
            raise ValueError("protected service changed during staging")
        (stage_root / "release-evidence.json").write_text(
            json.dumps(pin_record, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        os.replace(stage_root, destination)
    return pin_record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--bundle-root", type=Path, required=True)
    parser.add_argument("--service-executable", type=Path, required=True)
    parser.add_argument("--service-sha256", required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    try:
        evidence = stage(
            args.manifest, args.bundle_root, args.service_executable,
            args.service_sha256, args.destination,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"DENIED: E2 release asset staging: {error}", file=sys.stderr)
        return 1
    print("PASS: staged approved LSP resources; binaries are NOT authorized to run")
    print(json.dumps(evidence, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
