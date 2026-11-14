#!/usr/bin/env python3
"""INT02-F: offline, exact-byte LSP installer resource handoff.

A resource handoff is *not* code signing, OS service admission, an installed
package, or successful native language inference. It consumes only a separately
reviewed and already-staged payload. Never fetches or executes a provider.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys

from stage_language_server_release import digest_file, windows_lsp_tree_digest
from stage_linux_language_server_release import bundle_digest

PINS = {
    "clangd": ("clangd", "LU_IDE_E2_CLANGD_BUNDLE_SHA256"),
    "rust-analyzer": ("rust-analyzer", "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256"),
    "ruff": ("ruff", "LU_IDE_E2_RUFF_BUNDLE_SHA256"),
    "pyright": ("pyright-langserver", "LU_IDE_E2_PYRIGHT_BUNDLE_SHA256"),
}
HEX = re.compile(r"^[0-9a-f]{64}$")


def regular_tree(root: Path) -> set[str]:
    if not root.is_dir() or root.is_symlink():
        raise ValueError("staging source must be a real directory")
    names: set[str] = set()
    total = 0
    for directory, folders, files in os.walk(root, followlinks=False):
        for name in folders + files:
            item = Path(directory) / name
            if item.is_symlink():
                raise ValueError("symlink/reparse link in LSP resources")
        for name in files:
            item = Path(directory) / name
            if not item.is_file():
                raise ValueError("nonregular staged file")
            relative = item.relative_to(root).as_posix()
            if not relative or relative.casefold() in names:
                raise ValueError("duplicate or unsafe staged source")
            names.add(relative.casefold())
            total += item.stat().st_size
            if len(names) > 2500 or total > 3 * 1024 * 1024 * 1024:
                raise ValueError("stage exceeds offline package bounds")
    return names


def checked_stage(source: Path, platform: str) -> dict[str, str]:
    if platform not in ("windows", "linux"):
        raise ValueError("unsupported release platform")
    names = regular_tree(source)
    evidence_file = ("release-evidence.json" if platform == "windows" else
                     "linux-lsp-staging-evidence.json")
    if evidence_file not in names:
        raise ValueError("missing reviewed stage evidence")
    evidence = json.loads((source / evidence_file).read_text(encoding="utf-8"))
    if evidence.get("schema") != ("lu.ide.e2.staged-release.v1" if platform == "windows" else
                                  "lu.ide.linux-lsp-staging.v1"):
        raise ValueError("unrecognized release stage")
    if platform == "linux" and evidence.get("releaseApprovedInManifest") is not True:
        raise ValueError("Linux release approval not evidenced")
    servers = evidence.get("servers")
    if not isinstance(servers, dict) or not servers or not set(servers) <= set(PINS):
        raise ValueError("unrecognized or empty LSP provider inventory")
    result: dict[str, str] = {}
    expected_files = {evidence_file}
    for ident, metadata in servers.items():
        if not isinstance(metadata, dict):
            raise ValueError("malformed server receipt")
        executable, env = PINS[ident]
        name = executable + (".exe" if platform == "windows" else "")
        relative = "language-servers/" + ident + "/bin/" + name
        binary = source / relative
        root = binary.parent.parent
        if binary.is_symlink() or not binary.is_file():
            raise ValueError("missing staged native executable")
        expected_binary = metadata.get("binarySha256")
        expected_bundle = metadata.get("bundleSha256")
        if not isinstance(expected_binary, str) or not HEX.fullmatch(expected_binary) or (
            not isinstance(expected_bundle, str) or not HEX.fullmatch(expected_bundle)
        ):
            raise ValueError("missing exact release digest")
        if metadata.get("rustEnvPin") != env or digest_file(binary) != expected_binary:
            raise ValueError("executable changed or pin misrouted")
        observed = (windows_lsp_tree_digest(root) if platform == "windows"
                    else bundle_digest(root))
        if observed != expected_bundle:
            raise ValueError("full native bundle changed since staging")
        result[env] = observed
        expected_files.update(name for name in names
            if name.startswith(("language-servers/" + ident + "/").casefold()))
    if platform == "windows":
        service = "recovery/windows_recovery_service.exe"
        expected = evidence.get("serviceSha256")
        if (service not in names or not isinstance(expected, str) or
                not HEX.fullmatch(expected) or
                digest_file(source / service) != expected):
            raise ValueError("protected service missing or changed")
        result["LU_IDE_E2_D7D_SERVICE_SHA256"] = expected
        expected_files.add(service)
    if names != expected_files:
        raise ValueError("stage includes an unreviewed executable or extra resource")
    return result


def prepare(source: Path, destination: Path, pins_file: Path, platform: str) -> dict:
    """Copies only a complete, SHA-remeasured source; never overwrites."""
    if destination.exists() or destination.is_symlink() or (
        pins_file.exists() or pins_file.is_symlink()
    ):
        raise ValueError("output destinations must not already exist")
    if pins_file.resolve().is_relative_to(destination.resolve()):
        raise ValueError("build pins cannot be included in the copied runtime bundle")
    pins = checked_stage(source, platform)
    destination.parent.mkdir(parents=True, exist_ok=True)
    pins_file.parent.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copytree(source, destination, symlinks=False)
        if checked_stage(destination, platform) != pins:
            raise ValueError("resources changed during the installer handoff")
        # These keys are *inputs* to a separately reviewed native-lsp build.
        # Runtime code still refuses any mismatched installed digest or missing
        # Windows protected service. The host never reads this as a live grant.
        pins_file.write_text(json.dumps({
            "schema": "lu.ide.int02f.compile-pins.v1",
            "platform": platform,
            "pins": pins,
            "releaseInstalled": False,
            "nativeHumanAcceptance": False,
        }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except Exception:
        if destination.exists():
            shutil.rmtree(destination)
        if pins_file.exists():
            pins_file.unlink()
        raise
    return {"providerCount": len(pins) - (1 if platform == "windows" else 0),
            "resourceHandoffVerified": True, "nativeInstalled": False,
            "nativeWorkerAccepted": False}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reviewed-stage", type=Path, required=True)
    parser.add_argument("--resource-destination", type=Path, required=True)
    parser.add_argument("--compile-pins", type=Path, required=True)
    parser.add_argument("--platform", choices=("windows", "linux"), required=True)
    args = parser.parse_args()
    try:
        outcome = prepare(args.reviewed_stage, args.resource_destination,
                          args.compile_pins, args.platform)
    except (OSError, ValueError, TypeError, KeyError) as exc:
        print("DENIED: offline LSP bundle handoff: " + str(exc), file=sys.stderr)
        return 2
    print(json.dumps(outcome, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
