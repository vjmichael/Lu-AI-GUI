#!/usr/bin/env python3
"""CI-only byte-verified LSP resources for genuine *technical* NSIS/Debian tests.

Not a release stager. No releaseApproved override, no runtime execution, no
production service and no binary artifact upload. Only the six original
published SHA-pinned release archives and four pinned Git-blob notices may be
used. Exits closed on missing/tampered inputs and never weakens Lu's native
process authorization.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile

from collect_original_lsp_notices import collect, validate as validate_notices
from review_upstream_lsp_archives import (
    checked_manifest, download_locked, extract_one, sha256, MAX_ARCHIVE
)
from stage_linux_language_server_release import bundle_digest as linux_tree_sha
from stage_language_server_release import windows_lsp_tree_digest

PLATFORMS={"linux-x86_64","windows-x86_64"}
PROVIDERS={"clangd","rust-analyzer","ruff"}
PIN_KEYS={
    "clangd":"LU_IDE_E2_CLANGD_BUNDLE_SHA256",
    "rust-analyzer":"LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256",
    "ruff":"LU_IDE_E2_RUFF_BUNDLE_SHA256",
}
SCHEMA="lu.ide.e2.technical-installed-lsp-resources.v1"


def read_manifest(path:Path)->dict:
    data=json.loads(path.read_text(encoding="utf-8"))
    if data.get("releaseApproved") is not False:
        raise ValueError("technical fixture cannot override release approval")
    return data


def safe_source_dir(path:Path)->None:
    if path.is_symlink() or not path.is_dir():
        raise ValueError("reviewed local directory absent/linked")


def stage(
    release_manifest:Path,notices_manifest:Path,archive_cache:Path,
    output:Path,platform:str,commit_sha:str,*,fetch:bool=False
)->dict:
    if platform not in PLATFORMS or not re.fullmatch(r"[a-f0-9]{40}",commit_sha):
        raise ValueError("invalid native CI platform or exact HEAD")
    if output.exists() or output.is_symlink() or output.parent.is_symlink():
        raise ValueError("technical installed output must be a new directory")
    if not os.environ.get("GITHUB_ACTIONS")=="true":
        raise ValueError("technical resources restricted to disposable CI")
    temp_root=Path(os.environ.get("RUNNER_TEMP",""))
    if not temp_root.is_dir() or not output.resolve().is_relative_to(temp_root.resolve()):
        raise ValueError("technical resource output must remain within RUNNER_TEMP")
    release=read_manifest(release_manifest)
    notice_manifest=read_manifest(notices_manifest)
    rows=checked_manifest(release)
    validate_notices(notice_manifest)
    archive_cache.mkdir(parents=True,exist_ok=True)
    safe_source_dir(archive_cache)

    matched={(provider,osname):metadata for provider,osname,metadata in rows}
    if {(p,platform) for p in PROVIDERS} - set(matched):
        raise ValueError("technical native provider set incomplete")
    receipt={
        "schema":SCHEMA,
        "sourceHead":commit_sha,
        "platform":platform,
        "releaseApproved":False,
        "redistributionReviewed":False,
        "signedServiceInstalled":False,
        "runLanguageServers":False,
        "installedProtectedSessionAccepted":False,
        "upstreamNoticeDependencyGraphApproved":False,
        "resources":{},
    }
    with tempfile.TemporaryDirectory(prefix="lu-ide-test-only-lsp-",dir=temp_root) as tmp:
        scratch=Path(tmp)
        originals=scratch/"originals"
        notice_receipt=collect(notice_manifest,originals,allow_network=fetch)
        if notice_receipt["redistributionApproved"] or not (
            notice_receipt["originalBytesVerified"]
        ):
            raise ValueError("technical original notice identity denied")
        staged=scratch/"resources"
        staged.mkdir()
        for provider in sorted(PROVIDERS):
            entry=matched[(provider,platform)]
            archive=archive_cache/entry["name"]
            if archive.is_symlink():
                raise ValueError("symlink to official archive denied")
            if not archive.is_file():
                if not fetch:
                    raise ValueError("required offline original release archive missing")
                download_locked(entry["url"],archive)
            if archive.stat().st_size<1 or archive.stat().st_size>MAX_ARCHIVE:
                raise ValueError("official archive violates compressed budget")
            if sha256(archive)!=entry["sha256"]:
                raise ValueError("official release archive digest mismatch")
            server=staged/"language-servers"/provider
            binaries=server/"bin"
            binaries.mkdir(parents=True)
            binary=binaries/entry["binary"]
            # Archive format/members and expanded binary length checked here.
            digest,length=extract_one(archive,entry["kind"],entry["binary"],binary)
            binary.chmod(0o755 if platform=="linux-x86_64" else 0o644)
            notices=server/"NOTICES"
            notices.mkdir()
            source_records=[row for row in notice_manifest["notices"]
                            if row["provider"]==provider]
            if not source_records:
                raise ValueError("no original upstream license notice for "+provider)
            for item in source_records:
                src=originals/item["destination"]
                if src.is_symlink() or not src.is_file():
                    raise ValueError("original license cannot be missing or linked")
                dest=notices/Path(item["destination"]).name
                shutil.copyfile(src,dest)
                if sha256(dest)!=sha256(src):
                    raise ValueError("notice SHA changed while staging")
                dest.chmod(0o644)
            expected=linux_tree_sha(server) if platform=="linux-x86_64" else (
                windows_lsp_tree_digest(server)
            )
            receipt["resources"][provider]={
                "archiveSha256":entry["sha256"],
                "binarySha256":digest,
                "binaryBytes":length,
                "bundleSha256":expected,
                "compileTimePin":PIN_KEYS[provider],
                "licenseFiles":len(source_records),
            }
        # The CI-only sentinel is OUTSIDE each hashed provider tree so a
        # provider's native full-tree digest cannot accidentally include it.
        (staged/"DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION.txt").write_text(
            "TECHNICAL CI ONLY. Unapproved provider redistribution and signed-service deployment.\n"
            "No production approval, runtime permission, installed protected LSP or end-user release.\n",
            encoding="utf-8"
        )
        (staged/"technical-lsp-resource-receipt.json").write_text(
            json.dumps(receipt,indent=2,sort_keys=True)+"\n",encoding="utf-8"
        )
        # OS atomic rename: avoid accidentally leaving partial resource trees.
        os.replace(staged,output)
    return receipt


def main()->int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--release-manifest",required=True,type=Path)
    parser.add_argument("--notice-manifest",required=True,type=Path)
    parser.add_argument("--archive-cache",required=True,type=Path)
    parser.add_argument("--output",required=True,type=Path)
    parser.add_argument("--platform",choices=sorted(PLATFORMS),required=True)
    parser.add_argument("--source-sha",required=True)
    parser.add_argument("--fetch-locked",action="store_true")
    args=parser.parse_args()
    try:
        receipt=stage(args.release_manifest,args.notice_manifest,args.archive_cache,
                      args.output,args.platform,args.source_sha,fetch=args.fetch_locked)
    except (OSError,ValueError,KeyError,TypeError,json.JSONDecodeError) as error:
        print("DENIED: test-only LSP native resources: "+str(error),file=sys.stderr)
        return 2
    print("DEV02_TEST_ONLY_THREE_NATIVE_LANGUAGES_ORIGINAL_HASHES=PASS")
    print("DEV02_TEST_ONLY_SOURCE_HEAD="+receipt["sourceHead"])
    print("DEV02_DISTRIBUTION_SIGNING_AND_PROTECTED_LSP=NOT_ACCEPTED")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
