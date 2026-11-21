#!/usr/bin/env python3
"""Build-machine-only Node 22.16.0 offline candidate; NEVER installer approval.

The CI setup-node action provisions an exact selected Node release, but this
script never treats a downloaded tool or setup action as an approved binary.
It copies the native binary/license, executes a version smoke test and emits
an unapproved candidate zip with a reproducible full-tree checksum.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
from package_reviewed_runtime import package, Refused

VERSION = "v22.16.0"


def stage_node(stage: Path) -> str:
    stage.mkdir(parents=True, exist_ok=False)
    installed = subprocess.run(
        ["node", "-p", "process.execPath"],
        text=True, capture_output=True, check=True, timeout=30,
    ).stdout.strip()
    source = Path(installed).resolve(strict=True)
    if not source.is_file():
        raise Refused("node executable missing")
    version = subprocess.run(
        [str(source), "--version"], text=True, capture_output=True,
        check=True, timeout=30,
    ).stdout.strip()
    if version != VERSION:
        raise Refused("wrong Node version; refusing artifact")
    licenses = [
        source.parent / "LICENSE",
        source.parent.parent / "LICENSE",
        source.parent / "LICENSE.md",
        source.parent.parent / "LICENSE.md",
    ]
    license_file = next((p for p in licenses if p.is_file() and not p.is_symlink()), None)
    if license_file is None:
        raise Refused("source distribution has no verified Node license file")
    (stage / "bin").mkdir()
    (stage / "LICENSES").mkdir()
    dest = stage / "bin" / ("node.exe" if sys.platform == "win32" else "node")
    shutil.copy2(source, dest, follow_symlinks=True)
    shutil.copy2(license_file, stage / "LICENSES/node.txt", follow_symlinks=False)
    smoke = subprocess.run([str(dest), "--version"],
                           text=True, capture_output=True, check=True, timeout=30)
    if smoke.stdout.strip() != VERSION:
        raise Refused("relocated Node executable did not run")
    return version


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        version = stage_node(args.stage)
        platform = "windows-x64" if sys.platform == "win32" else "linux-x64"
        receipt = package(args.stage, args.output, "js_tests", platform, None, True)
        print(f"DEV03_NODE_CANDIDATE_VERSION={version}")
        print(f"DEV03_NODE_CANDIDATE_TREE_SHA256={receipt['runtimeTreeSha256']}")
        print("DEV03_NATIVE_OS_APPROVAL=NOT_PROVEN")
        print("DEV03_INSTALLED_RUNTIME=NOT_APPROVED")
    except (Refused, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        print(f"DEV03_NODE_CANDIDATE_DENIED={error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
