#!/usr/bin/env python3
"""WO7 stage-to-Tauri resource handoff (never signs or blesses a release).

Requires a complete independently prepared payload, current exact source SHA,
and an identical local compiled desktop executable. Rejects unlisted bytes,
links, source drift and preexisting resources before copying to a clean tree.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

from verify_wo7_nsis_release import Denied, safe_asset


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as h:
        for block in iter(lambda: h.read(1048576), b""):
            digest.update(block)
    return digest.hexdigest()


def prepare(payload: Path, desktop: Path, tauri_dir: Path, source_sha: str) -> dict:
    root = payload.resolve(strict=True)
    attestation = json.loads(safe_asset(root, "resources/wo7-staged-tree.json").read_text(encoding="utf-8"))
    if attestation.get("schema") != "lu.ide.wo7.staged-tree.v1" or attestation.get("sourceSha") != source_sha:
        raise Denied("source_and_stage_attestation_mismatch")
    if attestation.get("releaseApproved") is not False:
        raise Denied("stage_must_not_be_release_claim")
    rows = attestation.get("files")
    if not isinstance(rows, list) or not rows:
        raise Denied("empty_staged_tree")
    seen = set()
    expected = {"resources/wo7-staged-tree.json",
                "resources/wo7-service-policy.json",
                "resources/installer/wo7-service-lifecycle.ps1"}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("destination"), str):
            raise Denied("invalid_staged_record")
        dest, wanted = row["destination"], row.get("sha256")
        if dest.casefold() in seen:
            raise Denied("duplicate_staged_target")
        seen.add(dest.casefold())
        src = safe_asset(root, dest)
        if hash_file(src).lower() != wanted:
            raise Denied("changed_staged_binary")
        expected.add(dest)
        for notice in row.get("notices", []):
            path = notice.get("path")
            if path in expected or not isinstance(path, str):
                raise Denied("duplicate_or_invalid_notice")
            nsrc = safe_asset(root, path)
            if hash_file(nsrc).lower() != notice.get("sha256"):
                raise Denied("changed_staged_notice")
            expected.add(path)
    for path, key in (("resources/wo7-service-policy.json", "servicePolicySha256"),
                      ("resources/installer/wo7-service-lifecycle.ps1", "lifecycleScriptSha256")):
        if hash_file(safe_asset(root, path)).lower() != attestation.get(key):
            raise Denied("changed_service_install_policy")
    actual_files = set()
    for directory, folders, files in os.walk(root, followlinks=False):
        for name in folders + files:
            item = Path(directory) / name
            if item.is_symlink():
                raise Denied("link_in_stage_tree")
        for name in files:
            actual_files.add((Path(directory) / name).relative_to(root).as_posix())
    if actual_files != expected:
        raise Denied("unlisted_or_missing_stage_bytes")
    staged_desktop = safe_asset(root, "lu-ide-desktop.exe")
    if not desktop.is_file() or desktop.is_symlink() or hash_file(staged_desktop) != hash_file(desktop):
        raise Denied("compiled_desktop_differs_from_reviewed_payload")
    resources = tauri_dir / "resources"
    if resources.exists() or resources.is_symlink():
        raise Denied("tauri_resources_must_be_new")
    if not tauri_dir.is_dir():
        raise Denied("tauri_build_directory_absent")
    shutil.copytree(root / "resources", resources, symlinks=False)
    # Postcopy verification includes all manifest-listed resources.
    for rel in expected:
        if rel == "lu-ide-desktop.exe":
            continue
        if hash_file(safe_asset(root, rel)) != hash_file(safe_asset(tauri_dir, rel)):
            shutil.rmtree(resources)
            raise Denied("tauri_resource_bytes_changed")
    return {"sourceSha": source_sha, "resources": len(expected) - 1,
            "readyForUnsignedNsisBundleAssemblyOnly": True,
            "signedOrInstalled": False}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--payload", type=Path, required=True)
    p.add_argument("--desktop", type=Path, required=True)
    p.add_argument("--tauri-dir", type=Path, required=True)
    p.add_argument("--source-sha", required=True)
    a = p.parse_args()
    try:
        result = prepare(a.payload, a.desktop, a.tauri_dir, a.source_sha)
    except (OSError, TypeError, ValueError, KeyError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_tauri_bundle_stage"),
              file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
