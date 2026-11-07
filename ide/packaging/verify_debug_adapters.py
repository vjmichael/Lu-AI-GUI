#!/usr/bin/env python3
"""WO6 installed LLDB-DAP release safety gate. No downloads or staging.

The default IDE has NO approved debugger. GitHub source/CI fixture binaries do
not turn into redistributable release assets. Both platform bundle-tree SHA pins
must come from independently reviewed, signed/pinned native release artifacts
and matching transitive licenses; only then may the installer stage them.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

MANIFEST = Path(__file__).with_name("debug-adapters.manifest.json")
HEX = set("0123456789abcdefABCDEF")


def errors(data: object, release: bool = False) -> list[str]:
    issues: list[str] = []
    if not isinstance(data, dict):
        return ["manifest_not_object"]
    if data.get("schemaVersion") != 1 or data.get("adapterId") != "lldb-dap":
        issues.append("unsupported_adapter_schema")
    if data.get("licenseSpdx") != "Apache-2.0 WITH LLVM-exception":
        issues.append("unreviewed_license")
    fixture = data.get("fixedDebuggee", {})
    if not isinstance(fixture, dict) or fixture.get("userSelectedTarget") is not False:
        issues.append("arbitrary_debuggee_disallowed")
    if not release:
        return issues
    if data.get("releaseApproved") is not True:
        issues.append("release_approval_absent")
    review = data.get("review")
    if not isinstance(review, dict) or any(
        review.get(key) != "approved"
        for key in ("artifactProvenance", "transitiveLicenses", "security",
                    "signatures", "installedNativeAcceptance")
    ):
        issues.append("independent_review_incomplete")
    platforms = data.get("platforms", {})
    if not isinstance(platforms, dict) or set(platforms) != {"windows-x86_64", "linux-x86_64"}:
        issues.append("platform_matrix_incomplete")
        return issues
    for platform, package in platforms.items():
        if not isinstance(package, dict):
            issues.append(f"{platform}_manifest_invalid")
            continue
        digest = package.get("bundleSha256")
        if not isinstance(digest, str) or len(digest) != 64 or any(
            char not in HEX for char in digest
        ):
            issues.append(f"{platform}_sha_missing")
        if package.get("shaAlgorithm") != "lu-ide-native-bundle-tree":
            issues.append(f"{platform}_sha_algorithm_wrong")
        if not isinstance(package.get("licenseNotice"), str) or not package["licenseNotice"].strip():
            issues.append(f"{platform}_notices_missing")
        if package.get("dependenciesReviewed") is not True:
            issues.append(f"{platform}_dependencies_unreviewed")
        if package.get("installedAppAccepted") is not True:
            issues.append(f"{platform}_installed_acceptance_missing")
        if platform == "windows-x86_64":
            thumb = package.get("authenticodeSignerThumbprint")
            if not isinstance(thumb, str) or len(thumb) != 40 or any(
                char not in HEX for char in thumb
            ):
                issues.append("windows_signing_identity_unverified")
        if platform == "linux-x86_64" and package.get("rootOwnedImmutablePackage") is not True:
            issues.append("linux_filesystem_provenance_unverified")
        expected = {
            "windows-x86_64": ("bin/lldb-dap.exe", "debuggee/wo6-selftest.exe"),
            "linux-x86_64": ("bin/lldb-dap", "debuggee/wo6-selftest"),
        }[platform]
        if (package.get("executable"), package.get("debuggee")) != expected:
            issues.append(f"{platform}_unexpected_executable")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description="Fail-closed WO6 native debug adapter audit")
    parser.add_argument("--release", action="store_true", help="require full native redistribution approval")
    args = parser.parse_args()
    try:
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        print("DENIED: debugger manifest missing or invalid")
        return 2
    found = errors(data, args.release)
    if found:
        for problem in found:
            print(f"DENIED: {problem}")
        return 1
    print("PASS: reviewed source manifest shape only" if not args.release else
          "PASS: complete native debugger release metadata (OS tests remain separately required)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
