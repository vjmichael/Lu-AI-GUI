#!/usr/bin/env python3
"""DEV-02.5: disposable CI-only native LSP test payload and hash-checked install.

This deliberately is NOT an Lu installer, a protected language-server session,
a full dependency-closure package, or permission to redistribute executables.
No candidate is launched. The output stays under RUNNER_TEMP and is never
uploaded by the CI workflow. Only a bounded JSON evidence receipt is uploaded.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import zipfile

from collect_original_lsp_notices import validate as validate_notices, check_original
from review_upstream_lsp_archives import (
    TARGETS, checked_manifest, download_locked, extract_one, sha256,
    MAX_ARCHIVE,
)

SCHEMA = "lu.ide.dev02.disposable-native-lsp-payload.v1"
MAX_PACKAGE_MEMBER = 512 * 1024 * 1024


def allowed_temp_destination(path: Path) -> None:
    runner = Path(os.environ.get("RUNNER_TEMP", ""))
    if os.environ.get("GITHUB_ACTIONS") != "true" or not runner.is_dir():
        raise ValueError("disposable GitHub Actions runner required")
    resolved = path.resolve(strict=False)
    if not resolved.is_relative_to(runner.resolve()):
        raise ValueError("destination outside isolated RUNNER_TEMP")
    for ancestor in (path, *path.parents):
        if ancestor == runner or ancestor == runner.parent:
            break
        if ancestor.is_symlink():
            raise ValueError("linked destination ancestor denied")
    if path.exists() or path.is_symlink():
        raise ValueError("output root must be absent")


def file_digest(path: Path) -> dict:
    return {"sha256": sha256(path), "bytes": path.stat().st_size}


def install_verified(package: Path, destination: Path, expected: dict) -> dict:
    """Install only our exact sealed member inventory, never extractall()."""
    if destination.exists() or destination.is_symlink():
        raise ValueError("test install must start absent")
    members = expected["files"]
    provider_names = {x for x in members if x.startswith("providers/")}
    notice_names = {x for x in members if x.startswith("notices/")}
    if (expected.get("schema") != SCHEMA or
            expected.get("releaseApproved") is not False or
            expected.get("redistributionApproved") is not False or
            expected.get("nativeWorkerContainmentAccepted") is not False or
            expected.get("dependencyClosureVerified") is not False or
            expected.get("noAutomaticExecution") is not True or
            len(provider_names) != 3 or len(notice_names) != 4 or
            len(members) != 7):
        raise ValueError("incomplete or unauthorized candidate package")
    from pathlib import PurePosixPath
    for name, record in members.items():
        parts = PurePosixPath(name).parts
        if (not parts or any(p in ("", ".", "..") for p in parts)
                or name.startswith("/") or "\\" in name
                or not isinstance(record.get("bytes"), int)
                or not 0 < record["bytes"] <= MAX_PACKAGE_MEMBER
                or not isinstance(record.get("sha256"), str)
                or len(record["sha256"]) != 64):
            raise ValueError("unsafe member manifest")
    with zipfile.ZipFile(package) as archive:
        infos = archive.infolist()
        names = [i.filename for i in infos]
        if (len(infos) != 8 or len(set(names)) != 8
                or set(names) != set(members) | {"bundle-manifest.json"}):
            raise ValueError("unexpected, duplicate or missing package members")
        for info in infos:
            if (info.is_dir() or info.file_size > MAX_PACKAGE_MEMBER
                    or ((info.external_attr >> 16) & 0o170000) == 0o120000):
                raise ValueError("linked, directory or oversized package member")
        observed = json.loads(archive.read("bundle-manifest.json"))
        if observed != expected:
            raise ValueError("package metadata differs from sealed manifest")
        destination.mkdir(mode=0o700)
        for name, record in sorted(members.items()):
            target = destination.joinpath(*name.split("/"))
            target.parent.mkdir(parents=True, exist_ok=True)
            digest = hashlib.sha256()
            count = 0
            with archive.open(name) as reader, target.open("xb") as output:
                while part := reader.read(1024 * 1024):
                    count += len(part)
                    if count > record["bytes"]:
                        raise ValueError("package member size exceeds seal")
                    digest.update(part)
                    output.write(part)
            if count != record["bytes"] or digest.hexdigest() != record["sha256"]:
                raise ValueError("installed member hash/length mismatch")
            if name.startswith("providers/") and os.name != "nt":
                target.chmod(0o700)
        for name, record in members.items():
            path = destination.joinpath(*name.split("/"))
            if path.is_symlink() or file_digest(path) != record:
                raise ValueError("post-install file identity mismatch")
    return {
        "schema": "lu.ide.dev02.disposable-native-lsp-install-receipt.v1",
        "payloadFilesVerified": 7,
        "hashReadbackPassed": True,
        "packageSha256": sha256(package),
        "releaseApproved": False,
        "executedInsideLu": False,
        "nativeWorkerContainmentAccepted": False,
        "dependencyClosureVerified": False,
        "genuineNsisOrDebInstalled": False,
    }


def make_package(manifest: dict, notices: dict, archives: Path,
                 notice_root: Path, root: Path, platform: str,
                 *, fetch: bool = False) -> dict:
    if platform not in TARGETS:
        raise ValueError("unsupported native platform")
    native = checked_manifest(manifest)
    original = validate_notices(notices)
    if root.exists() or root.is_symlink():
        raise ValueError("test-only package root already exists")
    # Validate every notice first; unsigned candidate bytes are not install authority.
    notice_bytes = []
    for row in original:
        source = notice_root / row["destination"]
        if (source.is_symlink() or not source.is_file()
                or not source.resolve().is_relative_to(notice_root.resolve())):
            raise ValueError("linked, missing or escaped original upstream notice")
        data = source.read_bytes()
        check_original(row, data)
        notice_bytes.append((row["destination"], data))
    selected = [(name, info) for name, target, info in native if target == platform]
    if len(selected) != 3:
        raise ValueError("expected exactly three original native providers")
    # Download only immutable official SHA-locked archives, NEVER "latest".
    for name, info in selected:
        archive = archives / info["name"]
        if archive.is_symlink():
            raise ValueError("linked upstream candidate archive")
        if not archive.is_file():
            if not fetch:
                raise ValueError("offline original archive missing")
            archives.mkdir(parents=True, exist_ok=True)
            download_locked(info["url"], archive)
        if not (0 < archive.stat().st_size <= MAX_ARCHIVE):
            raise ValueError("unbounded candidate archive")
        if sha256(archive) != info["sha256"]:
            raise ValueError("candidate original upstream SHA256 mismatch")
    root.mkdir(mode=0o700)
    stage = root / "staged"
    stage.mkdir()
    files = {}
    for name, info in selected:
        rel = f"providers/{name}/{info['binary']}"
        target = stage.joinpath(*rel.split("/"))
        target.parent.mkdir(parents=True, exist_ok=True)
        digest, size = extract_one(
            archives / info["name"], info["kind"], info["binary"], target
        )
        files[rel] = {"sha256": digest, "bytes": size}
    for name, data in notice_bytes:
        rel = "notices/" + name
        target = stage.joinpath(*rel.split("/"))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        files[rel] = file_digest(target)
    if len(files) != 7:
        raise ValueError("provider or notice lost in candidate package")
    seal = {
        "schema": SCHEMA, "platform": platform, "files": files,
        "releaseApproved": False, "redistributionApproved": False,
        "nativeWorkerContainmentAccepted": False,
        "dependencyClosureVerified": False, "sourceOnlyNoticesVerified": True,
        "candidateExecutables": sorted(name for name in files if name.startswith("providers/")),
        "noAutomaticExecution": True,
    }
    package = root / "native-lsp-disposable-test-payload.zip"
    with zipfile.ZipFile(package, "x", zipfile.ZIP_STORED) as writer:
        for name in sorted(files):
            writer.write(stage.joinpath(*name.split("/")), arcname=name)
        writer.writestr(
            "bundle-manifest.json",
            json.dumps(seal, sort_keys=True, indent=2) + "\n"
        )
    installed = root / "installed"
    receipt = install_verified(package, installed, seal)
    receipt["platform"] = platform
    receipt["archiveDigestsVerified"] = 3
    receipt["originalNoticesVerified"] = 4
    (root / "verification-receipt.json").write_text(
        json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--notices-manifest", type=Path, required=True)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--notices-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--platform", choices=sorted(TARGETS), required=True)
    parser.add_argument("--fetch-locked", action="store_true")
    a = parser.parse_args()
    try:
        allowed_temp_destination(a.output)
        result = make_package(
            json.loads(a.manifest.read_text(encoding="utf-8")),
            json.loads(a.notices_manifest.read_text(encoding="utf-8")),
            a.archives, a.notices_root, a.output, a.platform,
            fetch=a.fetch_locked
        )
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile,
            json.JSONDecodeError) as error:
        print("DENIED: test-only LSP staging: " + str(error), file=sys.stderr)
        return 2
    print("DEV02_DISPOSABLE_THREE_NATIVE_PROVIDER_BYTES_VERIFIED=PASS")
    print("DEV02_DISPOSABLE_FOUR_ORIGINAL_NOTICES_VERIFIED=PASS")
    print("DEV02_DISPOSABLE_HASH_CHECKED_TEST_INSTALL=PASS")
    print("DEV02_REAL_INSTALLED_NSIS_OR_DEB_AND_PROTECTED_LSP=NOT_TESTED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
