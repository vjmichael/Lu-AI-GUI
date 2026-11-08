#!/usr/bin/env python3
"""DEV-02.6B: preserve complete *original* native LSP release file trees.

This assembles a disposable candidate resource tree, NOT a licensed or
protected production installer. Refuses to pretend a single extracted EXE is a
complete clangd/LLVM distribution. Every original release regular file must be
accounted for, or the candidate fails closed. No arbitrary archive extraction.
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
import shutil
import sys
import tarfile
import tempfile
import zipfile

from collect_original_lsp_notices import validate as validate_notices, check_original
from review_upstream_lsp_archives import checked_manifest, download_locked, sha256, MAX_ARCHIVE

SCHEMA = "lu.ide.dev02.full-original-native-lsp-candidate.v1"
MAX_FILES_WIN = 500
MAX_FILES_LINUX = 1000
MAX_TOTAL_WIN = 512 * 1024 * 1024
MAX_TOTAL_LINUX = 1024 * 1024 * 1024
MAX_FILE_WIN = 256 * 1024 * 1024
MAX_FILE_LINUX = 512 * 1024 * 1024
SAFE_SEGMENT = re.compile(r"^[A-Za-z0-9._+-]{1,200}$")


def safe_parts(path: str) -> tuple[str, ...]:
    if (not isinstance(path, str) or not path or len(path) > 1500
            or path.startswith("/") or "\\" in path or ":" in path
            or "\x00" in path):
        raise ValueError("unsafe original release archive member")
    # PurePosixPath silently normalizes duplicate separators and dot parts:
    # compare each raw segment instead of accepting the normalized form.
    segments = path.split("/")
    if path.endswith("/"):
        segments = segments[:-1]
    if (not segments or len(segments) > 18
            or any(not SAFE_SEGMENT.fullmatch(part) or part in (".","..")
                   for part in segments)):
        raise ValueError("archive traversal or nonportable entry")
    return tuple(segments)


def archive_entries(archive: Path, kind: str, executable: str):
    """Returns a bounded list of (original path, size, open-reader callback).

    Callbacks live only inside the with block in stage_provider(), so this
    function yields the archive handle and checked entries together.
    """
    if kind == "gzip":
        return None, [(executable, 0, None)]
    if kind == "zip":
        opened = zipfile.ZipFile(archive)
        members = opened.infolist()
        entries = []
        for info in members:
            safe_parts(info.filename)
            if info.is_dir():
                continue
            mode = (info.external_attr >> 16) & 0o170000
            if mode in (0o120000, 0o040000):
                opened.close()
                raise ValueError("linked or disguised original ZIP file")
            entries.append((info.filename, info.file_size, info))
        return opened, entries
    if kind == "tar.gz":
        opened = tarfile.open(archive, "r:gz")
        entries = []
        for member in opened:
            safe_parts(member.name)
            if member.isdir():
                continue
            if not member.isfile() or member.issym() or member.islnk():
                opened.close()
                raise ValueError("special/link original TAR entry")
            entries.append((member.name, member.size, member))
        return opened, entries
    raise ValueError("unreviewed archive kind")


def normalize_original_tree(names: list[str], executable: str) -> dict[str, str]:
    """Map the *one* original executable to bin/<name>, preserving all siblings.

    Only a shared original archive prefix may be stripped; neither DLLs nor
    Clang lib/clang/ resource paths may be flattened into a fake 'bin' tree.
    """
    candidates = [name for name in names
                  if safe_parts(name)[-1] == executable]
    if len(candidates) != 1:
        raise ValueError("one and only one original native executable required")
    source = safe_parts(candidates[0])
    if len(source) >= 2 and source[-2] == "bin":
        prefix = source[:-2]
        shortcut = False
    else:
        prefix = source[:-1]
        shortcut = True
    mapped: dict[str, str] = {}
    seen = set()
    for name in names:
        parts = safe_parts(name)
        if parts[:len(prefix)] != prefix:
            raise ValueError("original asset outside expected common release root")
        rel = parts[len(prefix):]
        if not rel:
            raise ValueError("empty normalized native path")
        if shortcut and name == candidates[0]:
            rel = ("bin", executable)
        elif shortcut and len(rel) == 1 and rel[0].lower().endswith((".dll",".so")):
            rel = ("bin", rel[0])
        target = "/".join(rel)
        case_insensitive = target.casefold()
        if case_insensitive in seen:
            raise ValueError("duplicate/case-aliased native archive resource")
        seen.add(case_insensitive)
        mapped[name] = target
    if mapped[candidates[0]] != "bin/" + executable:
        raise ValueError("native executable not in fixed bin directory")
    return mapped


def copy_verified(reader, target: Path, expected: int, total: list[int],
                  per_file: int, max_total: int) -> dict:
    digest = hashlib.sha256()
    count = 0
    target.parent.mkdir(parents=True, exist_ok=True)
    with target.open("xb") as output:
        while data := reader.read(1024 * 1024):
            count += len(data)
            total[0] += len(data)
            if count > per_file or total[0] > max_total:
                raise ValueError("native full resource tree exceeds worker quota")
            digest.update(data)
            output.write(data)
    if count < 1 or (expected and count != expected):
        raise ValueError("original resource file is empty or has wrong length")
    return {"sha256":digest.hexdigest(), "bytes":count}


def stage_provider(archive: Path, kind: str, executable: str, destination: Path,
                   platform: str) -> dict:
    max_files = MAX_FILES_WIN if platform == "windows-x86_64" else MAX_FILES_LINUX
    max_total = MAX_TOTAL_WIN if platform == "windows-x86_64" else MAX_TOTAL_LINUX
    max_file = MAX_FILE_WIN if platform == "windows-x86_64" else MAX_FILE_LINUX
    opened, entries = archive_entries(archive, kind, executable)
    try:
        if not entries or len(entries) > max_files-4:
            raise ValueError("native release file count exceeds protected worker quota")
        names = [name for name,_,_ in entries]
        mapping = normalize_original_tree(names, executable)
        total = [0]
        files = {}
        for name, expected, member in entries:
            if expected < 0 or expected > max_file:
                raise ValueError("oversized original native resource")
            relative = mapping[name]
            target = destination.joinpath(*relative.split("/"))
            if kind == "gzip":
                with gzip.open(archive, "rb") as stream:
                    result = copy_verified(stream,target,0,total,max_file,max_total)
            else:
                stream = opened.open(member) if kind == "zip" else opened.extractfile(member)
                if stream is None:
                    raise ValueError("archive member stream unavailable")
                with stream:
                    result = copy_verified(stream,target,expected,total,max_file,max_total)
            files[relative] = result
            if relative == "bin/" + executable and os.name != "nt":
                target.chmod(0o700)
        return {
            "originalArchiveEntries": len(entries),
            "stagedOriginalEntries": len(files),
            "originalBytesStaged": total[0],
            "files": files,
            "originalExecutableSha256": files["bin/"+executable]["sha256"],
            "resourceFileCount": sum(1 for path in files if
                path.startswith(("lib/clang/", "share/clang/", "lib/", "share/"))),
            "nativeLibraryFileCount": sum(1 for path in files if
                path.lower().endswith(".dll") or ".so" in Path(path).name),
        }
    finally:
        if opened is not None:
            opened.close()


def candidate(manifest: dict, original_notices: dict, archive_root: Path,
              notice_root: Path, destination: Path, platform: str, *,
              fetch_locked: bool=False) -> dict:
    if platform not in ("windows-x86_64","linux-x86_64"):
        raise ValueError("unsupported platform")
    upstream = checked_manifest(manifest)
    notices = validate_notices(original_notices)
    if destination.exists() or destination.is_symlink():
        raise ValueError("candidate destination exists")
    if destination.parent.is_symlink():
        raise ValueError("linked output parent")
    rows = [(name, item) for name, osname, item in upstream if osname == platform]
    if len(rows) != 3:
        raise ValueError("three original native releases required")
    checked_notices = []
    for row in notices:
        file = notice_root.joinpath(*row["destination"].split("/"))
        if (file.is_symlink() or not file.is_file()
                or not file.resolve().is_relative_to(notice_root.resolve())):
            raise ValueError("original notice absent, linked or outside source root")
        data = file.read_bytes()
        checked_notices.append((row["provider"],row["destination"],
                                data,check_original(row,data)))
    for ident,item in rows:
        file=archive_root/item["name"]
        if file.is_symlink():
            raise ValueError("linked original release archive")
        if not file.is_file():
            if not fetch_locked:
                raise ValueError("original pinned archive missing")
            archive_root.mkdir(parents=True,exist_ok=True)
            download_locked(item["url"],file)
        if not 0 < file.stat().st_size <= MAX_ARCHIVE or sha256(file) != item["sha256"]:
            raise ValueError("original native release digest mismatch")
    destination.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".dev02-fullnative-",dir=destination.parent) as temporary:
        root=Path(temporary)/"resources"
        root.mkdir()
        receipt = {
            "schema":SCHEMA, "platform":platform, "providers":{},
            "originalNoticesVerified":4,
            "releaseApproved":False, "redistributionApproved":False,
            "legalReviewComplete":False, "protectedLuProtocolAccepted":False,
            "realInstalledNsisDebAccepted":False,
            "dynamicDependencyClosureApproved":False,
            "allArchiveFileBytesRetained":True,
        }
        for ident, item in rows:
            source=archive_root/item["name"]
            target=root/ident
            target.mkdir()
            provider=stage_provider(source,item["kind"],item["binary"],target,platform)
            provider["archiveSha256"]=item["sha256"]
            provider["releaseVersion"]=next(row["version"] for row in manifest["releases"] if row["id"]==ident)
            notice_rows=[(relative,raw,record) for who,relative,raw,record in checked_notices if who==ident]
            for index,(_rel,raw,record) in enumerate(notice_rows):
                name = "NOTICES/" + f"{index:03d}-" + Path(_rel).name
                if name in provider["files"]:
                    raise ValueError("notice collides with original release")
                file=target.joinpath(*name.split("/"))
                file.parent.mkdir(exist_ok=True)
                file.write_bytes(raw)
                provider["files"][name]={"sha256":record["sha256"],"bytes":len(raw)}
            if len(provider["files"]) > (MAX_FILES_WIN if platform.startswith("windows") else MAX_FILES_LINUX):
                raise ValueError("original archive and notices exceed native worker file quota")
            receipt["providers"][ident]=provider
        if set(receipt["providers"]) != {"clangd","rust-analyzer","ruff"}:
            raise ValueError("provider set is incomplete")
        (root/"candidate-full-resource-receipt.json").write_text(
            json.dumps(receipt,sort_keys=True,indent=2)+"\n",encoding="utf-8"
        )
        os.replace(root,destination)
    return receipt


def main() -> int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--manifest",type=Path,required=True)
    parser.add_argument("--notices-manifest",type=Path,required=True)
    parser.add_argument("--archives",type=Path,required=True)
    parser.add_argument("--notices-root",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--platform",choices=["windows-x86_64","linux-x86_64"],required=True)
    parser.add_argument("--fetch-locked",action="store_true")
    a=parser.parse_args()
    try:
        runner=Path(os.environ.get("RUNNER_TEMP",""))
        if (os.environ.get("GITHUB_ACTIONS")!="true" or not runner.is_dir() or
                a.output.exists() or a.output.is_symlink() or
                not a.output.resolve().is_relative_to(runner.resolve()) or
                a.platform.startswith("windows") != (os.name=="nt")):
            raise ValueError("CI-only matching native OS disposable output required")
        result=candidate(
            json.loads(a.manifest.read_text()),
            json.loads(a.notices_manifest.read_text()),
            a.archives,a.notices_root,a.output,a.platform,
            fetch_locked=a.fetch_locked
        )
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError,
            zipfile.BadZipFile,tarfile.TarError,gzip.BadGzipFile) as exc:
        print("DENIED: complete native candidate resource intake: "+type(exc).__name__,
              file=sys.stderr)
        return 2
    print("DEV02_ALL_ORIGINAL_NATIVE_RELEASE_FILE_BYTES_STAGED=PASS")
    print("DEV02_ORIGINAL_LICENSE_TEXT_SHA256_VERIFIED="+str(result["originalNoticesVerified"]))
    print("DEV02_PROTECTED_LU_REAL_PROTOCOL_AND_RELEASE_AUTHORIZATION=OPEN")
    return 0

if __name__=="__main__":
    raise SystemExit(main())
