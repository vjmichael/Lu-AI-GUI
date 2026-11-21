#!/usr/bin/env python3
"""Verified upstream LSP archive intake: NO redistributable approval or install.

The only permitted network request is a release-specific HTTPS GitHub asset
that matches the reviewed manifest's full SHA256. Archive files are inspected
without extractall; exactly one bounded native executable is streamed into
a disposable temp directory, hashed, and deleted on exit. Only a JSON
review receipt is produced. No process is launched.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request
import zipfile

EXPECTED = {
    "clangd": "clangd/clangd",
    "rust-analyzer": "rust-lang/rust-analyzer",
    "ruff": "astral-sh/ruff",
}
TARGETS = {"windows-x86_64", "linux-x86_64"}
BINARY_NAMES = {"clangd": ("clangd.exe","clangd"),
                "rust-analyzer": ("rust-analyzer.exe","rust-analyzer"),
                "ruff": ("ruff.exe","ruff")}
MAX_ARCHIVE = 160 * 1024 * 1024
MAX_BINARY = 512 * 1024 * 1024
MAX_FILES = 5000
HEX = re.compile(r"^[a-f0-9]{64}$")


def checked_manifest(manifest: dict) -> list[tuple[str,str,dict]]:
    if manifest.get("schema") != "lu.ide.e2.lsp-upstream-archive-candidates.v1" or (
        manifest.get("releaseApproved") is not False
    ):
        raise ValueError("not a fixed unapproved LSP candidate manifest")
    records = manifest.get("releases")
    if not isinstance(records, list) or len(records) != 3:
        raise ValueError("exactly three release families required")
    seen = set()
    result = []
    for row in records:
        ident = row.get("id")
        if ident not in EXPECTED or ident in seen or (
            row.get("repo") != EXPECTED[ident]
        ):
            raise ValueError("unexpected or duplicate original upstream release")
        seen.add(ident)
        version = row.get("version")
        if not isinstance(version, str) or not re.fullmatch(
            r"[A-Za-z0-9._+-]{1,80}", version
        ):
            raise ValueError("version must be release-specific, not latest")
        targets = row.get("targets")
        if not isinstance(targets, dict) or set(targets) != TARGETS:
            raise ValueError("native provider release is missing an OS")
        for platform, item in targets.items():
            if not isinstance(item, dict) or set(item) != {
                "name", "url", "sha256", "kind", "binary"
            }:
                raise ValueError("invalid locked upstream archive record")
            expected_binary = BINARY_NAMES[ident][
                0 if platform == "windows-x86_64" else 1
            ]
            if item["binary"] != expected_binary or item["kind"] not in {
                "zip","tar.gz","gzip"
            } or not HEX.fullmatch(item["sha256"]):
                raise ValueError("unreviewed binary name, format or hash")
            uri = (
                f"https://github.com/{EXPECTED[ident]}/releases/download/"
                f"{version}/{item['name']}"
            )
            if item["url"] != uri or not re.fullmatch(
                r"[A-Za-z0-9._+-]{1,150}", item["name"]
            ):
                raise ValueError("asset URL not an exact approved upstream release")
            result.append((ident,platform,item))
    if seen != set(EXPECTED):
        raise ValueError("candidate LSP release family incomplete")
    return result


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as opened:
        while chunk := opened.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def checked_member_name(value: str) -> str:
    if not value or "\\" in value or value.startswith("/"):
        raise ValueError("unsafe archive member name")
    parts = PurePosixPath(value).parts
    if not parts or any(part in ("..","") for part in parts):
        raise ValueError("archive traversal")
    return parts[-1]


def copy_bounded(source, target: Path) -> tuple[str,int]:
    digest = hashlib.sha256()
    length = 0
    with target.open("xb") as output:
        while chunk := source.read(1024 * 1024):
            length += len(chunk)
            if length > MAX_BINARY:
                raise ValueError("native executable exceeds bounded decompression")
            digest.update(chunk)
            output.write(chunk)
    if length == 0:
        raise ValueError("empty reviewed native executable")
    return digest.hexdigest(), length


def extract_one(archive: Path, kind: str, expected_name: str, target: Path):
    if kind == "gzip":
        with gzip.open(archive, "rb") as reader:
            return copy_bounded(reader, target)
    if kind == "zip":
        with zipfile.ZipFile(archive) as zipped:
            members = zipped.infolist()
            if len(members) > MAX_FILES:
                raise ValueError("archive has excessive entries")
            matches = []
            for member in members:
                basename = checked_member_name(member.filename)
                if member.is_dir():
                    continue
                mode = (member.external_attr >> 16) & 0o170000
                if mode == 0o120000:
                    raise ValueError("symlink in candidate archive")
                if basename == expected_name:
                    matches.append(member)
            if len(matches) != 1:
                raise ValueError("candidate archive must have exactly one executable")
            if matches[0].file_size <= 0 or matches[0].file_size > MAX_BINARY:
                raise ValueError("unbounded candidate binary")
            with zipped.open(matches[0]) as reader:
                return copy_bounded(reader, target)
    if kind == "tar.gz":
        with tarfile.open(archive, "r:gz") as zipped:
            members = zipped.getmembers()
            if len(members) > MAX_FILES:
                raise ValueError("archive has excessive entries")
            matches = []
            for member in members:
                basename = checked_member_name(member.name)
                if member.issym() or member.islnk():
                    raise ValueError("linked candidate archive member")
                if basename == expected_name and member.isfile():
                    matches.append(member)
            if len(matches) != 1:
                raise ValueError("candidate archive must have exactly one executable")
            if matches[0].size <= 0 or matches[0].size > MAX_BINARY:
                raise ValueError("unbounded candidate binary")
            entry = zipped.extractfile(matches[0])
            if entry is None:
                raise ValueError("could not read candidate binary")
            with entry:
                return copy_bounded(entry, target)
    raise ValueError("candidate archive format denied")


def download_locked(url: str, target: Path) -> None:
    with urllib.request.urlopen(urllib.request.Request(
        url, headers={"User-Agent":"Lu-IDE-LSP-candidate-verified-CI"}
    ),timeout=75) as response:
        redirect = urllib.parse.urlsplit(response.geturl())
        if redirect.scheme != "https" or redirect.hostname not in {
            "github.com","release-assets.githubusercontent.com",
            "objects.githubusercontent.com"
        }:
            raise ValueError("asset redirect outside authorized GitHub hosts")
        count = 0
        with target.open("xb") as output:
            while block := response.read(1024 * 1024):
                count += len(block)
                if count > MAX_ARCHIVE:
                    raise ValueError("compressed release archive exceeds hard budget")
                output.write(block)
        if count == 0:
            raise ValueError("empty upstream release asset")


def review_archives(
    manifest: dict, archive_root: Path, receipt_path: Path,
    *, allow_network: bool = False,
) -> dict:
    rows = checked_manifest(manifest)
    if receipt_path.exists() or receipt_path.is_symlink():
        raise ValueError("receipt output must be new")
    receipt = {
        "schema": "lu.ide.e2.lsp-upstream-archive-review.v1",
        "releaseApproved": False,
        "redistributionApproved": False,
        "installedOrExecuted": False,
        "nativeContainmentAccepted": False,
        "originalNoticesVerified": False,
        "sources": [],
    }
    with tempfile.TemporaryDirectory(prefix="lu-ide-upstream-inspect-") as temp:
        for n,(ident,platform,item) in enumerate(rows):
            archive = archive_root / item["name"]
            if archive.is_symlink():
                raise ValueError("symlinked upstream archive cannot be accepted")
            if not archive.is_file():
                if not allow_network:
                    raise ValueError("offline archive not present")
                archive_root.mkdir(parents=True, exist_ok=True)
                download_locked(item["url"],archive)
            if archive.stat().st_size < 1 or archive.stat().st_size > MAX_ARCHIVE:
                raise ValueError("candidate compressed archive exceeds budget")
            if sha256(archive) != item["sha256"]:
                raise ValueError("candidate upstream archive digest mismatch")
            binary = Path(temp) / f"candidate-{n}"
            binary_hash,binary_size = extract_one(
                archive,item["kind"],item["binary"],binary
            )
            receipt["sources"].append({
                "provider": ident,
                "platform": platform,
                "releaseAssetName": item["name"],
                "archiveSha256": item["sha256"],
                "executableSha256": binary_hash,
                "executableBytes": binary_size,
            })
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    with receipt_path.open("x", encoding="utf-8") as output:
        json.dump(receipt,output,indent=2,sort_keys=True)
        output.write("\n")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest",required=True,type=Path)
    parser.add_argument("--archive-root",required=True,type=Path)
    parser.add_argument("--receipt",required=True,type=Path)
    parser.add_argument("--fetch-locked",action="store_true")
    args=parser.parse_args()
    try:
        data=json.loads(args.manifest.read_text(encoding="utf-8"))
        result=review_archives(
            data,args.archive_root,args.receipt,allow_network=args.fetch_locked
        )
    except (OSError, ValueError, TypeError, KeyError,
            json.JSONDecodeError, zipfile.BadZipFile, tarfile.TarError,
            gzip.BadGzipFile) as error:
        print("DENIED: upstream LSP technical candidate intake: "+str(error),
              file=sys.stderr)
        return 2
    print("LSP_SIX_OFFICIAL_ARCHIVE_SHA256_AND_EXECUTABLE_DIGESTS=PASS")
    print("LSP_CANDIDATE_ONLY_NO_LICENSE_OR_RELEASE_APPROVAL")
    print("LSP_CANDIDATES="+str(len(result["sources"])))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
