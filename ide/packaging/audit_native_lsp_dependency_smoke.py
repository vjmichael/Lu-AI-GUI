#!/usr/bin/env python3
"""DEV-02.6A: inspect actual fixed native LSP archive dependencies and run
only fixed --version probes in disposable CI. No source project is opened,
no arbitrary command is run, and no Lu protected-worker approval is implied.

This is *not* a complete release dependency closure. In particular, a
successful version probe cannot prove diagnostics/LSP, LLVM resource files,
minimum supported glibc, original full license graph or OS worker custody.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import signal
import struct
import subprocess
import sys
import tarfile
import urllib.parse
import zipfile

from review_upstream_lsp_archives import (
    MAX_ARCHIVE, checked_manifest, download_locked, sha256,
)

SCHEMA = "lu.ide.dev02.native-lsp-closure-smoke.v1"
MAX_ENTRIES = 10000
MAX_FILE = 512 * 1024 * 1024
MAX_STAGED = 1100 * 1024 * 1024
MAX_PROBE_SECONDS = 15
SYSTEM_DLLS = {
    "ntdll.dll", "kernel32.dll", "kernelbase.dll", "advapi32.dll",
    "user32.dll", "gdi32.dll", "gdi32full.dll", "shell32.dll",
    "ole32.dll", "oleaut32.dll", "comdlg32.dll", "comctl32.dll",
    "shlwapi.dll", "rpcrt4.dll", "secur32.dll", "bcrypt.dll",
    "crypt32.dll", "ws2_32.dll", "winmm.dll", "version.dll",
    "psapi.dll", "dbghelp.dll", "iphlpapi.dll", "setupapi.dll",
    "dwmapi.dll", "imm32.dll", "uxtheme.dll", "msvcrt.dll",
    "ucrtbase.dll", "normaliz.dll", "dnsapi.dll", "cfgmgr32.dll",
    "userenv.dll", "netapi32.dll", "wldap32.dll", "powrprof.dll",
    "propsys.dll", "urlmon.dll", "winhttp.dll",
}


def checked_relative(raw: str) -> tuple[str, ...]:
    if (not isinstance(raw, str) or not raw or "\\" in raw
            or raw.startswith("/") or "\x00" in raw or ":" in raw
            or len(raw) > 1024):
        raise ValueError("unsafe original archive path")
    parts = PurePosixPath(raw).parts
    if not parts or ".." in parts or any(p in ("", ".") for p in parts):
        raise ValueError("original archive traversal")
    return tuple(parts)


def wanted_native(raw: str, expected: str) -> bool:
    base = PurePosixPath(raw).name.lower()
    if base == expected.lower():
        return True
    return base.endswith(".dll") or (".so" in base and
           (base.endswith(".so") or ".so." in base))


def copy_limited(reader, destination: Path, budget: list[int]) -> tuple[str, int]:
    destination.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    total = 0
    with destination.open("xb") as output:
        while chunk := reader.read(1024 * 1024):
            total += len(chunk)
            budget[0] += len(chunk)
            if total > MAX_FILE or budget[0] > MAX_STAGED:
                raise ValueError("native archive expanded beyond fixed budget")
            digest.update(chunk)
            output.write(chunk)
    if total == 0:
        raise ValueError("empty native asset")
    return digest.hexdigest(), total


def stage_archive(archive: Path, kind: str, expected: str, root: Path) -> dict:
    """Preserve executable and same-original-archive native library layout.

    Never use extractall or install an executable in the Lu application.
    Resource metadata is noted but resources are NOT part of this phase.
    """
    if root.exists() or root.is_symlink():
        raise ValueError("native probe root must be absent")
    root.mkdir(parents=True, mode=0o700)
    seen: set[str] = set()
    copied: dict[str, dict] = {}
    resource_files = 0
    budget = [0]

    def item(name: str, reader, length: int):
        nonlocal resource_files
        parts = checked_relative(name)
        normalized = "/".join(parts)
        if normalized in seen:
            raise ValueError("duplicate original native asset path")
        seen.add(normalized)
        if len(seen) > MAX_ENTRIES or length < 0 or length > MAX_FILE:
            raise ValueError("unbounded original archive member")
        if ("lib/clang/" in normalized.lower()
                or "/resource/" in normalized.lower()):
            resource_files += 1
        if not wanted_native(normalized, expected):
            return
        path = root.joinpath(*parts)
        if path.exists() or path.is_symlink():
            raise ValueError("native archive path alias")
        digest, size = copy_limited(reader, path, budget)
        if length and size != length:
            raise ValueError("native archived size mismatch")
        copied[normalized] = {"sha256": digest, "bytes": size}
        if os.name != "nt":
            path.chmod(0o700)

    if kind == "zip":
        with zipfile.ZipFile(archive) as zipped:
            infos = zipped.infolist()
            if len(infos) > MAX_ENTRIES:
                raise ValueError("excessive zip member count")
            for info in infos:
                parts = checked_relative(info.filename)
                if info.is_dir():
                    continue
                mode = (info.external_attr >> 16) & 0o170000
                if mode == 0o120000:
                    raise ValueError("linked upstream asset cannot be staged")
                if wanted_native(info.filename, expected):
                    with zipped.open(info) as reader:
                        item(info.filename, reader, info.file_size)
                else:
                    item(info.filename, io.BytesIO(b""), info.file_size)
    elif kind == "tar.gz":
        with tarfile.open(archive, "r:gz") as zipped:
            members = zipped.getmembers()
            if len(members) > MAX_ENTRIES:
                raise ValueError("excessive tar member count")
            for member in members:
                parts = checked_relative(member.name)
                if member.isdir():
                    continue
                if member.issym() or member.islnk() or not member.isfile():
                    raise ValueError("linked or special upstream member denied")
                if wanted_native(member.name, expected):
                    source = zipped.extractfile(member)
                    if source is None:
                        raise ValueError("missing native tar member")
                    with source:
                        item(member.name, source, member.size)
                else:
                    item(member.name, io.BytesIO(b""), member.size)
    elif kind == "gzip":
        with gzip.open(archive, "rb") as reader:
            item(expected, reader, 0)
    else:
        raise ValueError("unknown original archive kind")

    matches = [name for name in copied
               if PurePosixPath(name).name == expected]
    if len(matches) != 1:
        raise ValueError("expected exactly one original executable")
    return {
        "main": root.joinpath(*matches[0].split("/")),
        "files": copied,
        "resourceEntriesNotBundled": resource_files,
        "stagedBytes": budget[0],
    }


def _number(raw: bytes, pos: int, width: int) -> int:
    if pos < 0 or pos + width > len(raw):
        raise ValueError("truncated PE dependency metadata")
    return int.from_bytes(raw[pos:pos + width], "little")


def pe_imports(raw: bytes) -> list[str]:
    """Read PE import and delay-import tables without loading the binary.

    Reject malformed tables instead of manufacturing an empty dependency list.
    """
    if len(raw) < 64 or raw[:2] != b"MZ":
        raise ValueError("not a Windows PE executable")
    pe = _number(raw, 0x3c, 4)
    if pe < 64 or pe + 24 > len(raw) or raw[pe:pe + 4] != b"PE\x00\x00":
        raise ValueError("invalid PE header")
    coff = pe + 4
    section_count = _number(raw, coff + 2, 2)
    optional_size = _number(raw, coff + 16, 2)
    if not 1 <= section_count <= 96:
        raise ValueError("invalid PE sections")
    optional = coff + 20
    if optional + optional_size > len(raw):
        raise ValueError("truncated PE optional header")
    magic = _number(raw, optional, 2)
    if magic == 0x20b:
        table_offset = 112
    elif magic == 0x10b:
        table_offset = 96
    else:
        raise ValueError("unknown PE optional header")
    section_header = optional + optional_size
    if section_header + section_count * 40 > len(raw):
        raise ValueError("truncated PE section table")
    sections = []
    for i in range(section_count):
        off = section_header + i * 40
        sections.append((_number(raw, off + 12, 4),
                         max(_number(raw, off + 8, 4),
                             _number(raw, off + 16, 4)),
                         _number(raw, off + 20, 4),
                         _number(raw, off + 16, 4)))

    def offset(rva: int) -> int:
        for start, virt_size, file_pos, raw_size in sections:
            if start <= rva < start + virt_size:
                delta = rva - start
                if delta >= raw_size or file_pos + delta >= len(raw):
                    break
                return file_pos + delta
        raise ValueError("PE import references unmapped image bytes")

    def read_name(rva: int) -> str:
        p = offset(rva)
        end = raw.find(b"\x00", p, min(len(raw), p + 129))
        if end == -1 or end == p:
            raise ValueError("invalid native dependency name")
        name = raw[p:end].decode("ascii", errors="strict").lower()
        if not re.fullmatch(r"[a-z0-9._+-]{1,128}", name):
            raise ValueError("invalid PE dependency basename")
        return name

    result = set()
    for directory, stride in ((1, 20), (13, 32)):
        # PE import directory and delay-import directory.
        if optional_size < table_offset + (directory + 1) * 8:
            continue
        rva = _number(raw, optional + table_offset + directory * 8, 4)
        size = _number(raw, optional + table_offset + directory * 8 + 4, 4)
        if rva == 0 or size == 0:
            continue
        start = offset(rva)
        saw_terminator = False
        for index in range(min(512, max(1, size // stride + 1))):
            here = start + index * stride
            if here + stride > len(raw):
                raise ValueError("truncated dependency import descriptor")
            descriptor = raw[here:here + stride]
            if not any(descriptor):
                saw_terminator = True
                break
            if directory == 13 and not (_number(descriptor, 0, 4) & 1):
                raise ValueError("non-RVA delay import requires separate analysis")
            name_rva = _number(descriptor, 12 if directory == 1 else 4, 4)
            result.add(read_name(name_rva))
        if not saw_terminator:
            raise ValueError("unterminated PE import table")
    return sorted(result)


def static_pe_report(path: Path, root: Path) -> dict:
    observed = pe_imports(path.read_bytes())
    siblings = {p.name.lower(): p for p in path.parent.iterdir()
                if p.is_file() and p.suffix.lower() == ".dll"}
    elsewhere = {p.name.lower() for p in root.rglob("*.dll")
                 if p.is_file()}
    return {
        "importedNames": observed,
        "archiveSameDirectory": sorted(x for x in observed if x in siblings),
        "archiveElsewhereUnverified": sorted(x for x in observed
                                          if x not in siblings and x in elsewhere),
        "osCandidateUnverified": sorted(x for x in observed if
            x not in elsewhere and (x in SYSTEM_DLLS or
            x.startswith("api-ms-win-") or x.startswith("ext-ms-"))),
        "externalUnresolved": sorted(x for x in observed if
            x not in elsewhere and x not in SYSTEM_DLLS and
            not x.startswith(("api-ms-win-", "ext-ms-"))),
    }


def static_elf_report(path: Path, root: Path) -> dict:
    if path.read_bytes()[:4] != b"\x7fELF":
        raise ValueError("original Linux executable is not ELF")
    dynamic = subprocess.run(
        ["readelf", "-d", str(path)], capture_output=True, text=True,
        timeout=12, check=False
    )
    if dynamic.returncode:
        raise ValueError("ELF dynamic table inspection failed")
    required = sorted(set(re.findall(
        r"\(NEEDED\)\s+Shared library:\s+\[([^\]]{1,128})\]",
        dynamic.stdout
    )))
    paths = re.findall(r"\((?:RPATH|RUNPATH)\).*?\[([^\]]*)\]", dynamic.stdout)
    version = subprocess.run(
        ["readelf", "--version-info", str(path)],
        capture_output=True, text=True, timeout=12, check=False
    )
    if version.returncode:
        raise ValueError("ELF version table inspection failed")
    glibc = sorted(set(re.findall(r"\bGLIBC_[0-9]+\.[0-9]+(?:\.[0-9]+)?\b",
                                version.stdout)))
    local = {p.name for p in root.rglob("*") if p.is_file()}
    return {
        "neededNames": required,
        "archiveNamesButLoaderPathUnverified": sorted(x for x in required if x in local),
        "externalOsAndRuntimeUnverified": sorted(x for x in required if x not in local),
        "originalRpaths": paths[:20],
        "glibcSymbolRequirements": glibc,
    }


def version_probe(executable: Path, ident: str) -> dict:
    cmd = [str(executable), "--version"]
    kwargs = {"stdin": subprocess.DEVNULL, "stdout": subprocess.PIPE,
              "stderr": subprocess.PIPE, "cwd": str(executable.parent),
              "shell": False}
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        kwargs["start_new_session"] = True
    proc = subprocess.Popen(cmd, **kwargs)
    forced = False
    try:
        stdout, stderr = proc.communicate(timeout=MAX_PROBE_SECONDS)
    except subprocess.TimeoutExpired:
        forced = True
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, timeout=8, check=False)
        else:
            os.killpg(proc.pid, signal.SIGKILL)
        stdout, stderr = proc.communicate(timeout=10)
    combined = (stdout + b"\n" + stderr)[:4096]
    marker = {"clangd": b"clangd", "rust-analyzer": b"rust-analyzer",
              "ruff": b"ruff"}[ident]
    accepted = not forced and proc.returncode == 0 and (
        marker in combined.lower()
    )
    return {
        "versionMarkerFound": marker in combined.lower(),
        "exitedZero": proc.returncode == 0,
        "forcedTermination": forced,
        "reaped": proc.poll() is not None,
        "versionOutputSha256": hashlib.sha256(stdout + stderr).hexdigest(),
        "versionProbePassed": accepted,
    }


def inspect(manifest: dict, archive_root: Path, output: Path,
            platform: str, *, fetch: bool = False) -> dict:
    rows = checked_manifest(manifest)
    choices = [(name, info) for name, target, info in rows if target == platform]
    if len(choices) != 3:
        raise ValueError("native OS requires exactly three pinned providers")
    if output.exists() or output.is_symlink():
        raise ValueError("CI-only fresh output directory required")
    output.mkdir(parents=True, mode=0o700)
    packet = {
        "schema": SCHEMA, "platform": platform, "providers": [],
        "originalPinnedAssetsVerified": False,
        "allVersionProbesPassed": False, "releaseApproved": False,
        "redistributionApproved": False, "protectedLuWorkerTested": False,
        "genuineInstalledNsisOrDeb": False,
        "fullDependencyClosureApproved": False,
        "fullLLVMResourcesReviewed": False,
    }
    for ident, item in choices:
        entry = {"provider": ident, "archiveSha256": item["sha256"],
                 "originalArchiveVerified": False, "versionProbePassed": False}
        packet["providers"].append(entry)
        try:
            archive = archive_root / item["name"]
            if archive.is_symlink():
                raise ValueError("symlinked upstream archive")
            if not archive.is_file():
                if not fetch:
                    raise ValueError("original archive absent in offline mode")
                archive_root.mkdir(parents=True, exist_ok=True)
                download_locked(item["url"], archive)
            if not 0 < archive.stat().st_size <= MAX_ARCHIVE:
                raise ValueError("archive outside original compressed size bounds")
            if sha256(archive) != item["sha256"]:
                raise ValueError("original archive SHA256 mismatch")
            entry["originalArchiveVerified"] = True
            staged = stage_archive(
                archive, item["kind"], item["binary"], output / ident
            )
            main = staged["main"]
            entry["originalStagedFiles"] = staged["files"]
            entry["resourceEntriesNotBundled"] = staged["resourceEntriesNotBundled"]
            entry["staticDependencyInspection"] = (
                static_pe_report(main, output / ident)
                if platform == "windows-x86_64" else
                static_elf_report(main, output / ident)
            )
            entry.update(version_probe(main, ident))
        except (OSError, ValueError, subprocess.SubprocessError,
                UnicodeError, zipfile.BadZipFile, tarfile.TarError,
                gzip.BadGzipFile) as exc:
            # Preserve failure classes, never raw paths or tool output.
            entry["failureClass"] = type(exc).__name__
    packet["originalPinnedAssetsVerified"] = all(
        p["originalArchiveVerified"] for p in packet["providers"]
    )
    packet["allVersionProbesPassed"] = all(
        p["versionProbePassed"] for p in packet["providers"]
    )
    with (output / "candidate-dependency-smoke.json").open(
        "x", encoding="utf-8"
    ) as handle:
        json.dump(packet, handle, sort_keys=True, indent=2)
        handle.write("\n")
    return packet


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--manifest", required=True, type=Path)
    p.add_argument("--archives", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    p.add_argument("--platform", required=True,
                   choices=("windows-x86_64", "linux-x86_64"))
    p.add_argument("--fetch-locked", action="store_true")
    a = p.parse_args()
    try:
        runner = Path(os.environ.get("RUNNER_TEMP", ""))
        if (os.environ.get("GITHUB_ACTIONS") != "true"
                or not runner.is_dir()
                or not a.output.resolve().is_relative_to(runner.resolve())
                or a.output.exists() or a.output.is_symlink()
                or (a.platform.startswith("windows") != (os.name == "nt"))):
            raise ValueError("only matching native disposable CI output allowed")
        result = inspect(json.loads(a.manifest.read_text()), a.archives,
                         a.output, a.platform, fetch=a.fetch_locked)
    except (ValueError, OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print("DENIED: native LSP candidate static/probe intake: " +
              type(exc).__name__, file=sys.stderr)
        return 2
    if not (result["originalPinnedAssetsVerified"] and
            result["allVersionProbesPassed"]):
        print("DEV02_NATIVE_PROVIDER_VERSION_PROBES=FAILED_REVIEW_REQUIRED")
        return 1
    print("DEV02_REAL_THREE_PINNED_NATIVE_PROVIDER_VERSION_PROBES=PASS")
    print("DEV02_REAL_INSTALLED_PROTECTED_LU_AND_FULL_DEPENDENCY_APPROVAL=OPEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
