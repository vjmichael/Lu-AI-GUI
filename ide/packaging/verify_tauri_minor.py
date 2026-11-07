#!/usr/bin/env python3
"""Fail closed before NSIS when Tauri Rust/JS/CLI minor versions drift.

Source and installed package checks, not a substitute for an actual installer.
"""
import argparse
import json
from pathlib import Path
import re
import sys
import tomllib

EXACT_RUST = re.compile(r"=(\d+)\.(\d+)\.(\d+)")
EXACT_NPM = re.compile(r"(\d+)\.(\d+)\.(\d+)")
CLI_VERSION = re.compile(r"@tauri-apps/cli@(\d+\.\d+\.\d+)")
WORKFLOWS = (
    ".github/workflows/ide-e2-supervised-lsp.yml",
    ".github/workflows/ide-e2-unsigned-installed-smoke.yml",
)

class VersionDenied(ValueError):
    pass

def check_alignment(root: Path):
    root = Path(root)
    try:
        cargo = tomllib.loads((root / "ide/app/src-tauri/Cargo.toml").read_text(encoding="utf-8"))
        package = json.loads((root / "ide/app/package.json").read_text(encoding="utf-8"))
        rust_spec = cargo["dependencies"]["tauri"]["version"]
        api_spec = package["dependencies"]["@tauri-apps/api"]
    except (OSError, KeyError, TypeError, ValueError) as e:
        raise VersionDenied("required_tauri_dependency_unreadable") from e
    rust = EXACT_RUST.fullmatch(rust_spec) if isinstance(rust_spec, str) else None
    api = EXACT_NPM.fullmatch(api_spec) if isinstance(api_spec, str) else None
    if not rust:
        raise VersionDenied("rust_tauri_requires_exact_patch_pin")
    if not api:
        raise VersionDenied("javascript_api_requires_exact_patch_pin")
    minor = rust.group(1, 2)
    if api.group(1, 2) != minor:
        raise VersionDenied("rust_js_major_minor_mismatch")
    for rel in WORKFLOWS:
        try:
            body = (root / rel).read_text(encoding="utf-8")
        except OSError as e:
            raise VersionDenied("required_installed_workflow_missing") from e
        found = CLI_VERSION.findall(body)
        if len(found) != 1:
            raise VersionDenied("installed_workflow_requires_one_exact_cli_pin:" + rel)
        match = EXACT_NPM.fullmatch(found[0])
        if not match or match.group(1, 2) != minor:
            raise VersionDenied("installed_workflow_cli_major_minor_mismatch:" + rel)
        if "--bundles nsis" not in body or "Test-InstalledAlphaNsis.ps1" not in body:
            raise VersionDenied("installed_native_smoke_gate_removed:" + rel)
    # Cross-check the resolved Tauri runtime graph. Cargo's semver ranges
    # caused real mismatched Monitor/Error APIs within the Tauri crate.
    lock = root / "ide/app/src-tauri/Cargo.lock"
    if lock.exists():
        try:
            packages = tomllib.loads(lock.read_text(encoding="utf-8"))["package"]
        except (OSError, ValueError, TypeError, KeyError) as e:
            raise VersionDenied("resolved_cargo_lock_unreadable") from e
        for component in ("tauri", "tauri-runtime", "tauri-runtime-wry"):
            resolved = [p["version"] for p in packages if p.get("name") == component]
            if len(resolved) != 1 or not EXACT_NPM.fullmatch(resolved[0]):
                raise VersionDenied("resolved_tauri_component_missing_or_ambiguous:" + component)
            version = EXACT_NPM.fullmatch(resolved[0])
            if version.group(1, 2) != minor:
                raise VersionDenied("resolved_tauri_runtime_major_minor_mismatch:" + component)
            if component == "tauri" and resolved[0] != rust_spec[1:]:
                raise VersionDenied("resolved_tauri_core_patch_drift")
    installed = root / "ide/app/node_modules/@tauri-apps/api/package.json"
    if installed.exists():
        try:
            actual = json.loads(installed.read_text(encoding="utf-8"))["version"]
        except (OSError, TypeError, ValueError, KeyError) as e:
            raise VersionDenied("installed_api_version_unreadable") from e
        if actual != api_spec:
            raise VersionDenied("installed_api_version_differs_from_source_pin")
    return {"rust": rust_spec, "api": api_spec, "minor": ".".join(minor),
            "nativeInstallerGatesPreserved": True}

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--repo-root", required=True, type=Path)
    args = p.parse_args()
    try:
        info = check_alignment(args.repo_root)
    except VersionDenied as e:
        print("DENIED: " + str(e), file=sys.stderr)
        return 2
    print(json.dumps(info, sort_keys=True))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
