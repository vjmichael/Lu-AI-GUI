#!/usr/bin/env python3
"""WO7-B reviewed native Windows payload staging.

No download, installer run, signing, or declaration of installed acceptance.
An input inventory is never executable authority. Only release-review-approved,
byte-pinned files enter a NEW tree; every notice and output receives a digest.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile

from verify_wo7_nsis_release import Denied, safe_asset

SHA = re.compile(r"[a-fA-F0-9]{64}\Z")
REV = re.compile(r"[a-fA-F0-9]{40}\Z")
ROLES = {"desktop", "service", "debugger", "debuggee", "runtime", "language-server"}
FIXED = {
    "desktop": "lu-ide-desktop.exe",
    "service": "resources/recovery/windows_recovery_service.exe",
    "debugger": "resources/debugger/bin/lldb-dap.exe",
    "debuggee": "resources/debugger/debuggee/wo6-selftest.exe",
}
REQUIRED = {"desktop", "service", "debugger", "debuggee"}
MAX_ASSET = 1024 * 1024 * 1024


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def safe_target(value: object) -> Path:
    if not isinstance(value, str) or not value or len(value) > 240 or "\\" in value or ":" in value:
        raise Denied("unsafe_destination")
    parts = value.split("/")
    if any(not p or p in {".", ".."} for p in parts):
        raise Denied("unsafe_destination")
    if any(p.lower() in {"con", "prn", "aux", "nul"} or p.endswith((" ", ".")) for p in parts):
        raise Denied("unsafe_destination")
    return Path(*parts)


def validate(data: dict, source_sha: str) -> list[dict]:
    if data.get("schema") != "lu.ide.wo7.bundle.v1" or data.get("approved") is not True:
        raise Denied("bundle_not_review_approved")
    if not REV.fullmatch(source_sha) or data.get("sourceSha") != source_sha:
        raise Denied("source_mismatch")
    installer = data.get("installerScript")
    if not isinstance(installer, dict) or not isinstance(installer.get("sha256"), str) or not SHA.fullmatch(installer["sha256"]):
        raise Denied("pinned_lifecycle_script_missing")
    if not isinstance(installer.get("source"), str):
        raise Denied("lifecycle_script_source_missing")
    entries = data.get("files")
    if not isinstance(entries, list) or not 4 <= len(entries) <= 100:
        raise Denied("bundle_file_inventory_incomplete")
    roles, destinations, notices = set(), set(), set()
    for row in entries:
        if not isinstance(row, dict) or row.get("role") not in ROLES:
            raise Denied("unrecognized_bundle_role")
        role = row["role"]
        destination = row.get("destination")
        target = safe_target(destination)
        normalized = destination.casefold()
        if normalized in destinations or normalized in notices:
            raise Denied("duplicate_casefold_destination")
        destinations.add(normalized)
        if role in FIXED and destination != FIXED[role]:
            raise Denied("fixed_binary_path_mismatch")
        if role in ("runtime", "language-server") and not (
            destination.startswith("resources/debugger/bin/") if role == "runtime"
            else destination.startswith("resources/language-servers/")
        ):
            raise Denied("unapproved_runtime_destination")
        if role == "service":
            signer = row.get("signerThumbprint")
            if not isinstance(signer, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", signer):
                raise Denied("protected_service_signer_pin_missing")
        if role in roles and role in REQUIRED:
            raise Denied("duplicate_required_binary")
        roles.add(role)
        if not isinstance(row.get("sha256"), str) or not SHA.fullmatch(row["sha256"]):
            raise Denied("missing_input_digest")
        if not isinstance(row.get("license"), str) or not row["license"].strip():
            raise Denied("license_identifier_missing")
        if row.get("dependenciesReviewed") is not True or row.get("importClosureReviewed") is not True:
            raise Denied("dependency_and_import_review_missing")
        marks = row.get("notices")
        if not isinstance(marks, list) or not marks:
            raise Denied("license_notice_missing")
        for notice in marks:
            if not isinstance(notice, dict) or not isinstance(notice.get("sha256"), str) or not SHA.fullmatch(notice["sha256"]):
                raise Denied("notice_digest_missing")
            # Copy notices under a controlled prefix, never trusting supplied output paths.
            if not isinstance(notice.get("source"), str):
                raise Denied("notice_source_missing")
        if target.suffix.lower() not in {".exe", ".dll"}:
            raise Denied("unexpected_executable_asset_suffix")
    if not REQUIRED.issubset(roles):
        raise Denied("fixed_release_roles_missing")
    return entries


def stage(manifest: Path, inputs: Path, output: Path, source_sha: str) -> dict:
    data = json.loads(manifest.read_text(encoding="utf-8"))
    entries = validate(data, source_sha)
    if output.exists() or output.is_symlink():
        raise Denied("output_must_be_new")
    root = inputs.resolve(strict=True)
    helper = safe_asset(root, data["installerScript"]["source"])
    if sha256(helper).lower() != data["installerScript"]["sha256"].lower():
        raise Denied("lifecycle_script_digest_changed")
    prepared = []
    for row in entries:
        src = safe_asset(root, row["source"])
        if src.stat().st_size == 0 or src.stat().st_size > MAX_ASSET:
            raise Denied("invalid_input_size")
        if sha256(src).lower() != row["sha256"].lower():
            raise Denied("input_digest_changed")
        record = {"role": row["role"], "destination": row["destination"],
                  "sha256": row["sha256"].lower(), "license": row["license"],
                  "notices": []}
        notice_assets = []
        for notice in row["notices"]:
            nsrc = safe_asset(root, notice["source"])
            if nsrc.stat().st_size == 0 or nsrc.stat().st_size > 1024 * 1024:
                raise Denied("notice_size_invalid")
            if sha256(nsrc).lower() != notice["sha256"].lower():
                raise Denied("notice_changed")
            notice_assets.append(nsrc)
        prepared.append((row, src, record, notice_assets))
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".wo7-stage-", dir=output.parent) as tmp:
        stage_root = Path(tmp) / "payload"
        stage_root.mkdir()
        attestation = {"schema": "lu.ide.wo7.staged-tree.v1", "sourceSha": source_sha,
                       "releaseApproved": False, "files": []}
        for i, (row, src, record, marks) in enumerate(prepared):
            dest = stage_root / safe_target(row["destination"])
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dest, follow_symlinks=False)
            if sha256(dest).lower() != record["sha256"]:
                raise Denied("post_copy_digest_changed")
            for j, mark in enumerate(marks):
                notice_name = f"{i:03d}-{j:03d}.txt"
                notice_dest = stage_root / "resources" / "third-party-notices" / notice_name
                notice_dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(mark, notice_dest, follow_symlinks=False)
                notice_digest = sha256(notice_dest)
                if notice_digest != row["notices"][j]["sha256"].lower():
                    raise Denied("post_copy_notice_changed")
                record["notices"].append({"path": "resources/third-party-notices/" + notice_name,
                                           "sha256": notice_digest})
            attestation["files"].append(record)
        installer_path = stage_root / "resources" / "installer" / "wo7-service-lifecycle.ps1"
        installer_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(helper, installer_path, follow_symlinks=False)
        if sha256(installer_path).lower() != data["installerScript"]["sha256"].lower():
            raise Denied("post_copy_lifecycle_script_changed")
        service = next(row for row in entries if row["role"] == "service")
        policy = {"schema": "lu.ide.wo7.service-policy.v1", "releaseApproved": True,
                  "sourceSha": source_sha, "serviceSha256": service["sha256"].lower(),
                  "signerThumbprint": service["signerThumbprint"].lower()}
        policy_file = stage_root / "resources" / "wo7-service-policy.json"
        policy_file.write_text(json.dumps(policy, indent=2, sort_keys=True) + "\\n", encoding="utf-8")
        attestation["servicePolicySha256"] = sha256(policy_file)
        attestation["lifecycleScriptSha256"] = sha256(installer_path)
        (stage_root / "resources" / "wo7-staged-tree.json").write_text(
            json.dumps(attestation, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        os.replace(stage_root, output)
    return attestation


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--inputs", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--source-sha", required=True)
    a = p.parse_args()
    try:
        result = stage(a.manifest, a.inputs, a.output, a.source_sha)
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_or_unavailable_input"), file=sys.stderr)
        return 1
    print(json.dumps({"stage": "reviewed_payload_only", "files": len(result["files"]),
                      "releaseAccepted": False}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
