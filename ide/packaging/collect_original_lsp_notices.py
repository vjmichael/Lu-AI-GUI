#!/usr/bin/env python3
"""Pin original upstream LSP notices by Git content SHA; does not approve release.

Only fixed source/revision/path tuples from the review manifest may be fetched.
No project-controlled URL or script, no executable launch, no blanket copy of
dependency trees. This verifies the exact original upstream notice bytes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import urllib.parse
import urllib.request

SCHEMA = "lu.ide.e2.original-upstream-lsp-notices.v1"
EXPECTED = {
    ("clangd", "llvm/llvm-project", "llvmorg-23.1.0",
     "llvm/LICENSE.TXT", "clangd/NOTICES/LLVM-LICENSE.TXT"),
    ("rust-analyzer", "rust-lang/rust-analyzer", "2026-11-02",
     "LICENSE-MIT", "rust-analyzer/NOTICES/LICENSE-MIT"),
    ("rust-analyzer", "rust-lang/rust-analyzer", "2026-11-02",
     "LICENSE-APACHE", "rust-analyzer/NOTICES/LICENSE-APACHE"),
    ("ruff", "astral-sh/ruff", "0.17.4",
     "LICENSE", "ruff/NOTICES/LICENSE"),
}
HEX40 = re.compile(r"^[a-f0-9]{40}$")
MAX_NOTICE = 2 * 1024 * 1024


def validate(manifest: dict) -> list[dict]:
    if not isinstance(manifest, dict) or manifest.get("schema") != SCHEMA or (
        manifest.get("releaseApproved") is not False
    ):
        raise ValueError("unapproved original notice schema required")
    rows = manifest.get("notices")
    if not isinstance(rows, list) or len(rows) != len(EXPECTED):
        raise ValueError("exact native notice inventory required")
    matches = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "provider", "repo", "ref", "path", "gitBlobSha1", "destination"
        }:
            raise ValueError("unexpected notice metadata")
        key = tuple(row[k] for k in (
            "provider", "repo", "ref", "path", "destination"
        ))
        if key not in EXPECTED or key in matches or not HEX40.fullmatch(
            str(row["gitBlobSha1"])
        ):
            raise ValueError("unapproved original notice or Git blob digest")
        matches.add(key)
    if matches != EXPECTED:
        raise ValueError("incomplete original upstream notice set")
    return rows


def git_blob_sha1(data: bytes) -> str:
    return hashlib.sha1(
        b"blob " + str(len(data)).encode("ascii") + b"\x00" + data
    ).hexdigest()


def check_original(row: dict, data: bytes) -> dict:
    if not isinstance(data, bytes) or not (0 < len(data) <= MAX_NOTICE):
        raise ValueError("original notice is empty or excessive")
    if git_blob_sha1(data) != row["gitBlobSha1"]:
        raise ValueError("original upstream Git blob byte identity mismatch")
    return {
        "provider":row["provider"],
        "upstreamRepository":row["repo"],
        "releaseRef":row["ref"],
        "originalPath":row["path"],
        "gitBlobSha1":row["gitBlobSha1"],
        "sha256":hashlib.sha256(data).hexdigest(),
        "bytes":len(data),
        "reviewedForRedistribution":False,
    }


def fetch_original(row: dict) -> bytes:
    url = "https://raw.githubusercontent.com/" + "/".join((
        row["repo"],row["ref"],row["path"]
    ))
    with urllib.request.urlopen(
        urllib.request.Request(url,headers={
            "User-Agent":"Lu-IDE-upstream-original-license-intake"
        }),timeout=45
    ) as response:
        resolved=urllib.parse.urlsplit(response.geturl())
        if resolved.scheme != "https" or resolved.hostname not in (
            "raw.githubusercontent.com", "github.com"
        ):
            raise ValueError("original notice fetched from untrusted redirect")
        data=response.read(MAX_NOTICE+1)
    return data


def collect(manifest: dict, output: Path, *, allow_network:bool=False,
            source_root:Path|None=None)->dict:
    rows=validate(manifest)
    if output.exists() or output.is_symlink():
        raise ValueError("original notice output destination must be new")
    if output.parent.is_symlink():
        raise ValueError("symlinked output parent denied")
    checked=[]
    contents=[]
    for row in rows:
        if source_root is not None:
            source=source_root / row["destination"]
            if source.is_symlink() or not source.is_file() or (
                not source.resolve().is_relative_to(source_root.resolve())
            ):
                raise ValueError("original notice source missing/linked")
            contents.append(source.read_bytes())
        elif allow_network:
            contents.append(fetch_original(row))
        else:
            raise ValueError("network disabled and no original notices provided")
    for row,data in zip(rows,contents):
        checked.append(check_original(row,data))
    output.mkdir(parents=True,exist_ok=False)
    for row,data in zip(rows,contents):
        target=output / row["destination"]
        target.parent.mkdir(parents=True,exist_ok=True)
        with target.open("xb") as writer:
            writer.write(data)
    receipt={
        "schema":"lu.ide.e2.original-upstream-lsp-notice-receipt.v1",
        "originalBytesVerified":True,
        "redistributionApproved":False,
        "dependenciesAudited":False,
        "legalReviewSigned":False,
        "notices":checked,
    }
    with (output/"original-upstream-notice-receipt.json").open(
        "x",encoding="utf-8"
    ) as writer:
        json.dump(receipt,writer,indent=2,sort_keys=True)
        writer.write("\n")
    return receipt


def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--manifest",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--source-root",type=Path)
    p.add_argument("--fetch-pinned",action="store_true")
    args=p.parse_args()
    try:
        if args.fetch_pinned == (args.source_root is not None):
            raise ValueError("choose one: local originals or explicit pinned download")
        data=json.loads(args.manifest.read_text(encoding="utf-8"))
        receipt=collect(data,args.output,allow_network=args.fetch_pinned,
                        source_root=args.source_root)
        if len(receipt["notices"])!=4:
            raise ValueError("exactly four pinned original notices required")
    except (ValueError,OSError,TypeError,KeyError,
            json.JSONDecodeError) as e:
        print("DENIED: original LSP notice collection: "+str(e),file=sys.stderr)
        return 2
    print("DEV02_FOUR_UPSTREAM_ORIGINAL_NOTICE_GIT_BLOB_SHA1=PASS")
    print("DEV02_LICENSE_LEGAL_REVIEW_AND_REDISTRIBUTION=OPEN")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
