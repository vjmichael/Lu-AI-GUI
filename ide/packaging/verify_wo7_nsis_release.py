#!/usr/bin/env python3
"""WO7 signed-NSIS binary preflight, not a release or installed-app acceptance."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from verify_debug_adapters import errors as debugger_errors

HEX40 = re.compile(r"[a-fA-F0-9]{40}\Z")
HEX64 = re.compile(r"[a-fA-F0-9]{64}\Z")
GATES = ("nativeWindows", "installedSigned", "serviceLifecycle", "uninstall")


class Denied(ValueError):
    pass


def safe_asset(root, name):
    if not isinstance(name, str) or not name or "\\" in name or ":" in name:
        raise Denied("unsafe_asset_path")
    parts = name.split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise Denied("unsafe_asset_path")
    path = root
    for part in parts:
        path = path / part
        if path.is_symlink():
            raise Denied("symlink_asset_denied")
    if not path.is_file() or not path.resolve().is_relative_to(root.resolve()):
        raise Denied("missing_or_escaped_asset")
    return path


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def native_signature(path):
    if os.name != "nt":
        raise Denied("native_windows_signature_required")
    command = (
        "$ErrorActionPreference = 'Stop'; "
        "$s = Get-AuthenticodeSignature -LiteralPath $env:LU_IDE_WO7_FILE; "
        "[pscustomobject]@{status=[string]$s.Status; "
        "thumbprint=[string]$s.SignerCertificate.Thumbprint} | ConvertTo-Json -Compress"
    )
    env = os.environ.copy()
    env["LU_IDE_WO7_FILE"] = str(path)
    try:
        run = subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand",
             base64.b64encode(command.encode("utf-16-le")).decode("ascii")],
            capture_output=True, text=True, check=True, timeout=30, env=env)
        result = json.loads(run.stdout)
        return result["status"], result["thumbprint"]
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as exc:
        raise Denied("signature_verification_failed") from exc


def audit(manifest, root, source_sha, signature=native_signature):
    data = json.loads(manifest.read_text(encoding="utf-8"))
    if data.get("schema") != "lu.ide.wo7.signed-nsis.v1" or data.get("releaseApproved") is not True:
        raise Denied("release_unapproved")
    if not isinstance(source_sha, str) or not HEX40.fullmatch(source_sha) or data.get("sourceSha") != source_sha:
        raise Denied("source_sha_mismatch")
    if data.get("licensesReviewed") is not True:
        raise Denied("license_review_missing")
    states = data.get("acceptance")
    if not isinstance(states, dict) or any(states.get(k) != "approved" for k in GATES):
        raise Denied("native_and_installed_acceptance_missing")
    debugger = json.loads(Path(__file__).with_name("debug-adapters.manifest.json").read_text(encoding="utf-8"))
    if debugger_errors(debugger, release=True):
        raise Denied("wo6_debugger_not_release_approved")
    root = root.resolve(strict=True)
    evidence = []
    for item_type in ("installer", "service", "desktop"):
        entry = data.get(item_type)
        if not isinstance(entry, dict):
            raise Denied(item_type + "_record_missing")
        sha, thumb = entry.get("sha256"), entry.get("signerThumbprint")
        if not isinstance(sha, str) or not HEX64.fullmatch(sha):
            raise Denied(item_type + "_sha_pin_missing")
        if not isinstance(thumb, str) or not HEX40.fullmatch(thumb):
            raise Denied(item_type + "_signer_pin_missing")
        path = safe_asset(root, entry.get("path"))
        actual = digest(path)
        if actual.lower() != sha.lower():
            raise Denied(item_type + "_digest_mismatch")
        status, signed_by = signature(path)
        if status != "Valid" or signed_by.lower() != thumb.lower():
            raise Denied(item_type + "_signature_invalid")
        evidence.append({"kind": item_type, "sha256": actual, "signer": signed_by.lower()})
    return {"sourceSha": source_sha, "artifacts": evidence, "result": "binary_preflight_only",
            "installedAcceptancePerformed": False}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--manifest", type=Path, default=Path(__file__).with_name("wo7-release.manifest.json"))
    p.add_argument("--artifact-root", type=Path, required=True)
    p.add_argument("--source-sha", required=True)
    a = p.parse_args()
    if os.name != "nt":
        print("DENIED: real Windows signature verification required", file=sys.stderr)
        return 1
    try:
        print(json.dumps(audit(a.manifest, a.artifact_root, a.source_sha), sort_keys=True))
        return 0
    except (ValueError, TypeError, OSError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_release_input"), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
