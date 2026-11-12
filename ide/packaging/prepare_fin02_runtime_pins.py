#!/usr/bin/env python3
"""FIN-02C: offline verifier and manifest writer for *real* staged native bytes.

This never installs, signs, promotes or invokes a runtime. It requires an
independently prepared exact-pin inventory and a complete staged/installed
tree, rehashes the native custody digests, and writes compile pins once.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
import stat
from pathlib import Path

PIN = re.compile(r"[a-f0-9]{64}\Z")
MAX_FILES = 40000
MAX_BYTES = 4 * 1024 ** 3
MAX_FILE = 1024 ** 3
MAX_DEPTH = 32

WINDOWS = {
    "LU_IDE_WO6_WINDOWS_CARGO_BUNDLE_SHA256": ("resources/build-tools/windows-rust-offline", "win_tree"),
    "LU_IDE_DEV03_WINDOWS_PYTHON_SHA256": ("resources/build-tools/windows-runtimes/python_pytest", "runtime"),
    "LU_IDE_DEV03_WINDOWS_NODE_SHA256": ("resources/build-tools/windows-runtimes/js_tests", "runtime"),
    "LU_IDE_DEV03_WINDOWS_CMAKE_SHA256": ("resources/build-tools/windows-runtimes/cpp_cmake_build", "runtime"),
    "LU_IDE_WO6_WINDOWS_LLDB_DAP_SHA256": ("resources/debug-adapters/lldb-dap", "win_tree"),
    "LU_IDE_WO6_WINDOWS_D7D_SERVICE_SHA256": ("resources/recovery/windows_recovery_service.exe", "file"),
}
LINUX = {
    "LU_IDE_WO6_LINUX_TOOLCHAIN_SHA256": (None, "linux_cargo"),
    "LU_IDE_DEV03_LINUX_PYTHON_SHA256": ("/opt/lu-ide/runtimes/python_pytest", "runtime"),
    "LU_IDE_DEV03_LINUX_NODE_SHA256": ("/opt/lu-ide/runtimes/js_tests", "runtime"),
    "LU_IDE_DEV03_LINUX_CMAKE_SHA256": ("/opt/lu-ide/runtimes/cpp_cmake_build", "runtime"),
    "LU_IDE_WO6_LINUX_LLDB_DAP_SHA256": ("/opt/lu-ide/debug-adapters/lldb-dap", "linux_bundle"),
}
LINUX_UV = {
    "LU_IDE05_LINUX_UV_SHA256": ("/opt/lu-ide/runtimes/uv_sync_offline", "runtime"),
}
REQUIRED = {
    "uv_sync_offline": {
        "linux": ["bin/uv", "bin/python3", "LICENSES/uv.txt", "LICENSES/python.txt"],
    },
    "python_pytest": {
        "windows": ["bin/python.exe", "Lib/site-packages/pytest/__init__.py", "LICENSES/python.txt", "LICENSES/pytest.txt"],
        "linux": ["bin/python3", "lib/python3.13/site-packages/pytest/__init__.py", "LICENSES/python.txt", "LICENSES/pytest.txt"],
    },
    "js_tests": {
        "windows": ["bin/node.exe", "LICENSES/node.txt"],
        "linux": ["bin/node", "LICENSES/node.txt"],
    },
    "cpp_cmake_build": {
        "windows": ["bin/cmake.exe", "bin/ctest.exe", "bin/ninja.exe", "bin/clang.exe", "bin/clang++.exe",
                    "LICENSES/cmake.txt", "LICENSES/ninja.txt", "LICENSES/llvm.txt", "drivers/dev03-cmake.cmake"],
        "linux": ["bin/cmake", "bin/ctest", "bin/ninja", "bin/clang", "bin/clang++",
                  "LICENSES/cmake.txt", "LICENSES/ninja.txt", "LICENSES/llvm.txt", "drivers/dev03-cmake.cmake"],
    },
}

class Denied(ValueError):
    pass

def sha_file(path: Path) -> str:
    meta = path.lstat()
    if not stat.S_ISREG(meta.st_mode) or meta.st_size > MAX_FILE or meta.st_nlink != 1:
        raise Denied("FIN02_FILE_NOT_REGULAR_OR_UNTRUSTED")
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for part in iter(lambda: handle.read(65536), b""):
            h.update(part)
    if path.lstat().st_size != meta.st_size:
        raise Denied("FIN02_FILE_CHANGED_DURING_REHASH")
    return h.hexdigest()

def entries(root: Path):
    if root.is_symlink() or not root.is_dir():
        raise Denied("FIN02_BUNDLE_ROOT_NOT_DIR")
    files, total = [], 0
    def scan(current: Path, depth: int):
        nonlocal total
        if depth > MAX_DEPTH:
            raise Denied("FIN02_TREE_TOO_DEEP")
        for child in current.iterdir():
            if child.is_symlink():
                raise Denied("FIN02_SYMLINK_DENIED")
            if child.is_dir():
                scan(child, depth+1)
            elif child.is_file():
                rel = child.relative_to(root).as_posix()
                if (not rel.isascii() or len(rel.encode()) > 1024 or
                    any(s in ("", ".", "..") for s in rel.split("/"))):
                    raise Denied("FIN02_PATH_DENIED")
                info = child.lstat()
                total += info.st_size
                if len(files) >= MAX_FILES or total > MAX_BYTES:
                    raise Denied("FIN02_TREE_QUOTA")
                files.append((rel, info.st_size, sha_file(child)))
            else:
                raise Denied("FIN02_SPECIAL_FILE_DENIED")
    scan(root, 0)
    if not files:
        raise Denied("FIN02_EMPTY_TREE")
    return files

def tree_sha(root: Path, kind: str) -> str:
    files = entries(root)
    h = hashlib.sha256()
    if kind == "runtime":
        h.update(b"lu-ide-runtime-tree-v1\x00")
        for rel, size, digest in sorted(files):
            h.update(rel.encode()+b"\x00"+size.to_bytes(8,"little")+
                     digest.encode()+b"\x00")
    elif kind == "linux_bundle":
        for rel, _, digest in sorted(files):
            h.update(rel.encode()+b"\x00"+digest.encode()+b"\x00")
    elif kind == "win_tree":
        # D7 uses depth-first sorted sibling traversal, not path-flat sorting.
        by_name = {rel:(size,digest) for rel,size,digest in files}
        def visit(path: Path):
            for child in sorted(path.iterdir(),key=lambda p:p.name):
                if child.is_dir():
                    visit(child)
                else:
                    rel=child.relative_to(root).as_posix()
                    size,digest=by_name[rel]
                    h.update(rel.encode()+b"\x00"+size.to_bytes(8,"little")+
                             digest.encode()+b"\x00")
        visit(root)
    else:
        raise Denied("FIN02_TREE_KIND_DENIED")
    return h.hexdigest()

def linux_cargo() -> str:
    linker = next((p for p in (
        "/usr/bin/x86_64-linux-gnu-gcc-14",
        "/usr/bin/x86_64-linux-gnu-gcc-13",
        "/usr/bin/x86_64-linux-gnu-gcc-12",
        "/usr/bin/aarch64-linux-gnu-gcc-14",
        "/usr/bin/aarch64-linux-gnu-gcc-13",
        "/usr/bin/aarch64-linux-gnu-gcc-12",
    ) if Path(p).is_file() and not Path(p).is_symlink()), None)
    if linker is None:
        raise Denied("FIN02_ROOT_LINKER_MISSING")
    h=hashlib.sha256()
    for name in ("/usr/bin/bwrap", "/usr/bin/cargo", "/usr/bin/rustc", linker):
        path=Path(name)
        info=path.lstat()
        if (info.st_uid != 0 or info.st_mode & 0o6022 or
            not stat.S_ISREG(info.st_mode) or info.st_size > 32*1024*1024):
            raise Denied("FIN02_ROOT_SYSTEM_TOOL_INVALID")
        h.update(name.encode()+b"\x00"+sha_file(path).encode()+b"\x00")
    return h.hexdigest()

def verify(root: Path, platform: str, expected: dict,
           linux_uv: bool = False) -> dict:
    if platform not in ("windows", "linux"):
        raise Denied("FIN02_PLATFORM_DENIED")
    if linux_uv and platform != "linux":
        raise Denied("FIN05_WINDOWS_UV_ADMISSION_NOT_ACCEPTED")
    spec=WINDOWS if platform=="windows" else dict(LINUX)
    if linux_uv:
        spec.update(LINUX_UV)
    if set(expected) != set(spec):
        raise Denied("FIN02_PIN_SET_NOT_EXACT")
    pins={}
    for key, (location,kind) in spec.items():
        pin=expected[key]
        if not isinstance(pin,str) or PIN.fullmatch(pin) is None or pin == "0"*64:
            raise Denied("FIN02_EXPECTED_SHA_INVALID")
        if kind=="linux_cargo":
            digest=linux_cargo()
        else:
            path=(root/location) if platform=="windows" else Path(location)
            staged = None
            if platform=="linux":
                if kind=="runtime":
                    staged=root/"resources/build-tools/linux-runtimes"/path.name
                elif kind=="linux_bundle":
                    staged=root/"resources/debug-adapters/lldb-dap"
            if kind=="file":
                digest=sha_file(path)
            else:
                if kind=="win_tree" and "CARGO_BUNDLE" in key:
                    for needed in ("bin/cargo.exe","bin/rustc.exe"):
                        if not (path/needed).is_file():
                            raise Denied("FIN02_REQUIRED_CARGO_BINARY_MISSING")
                    for needed in ("lib","cargo-home"):
                        if not (path/needed).is_dir():
                            raise Denied("FIN02_REQUIRED_CARGO_DIRECTORY_MISSING")
                if kind=="runtime":
                    profile=path.name
                    for needed in REQUIRED[profile][platform]:
                        if not (path/needed).is_file():
                            raise Denied("FIN02_REQUIRED_RUNTIME_FILE_MISSING")
                digest=tree_sha(path,kind)
                if staged is not None and tree_sha(staged,kind)!=digest:
                    raise Denied("FIN02_LINUX_PACKAGE_PAYLOAD_DIFFERS_FROM_INSTALLED")
        if digest!=pin:
            raise Denied("FIN02_STAGED_BYTES_DO_NOT_MATCH_REVIEWED_PIN")
        pins[key]=digest
    return {"schema":"lu.ide.fin02.runtime-pins.v1","platform":platform,
        "releaseInstalled":False,"nativeHumanAcceptance":False,
        "executionAuthorized":False,"pins":pins}

def main() -> int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--tauri-dir", type=Path, required=True)
    parser.add_argument("--platform", choices=["windows","linux"], required=True)
    parser.add_argument("--reviewed-pins", type=Path, required=True)
    parser.add_argument("--linux-offline-uv", action="store_true")
    args=parser.parse_args()
    try:
        root=args.tauri_dir.resolve(strict=True)
        review=args.reviewed_pins
        if review.is_symlink() or not review.is_file():
            raise Denied("FIN02_REVIEWED_PIN_INVENTORY_MISSING")
        raw=json.loads(review.read_text(encoding="utf-8"))
        if raw.get("schema")!="lu.ide.fin02.independent-input-pins.v1" or raw.get("platform")!=args.platform:
            raise Denied("FIN02_REVIEWED_INPUT_SCHEMA_DENIED")
        result=verify(root,args.platform,raw.get("pins"),
                      args.linux_offline_uv)
        output=root/"fin02-runtime-pins.json"
        # Never silently replace a reviewed build input or overwrite an
        # artifact already inspected by a separate packager.
        with output.open("x",encoding="utf-8") as handle:
            json.dump(result,handle,sort_keys=True,indent=2)
            handle.write("\n")
        print(json.dumps({"pins":len(result["pins"]),
          "result":"build_stage_pin_input_only",
          "nativeAcceptance":False,"releaseApproved":False},sort_keys=True))
        return 0
    except (Denied,OSError,TypeError,KeyError,json.JSONDecodeError) as err:
        print("DENIED: "+str(err))
        return 1

if __name__=="__main__":
    raise SystemExit(main())
