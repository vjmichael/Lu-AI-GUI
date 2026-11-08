#!/usr/bin/env python3
"""Strict offline Linux Lu IDE native-language-server resource staging.

Requires a separately owner-approved release manifest, full Windows/Linux
asset hashes, reviewed original notices, and explicit dependencies. Nothing
is downloaded or executed. Linux installer must later install these files
root-owned and non-writable; the Rust LSP worker re-audits the entire tree
and the delegated kernel cgroup before granting a session.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile

from verify_language_servers import safe_file, verify, reviewed_native_resources

PLATFORM = "linux-x86_64"
EXECUTABLES = {
    "rust-analyzer": "rust-analyzer",
    "clangd": "clangd",
    "ruff": "ruff",
}
ENV_PINS = {
    "rust-analyzer": "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256",
    "clangd": "LU_IDE_E2_CLANGD_BUNDLE_SHA256",
    "ruff": "LU_IDE_E2_RUFF_BUNDLE_SHA256",
}


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(64 * 1024):
            digest.update(block)
    return digest.hexdigest()


def bundle_digest(root: Path) -> str:
    """Exactly mirrors Linux rust worker audit_bundle path+NUL+SHA+NUL."""
    digest = hashlib.sha256()
    files = []
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError("symlink in native Linux language server bundle")
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            if not relative or any(not (
                char.isascii() and (char.isalnum() or char in "._-+")
            ) for char in relative.replace("/", "")):
                raise ValueError("unsafe native bundle filename")
            files.append((relative, path))
        elif not path.is_dir():
            raise ValueError("non-file native bundle entry")
    for relative, path in sorted(files):
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(file_digest(path).encode("ascii"))
        digest.update(b"\0")
    return digest.hexdigest()


def stage_linux(
    manifest_path: Path,
    source_root: Path,
    destination: Path,
) -> dict:
    included = verify(manifest_path, source_root, release=True)
    rows = {
        row["id"]: row for row in json.loads(
            manifest_path.read_text(encoding="utf-8")
        )["servers"]
    }
    if not included or any(
        ident not in EXECUTABLES or rows[ident]["category"] != "native"
        for ident in included
    ):
        raise ValueError("unsupported or empty reviewed native LSP set")
    if destination.exists() or destination.is_symlink():
        raise ValueError("Linux resource staging destination already exists")
    if destination.parent.is_symlink():
        raise ValueError("untrusted Linux resource staging parent")
    destination.parent.mkdir(parents=True, exist_ok=True)

    receipt = {
        "schema": "lu.ide.linux-lsp-staging.v1",
        "releaseApprovedInManifest": True,
        "platform": PLATFORM,
        "installedRootOwned": False,
        "nativeWorkerVerified": False,
        "servers": {},
    }
    with tempfile.TemporaryDirectory(
        prefix=".lsp-linux-stage-", dir=destination.parent
    ) as temporary:
        stage_root = Path(temporary) / "resources"
        stage_root.mkdir(mode=0o755)
        for ident in included:
            row = rows[ident]
            source = safe_file(source_root, row["assetByPlatform"][PLATFORM])
            expected = row["sha256ByPlatform"][PLATFORM]
            server_root = stage_root / "language-servers" / ident
            binary = server_root / "bin" / EXECUTABLES[ident]
            binary.parent.mkdir(parents=True, mode=0o755)
            shutil.copyfile(source, binary)
            binary.chmod(0o755)
            if file_digest(binary) != expected:
                raise ValueError("binary digest changed while staging")
            # Original native DLL/ELF and Clang resource closure must survive
            # Debian staging, not only the main executable and legal notice.
            native_resources=reviewed_native_resources(row,source_root,PLATFORM)
            for relative,original,expected_resource in native_resources:
                asset=server_root.joinpath(*relative.split("/"))
                asset.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(original,asset)
                asset.chmod(0o755 if ".so" in asset.name else 0o644)
                if file_digest(asset)!=expected_resource:
                    raise ValueError("original native resource mutated during staging")
            notices = server_root / "NOTICES"
            notices.mkdir(mode=0o755)
            for index, relative in enumerate(row["licenseNoticeFiles"]):
                original = safe_file(source_root, relative)
                safe_basename = "".join(
                    char if (char.isascii() and (
                        char.isalnum() or char in "._-+"
                    )) else "_" for char in original.name
                )
                target = notices / f"{index:03d}-{safe_basename}"
                shutil.copyfile(original, target)
                target.chmod(0o644)
                if file_digest(original) != file_digest(target):
                    raise ValueError("original notice digest changed")
            checksum = bundle_digest(server_root)
            receipt["servers"][ident] = {
                "binarySha256": expected,
                "bundleSha256": checksum,
                "rustEnvPin": ENV_PINS[ident],
                "licenseNotices": len(row["licenseNoticeFiles"]),
                "originalNativeResourceFiles":len(native_resources),
                "releaseVersion": row["releaseVersion"],
            }
        (stage_root / "linux-lsp-staging-evidence.json").write_text(
            json.dumps(receipt, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        os.replace(stage_root, destination)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--bundle-root", required=True, type=Path)
    parser.add_argument("--destination", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = stage_linux(args.manifest, args.bundle_root, args.destination)
    except (OSError, ValueError, TypeError, KeyError,
            json.JSONDecodeError) as error:
        print(f"DENIED: Linux LSP resource staging: {error}",
              file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    print("STAGED_LINUX_LSP_RESOURCES_ONLY=PASS")
    print("LINUX_INSTALLED_ROOT_OWNED_WORKER=NOT_ACCEPTED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
