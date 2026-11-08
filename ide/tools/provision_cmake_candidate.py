#!/usr/bin/env python3
"""CI-only, hash-verified CMake/Ninja/LLVM offline candidate provisioner.

Inputs are exact upstream GitHub release archives with SHA-256 values recorded
in ide/runtime-pins/upstream_sources.json. They are downloaded ONLY on the
build machine, never in installed Lu or a protected OS worker.
Candidate success proves staging/smoke, not signature/legal/OS acceptance.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import sys
import tarfile
import urllib.request
import zipfile

from package_reviewed_runtime import package, Refused

HERE=Path(__file__).resolve().parent
LOCK=HERE.parent / "runtime-pins" / "upstream_sources.json"
LICENSE_NINJA=HERE / "runtime_licenses" / "ninja-1.13.0-COPYING"
DRIVER=HERE / "runtime_drivers" / "dev03-cmake.cmake"
MAX_ARCHIVE=3*1024**3
MAX_EXTRACTED=3*1024**3


def locked_sources(platform: str) -> dict:
    data=json.loads(LOCK.read_text())
    if data["schemaVersion"]!=1 or data["releaseBundleApproved"] is not False:
        raise Refused("unexpected source lock authority")
    if platform not in ("windows-x64","linux-x64"):
        raise Refused("unsupported CMake platform")
    return data["platforms"][platform]


def fetch_locked(cache: Path, info: dict) -> Path:
    name=info["file"]
    url=info["url"]
    sha=info["sha256"]
    expected=int(info["size"])
    if not name or "/" in name or "\\" in name or not url.startswith(
        "https://github.com/"
    ) or len(sha)!=64 or expected<=0 or expected>MAX_ARCHIVE:
        raise Refused("invalid reviewed source archive record")
    cache.mkdir(parents=True,exist_ok=True)
    target=cache/name
    if not target.exists():
        temp=target.with_suffix(target.suffix+".partial")
        if temp.exists():
            raise Refused("stale partial source archive")
        try:
            with urllib.request.urlopen(url,timeout=120) as incoming, \
                temp.open("xb") as dst:
                while chunk:=incoming.read(1024*1024):
                    dst.write(chunk)
                    if dst.tell()>expected:
                        raise Refused("upstream archive oversize")
            temp.rename(target)
        finally:
            if temp.exists():
                temp.unlink()
    h=hashlib.sha256()
    total=0
    with target.open("rb") as archive:
        while chunk:=archive.read(1024*1024):
            total+=len(chunk)
            if total>expected:
                raise Refused("upstream archive changed")
            h.update(chunk)
    if total!=expected or h.hexdigest()!=sha:
        raise Refused("upstream artifact digest mismatch")
    return target


def archive_relative(name: str) -> str | None:
    path=PurePosixPath(name)
    if path.is_absolute() or "\\" in name or any(
        item in ("",".","..") for item in path.parts
    ) or len(path.parts)<2:
        return None
    return "/".join(path.parts[1:])


def allow_component(category: str, rel: str) -> bool:
    if category=="cmake":
        return rel in ("bin/cmake","bin/ctest","bin/cmake.exe","bin/ctest.exe") \
            or rel.startswith(("share/","lib/")) \
            or rel.lower().endswith("/copyright.txt")
    if category=="llvm":
        name=PurePosixPath(rel).name.lower()
        return (rel.startswith("bin/") and (
            name.startswith(("clang","lld","ld.lld")) or name.endswith(".dll")
        )) or rel.startswith(("lib/clang/","lib/libllvm","lib/libclang",
                             "include/clang/")) \
            or name in ("license.txt","license.md")
    if category=="ninja":
        return rel in ("ninja","ninja.exe")
    return False


def add_entry(stage: Path,category: str,rel: str,source,remaining: list[int]):
    if not allow_component(category,rel):
        return
    if category=="ninja":
        rel="bin/ninja.exe" if rel.endswith(".exe") else "bin/ninja"
    if rel.lower().endswith("/copyright.txt") and category=="cmake":
        rel="LICENSES/cmake.txt"
    if category=="llvm" and PurePosixPath(rel).name.lower() in (
        "license.txt","license.md"
    ):
        rel="LICENSES/llvm.txt"
    target=stage.joinpath(*PurePosixPath(rel).parts)
    if target.exists():
        if rel.startswith("LICENSES/"):
            return
        raise Refused("duplicate runtime staging path")
    target.parent.mkdir(parents=True,exist_ok=True)
    with target.open("xb") as dest:
        while chunk:=source.read(65536):
            remaining[0]-=len(chunk)
            if remaining[0]<0:
                raise Refused("unpacked runtime tree exceeds budget")
            dest.write(chunk)
    if rel.startswith("bin/"):
        target.chmod(target.stat().st_mode|0o755)


def extract_zip(stage:Path,path:Path,category:str,remaining:list[int]):
    with zipfile.ZipFile(path) as zip:
        for info in zip.infolist():
            if info.is_dir():continue
            mode=(info.external_attr>>16)&0o170000
            if mode==0o120000:
                raise Refused("runtime archive symlink")
            rel=archive_relative(info.filename)
            if rel is None:
                # Ninja release contains only top-level "ninja" or
                # "ninja.exe" rather than a version-root directory.
                if category=="ninja" and info.filename in ("ninja","ninja.exe"):
                    rel=info.filename
                else:continue
            with zip.open(info) as source:
                add_entry(stage,category,rel,source,remaining)


def extract_tar(stage:Path,path:Path,category:str,remaining:list[int]):
    if path.name.endswith(".tar.zst"):
        try:
            import zstandard
        except ImportError as exc:
            raise Refused("build machine requires reviewed zstandard extractor") from exc
        with path.open("rb") as packed:
            stream=zstandard.ZstdDecompressor().stream_reader(packed)
            with tarfile.open(fileobj=stream,mode="r|") as tar:
                for entry in tar:
                    if entry.issym() or entry.islnk():
                        # All symlinks are replaced with separate regular
                        # copies from known selected binaries after unpack.
                        continue
                    if not entry.isfile():continue
                    rel=archive_relative(entry.name)
                    if rel and allow_component(category,rel):
                        file=tar.extractfile(entry)
                        if file is None: raise Refused("truncated LLVM archive")
                        with file:
                            add_entry(stage,category,rel,file,remaining)
    else:
        with tarfile.open(path,mode="r:gz") as tar:
            for entry in tar:
                if entry.issym() or entry.islnk():
                    continue
                if not entry.isfile():continue
                rel=archive_relative(entry.name)
                if rel and allow_component(category,rel):
                    file=tar.extractfile(entry)
                    if file is None: raise Refused("truncated CMake archive")
                    with file:
                        add_entry(stage,category,rel,file,remaining)


def provision(stage:Path,cache:Path,platform:str) -> None:
    pins=locked_sources(platform)
    stage.mkdir(parents=True,exist_ok=False)
    remaining=[MAX_EXTRACTED]
    for category in ("cmake","ninja","llvm"):
        source=fetch_locked(cache,pins[category])
        if source.suffix==".zip":
            extract_zip(stage,source,category,remaining)
        else:
            extract_tar(stage,source,category,remaining)
    (stage/"LICENSES").mkdir(exist_ok=True)
    shutil.copy2(LICENSE_NINJA,stage/"LICENSES/ninja.txt")
    (stage/"drivers").mkdir(exist_ok=True)
    shutil.copy2(DRIVER,stage/"drivers/dev03-cmake.cmake")
    # Some official Clang distributions store an executable as a symlink
    # to the core compiler. Materialize separate regular copies, never links.
    win=platform=="windows-x64"
    suffix=".exe" if win else ""
    primary=stage/"bin"/("clang"+suffix)
    companion=stage/"bin"/("clang++"+suffix)
    if not primary.is_file():
        choices=list((stage/"bin").glob("clang-*" + suffix))
        if len(choices)!=1:
            raise Refused("pinned archive does not contain one Clang compiler")
        shutil.copy2(choices[0],primary)
    if not companion.is_file():
        shutil.copy2(primary,companion)
    required=["bin/cmake"+suffix,"bin/ctest"+suffix,"bin/ninja"+suffix,
              "bin/clang"+suffix,"bin/clang++"+suffix,
              "LICENSES/cmake.txt","LICENSES/ninja.txt","LICENSES/llvm.txt",
              "drivers/dev03-cmake.cmake"]
    for rel in required:
        if not (stage/rel).is_file():
            raise Refused("required pinned runtime file missing: "+rel)
    for command in [
        [str(stage/"bin"/("cmake"+suffix)),"--version"],
        [str(stage/"bin"/("ctest"+suffix)),"--version"],
        [str(stage/"bin"/("ninja"+suffix)),"--version"],
        [str(stage/"bin"/("clang"+suffix)),"--version"],
    ]:
        result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                              timeout=35,check=True)
        if not result.stdout:
            raise Refused("staged compiler/runtime returned empty version")


def main():
    cli=argparse.ArgumentParser()
    cli.add_argument("--stage",type=Path,required=True)
    cli.add_argument("--cache",type=Path,required=True)
    cli.add_argument("--output",type=Path,required=True)
    args=cli.parse_args()
    platform="windows-x64" if sys.platform=="win32" else "linux-x64"
    try:
        provision(args.stage,args.cache,platform)
        receipt=package(args.stage,args.output,"cpp_cmake_build",platform,None,True)
        print("DEV03_CMAKE_CANDIDATE_TREE_SHA256="+receipt["runtimeTreeSha256"])
        print("DEV03_CMAKE_CANDIDATE_ONLY_NOT_NATIVE_OS_ACCEPTED=true")
    except (Refused,OSError,subprocess.CalledProcessError,
            subprocess.TimeoutExpired,tarfile.TarError,zipfile.BadZipFile) as exc:
        print("DEV03_CMAKE_CANDIDATE_DENIED="+str(exc),file=sys.stderr)
        return 2
    return 0


if __name__=="__main__":
    sys.exit(main())
