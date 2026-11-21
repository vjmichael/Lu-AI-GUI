#!/usr/bin/env python3
"""CI-only Python 3.13.5 + pytest 8.4.1 offline CANDIDATE builder.

Uses a version-selected build-machine interpreter; copies its standard library,
DLLs/shared libraries and a pinned-version pytest dependency closure into a
private staging tree. Candidate hashes are evidence, not release approval.
No pip/npm download happens in the installed Lu IDE or protected worker.
"""
from __future__ import annotations
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
from package_reviewed_runtime import package, Refused

PYTHON = (3, 13, 5)
PYTEST = "8.4.1"
# Exact wheel *versions* are constrained at packaging time, but independent
# wheel hashes/redistribution rights still require an explicit later review.
REQUIREMENTS = [
    f"pytest=={PYTEST}",
    "pluggy==1.6.0",
    "iniconfig==2.1.0",
    "packaging==25.0",
    "pygments==2.19.2",
]


def copytree(source: Path, dest: Path):
    def exclude(_folder: str, names: list[str]):
        return {n for n in names if n in {
            "__pycache__", "site-packages", "test", "tests", "idlelib",
            "tkinter", "turtledemo", "ensurepip", ".git",
        } or n.endswith(".pyc") or n.endswith(".pyo")}
    shutil.copytree(source, dest, symlinks=False, ignore=exclude)


def provision(stage: Path) -> tuple[str, str]:
    if tuple(sys.version_info[:3]) != PYTHON:
        raise Refused("unexpected build-machine Python version")
    stage.mkdir(parents=True, exist_ok=False)
    (stage / "bin").mkdir()
    (stage / "LICENSES").mkdir()
    prefix = Path(sys.base_prefix).resolve(strict=True)
    installed_exe = Path(sys.executable).resolve(strict=True)
    windows = sys.platform == "win32"
    target = stage / "bin" / ("python.exe" if windows else "python3")
    shutil.copy2(installed_exe, target)
    if windows:
        for name in ["python313.dll", "python3.dll", "vcruntime140.dll"]:
            candidate = prefix / name
            if candidate.is_file():
                shutil.copy2(candidate, stage / "bin" / name)
        dlls = prefix / "DLLs"
        if not dlls.is_dir():
            raise Refused("missing Python Windows DLLs")
        copytree(dlls, stage / "DLLs")
        std = prefix / "Lib"
        if not std.is_dir():
            raise Refused("missing Python Windows standard library")
        copytree(std, stage / "Lib")
        site = stage / "Lib" / "site-packages"
        site.mkdir()
        # Windows pythonXY._pth permits explicit relocated standard-library
        # paths even with Python -I isolated mode. Never import user PYTHONPATH.
        (stage / "bin" / "python313._pth").write_text(
            "..\\Lib\n..\\Lib\\site-packages\n.\\\nimport site\n")
    else:
        std = prefix / "lib" / "python3.13"
        if not std.is_dir():
            raise Refused("missing Linux Python 3.13 standard library")
        copytree(std, stage / "lib" / "python3.13")
        site = stage / "lib" / "python3.13" / "site-packages"
        site.mkdir()
        # Copy shared libpython used by the setup-python interpreter; do not
        # inherit it from an unrelated user-controlled host location.
        for source in sorted((prefix / "lib").glob("libpython3.13.so*")):
            if source.is_file():
                shutil.copy2(source, stage / "lib" / source.name)
    licenses = [
        prefix / "LICENSE.txt",
        prefix / "LICENSE",
        prefix / "LICENSE.md",
    ]
    python_license = next((p for p in licenses if p.is_file()), None)
    if python_license is None:
        raise Refused("Python distribution license not found")
    shutil.copy2(python_license, stage / "LICENSES/python.txt")

    # Network only during CI creation of CANDIDATE. The generated file tree
    # is fully archived and cannot pip install once the worker runs.
    subprocess.run(
        [sys.executable, "-m", "pip", "--isolated", "install", "--disable-pip-version-check",
         "--no-input", "--no-cache-dir", "--target",
         str(site), *REQUIREMENTS],
        check=True, timeout=180,
    )
    license_files = sorted(site.glob("pytest-*.dist-info/licenses/*")) + \
        sorted(site.glob("pytest-*.dist-info/LICENSE*"))
    license_files = [p for p in license_files if p.is_file()]
    if not license_files:
        raise Refused("pytest license missing from wheel distribution")
    shutil.copy2(license_files[0], stage / "LICENSES/pytest.txt")

    # Do not ship anything unless the relocated binary can actually import
    # pytest from the stage with PATH/PYTHONPATH ignored.
    command = [str(target), "-I", "-m", "pytest", "--version"]
    smoke = subprocess.run(command, text=True, capture_output=True,
                           timeout=30, check=True)
    if PYTEST not in (smoke.stdout + smoke.stderr):
        raise Refused("relocated offline pytest version mismatch")
    return f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}", PYTEST


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--stage",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    opts=parser.parse_args()
    try:
        python_version,pytest_version = provision(opts.stage)
        platform="windows-x64" if sys.platform=="win32" else "linux-x64"
        receipt=package(opts.stage,opts.output,"python_pytest",platform,None,True)
        print("DEV03_PYTHON_RUNTIME_CANDIDATE="+python_version)
        print("DEV03_PYTEST_RUNTIME_CANDIDATE="+pytest_version)
        print("DEV03_PYTHON_CANDIDATE_TREE_SHA256="+receipt["runtimeTreeSha256"])
        print("DEV03_RUNTIME_PIN_APPROVAL=OPEN")
    except (Refused,OSError,subprocess.CalledProcessError,subprocess.TimeoutExpired) as error:
        print("DEV03_PYTHON_CANDIDATE_DENIED="+str(error),file=sys.stderr)
        return 2
    return 0


if __name__=="__main__":
    sys.exit(main())
