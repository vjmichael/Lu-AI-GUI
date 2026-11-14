#!/usr/bin/env python3
"""Fetch exact official ty beta archive into ephemeral CI research only."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import sys
import tarfile
import urllib.request
import urllib.parse
import zipfile

VERSION="0.0.99"
PINS={
    "windows-x86_64":(
        "ty-x86_64-pc-windows-msvc.zip",
        "b6ee52f5921ba8c911e8b4dc28b0860c12aae96774c1e33369d2ee3bdb4df76a",
        "ty.exe",
    ),
    "linux-x86_64":(
        "ty-x86_64-unknown-linux-gnu.tar.gz",
        "b98eec859fa0a840dcfff013f208b7a9d71051bf3f693dff68a9bedb0e19c449",
        "ty",
    ),
}
MAX_COMPRESSED=50*1024*1024
MAX_BINARY=200*1024*1024


def read_ty(path:Path,platform:str)->bytes:
    filename,_,entry=PINS[platform]
    if platform=="windows-x86_64":
        with zipfile.ZipFile(path) as archive:
            names=archive.infolist()
            matches=[item for item in names if not item.is_dir() and
                     PurePosixPath(item.filename).name==entry]
            if len(matches)!=1:
                raise ValueError("official ty release must have one executable")
            if matches[0].file_size>MAX_BINARY:
                raise ValueError("excessive ty executable size")
            if (matches[0].external_attr>>16)&0o170000==0o120000:
                raise ValueError("linked ty executable")
            return archive.read(matches[0])
    with tarfile.open(path,"r:gz") as archive:
        names=archive.getmembers()
        matches=[item for item in names if item.isfile() and
                 PurePosixPath(item.name).name==entry]
        if len(matches)!=1:
            raise ValueError("official ty release must have one executable")
        if matches[0].size>MAX_BINARY:
            raise ValueError("excessive ty executable size")
        reader=archive.extractfile(matches[0])
        if reader is None:
            raise ValueError("missing official ty executable bytes")
        with reader:
            return reader.read(MAX_BINARY+1)


def stage(platform:str,destination:Path)->dict:
    if platform not in PINS:
        raise ValueError("platform unsupported")
    if destination.exists() or destination.is_symlink():
        raise ValueError("destination must be new and disposable")
    runner=Path(os.environ.get("RUNNER_TEMP",""))
    if os.environ.get("GITHUB_ACTIONS")!="true" or (
        not runner.is_dir() or
        not destination.resolve().is_relative_to(runner.resolve())
    ):
        raise ValueError("CI-only candidate cannot leave runner disposable directory")
    name,expected,binary_name=PINS[platform]
    url=f"https://github.com/astral-sh/ty/releases/download/{VERSION}/{name}"
    req=urllib.request.Request(url,headers={"User-Agent":"LuIDE-Python-LSP-CI-only"})
    with urllib.request.urlopen(req,timeout=60) as response:
        redirect=urllib.parse.urlsplit(response.geturl())
        if redirect.scheme!="https" or redirect.hostname not in (
            "github.com","release-assets.githubusercontent.com",
            "objects.githubusercontent.com"
        ):
            raise ValueError("unapproved release redirect host")
        compressed=response.read(MAX_COMPRESSED+1)
    if not compressed or len(compressed)>MAX_COMPRESSED:
        raise ValueError("unbounded ty release compressed asset")
    observed=hashlib.sha256(compressed).hexdigest()
    if observed!=expected:
        raise ValueError("official ty GitHub asset SHA256 mismatch")
    import tempfile
    with tempfile.TemporaryDirectory(dir=runner) as temporary:
        archive=Path(temporary)/name
        archive.write_bytes(compressed)
        raw=read_ty(archive,platform)
    if not raw or len(raw)>MAX_BINARY:
        raise ValueError("unbounded native ty binary")
    destination.mkdir(mode=0o700)
    binary=destination/binary_name
    binary.write_bytes(raw)
    if platform=="linux-x86_64":
        binary.chmod(0o700)
    receipt={
        "schema":"lu.ide.e2.ty-candidate-only.v1",
        "releaseApproved":False,
        "installedInLu":False,
        "version":VERSION,
        "upstreamRelease":url,
        "archiveSha256":observed,
        "candidateExecutableSha256":hashlib.sha256(raw).hexdigest(),
        "platform":platform,
        "candidateExecutableName":binary_name,
    }
    (destination/"candidate-receipt.json").write_text(
        json.dumps(receipt,indent=2,sort_keys=True)+"\n",encoding="utf-8"
    )
    return receipt


def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--platform",choices=sorted(PINS),required=True)
    p.add_argument("--destination",type=Path,required=True)
    args=p.parse_args()
    try:
        receipt=stage(args.platform,args.destination)
    except (ValueError,OSError,zipfile.BadZipFile,tarfile.TarError) as error:
        print("DENIED: ty beta CI candidate: "+str(error),file=sys.stderr)
        return 2
    print("DEV02_TY_ORIGINAL_PINNED_RELEASE_ARCHIVE=PASS")
    print("DEV02_TY_CANDIDATE_EXECUTABLE_SHA256="+receipt["candidateExecutableSha256"])
    print("DEV02_PYTHON_SEMANTIC_RELEASE_APPROVED=NO")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
