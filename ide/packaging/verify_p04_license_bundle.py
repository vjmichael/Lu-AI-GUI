#!/usr/bin/env python3
"""Audit actual verification-package license bytes and bind to built artifact.

No signer, license legal approval, source-only green test or synthetic receipt
can turn a test package into a production release.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

SHA40=re.compile(r"^[0-9a-f]{40}$",re.I)
SHA64=re.compile(r"^[0-9a-f]{64}$",re.I)

class Denied(ValueError):
    pass

def sha(path:Path):
    h=hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda:fh.read(1024*1024),b""):
            h.update(chunk)
    return h.hexdigest()

def safe_file(root:Path,rel:str):
    if not isinstance(rel,str) or len(rel)>250 or not rel or "\\" in rel or ":" in rel:
        raise Denied("unsafe_license_artifact_path")
    parts=rel.split("/")
    if any(not p or p in {".",".."} or p.endswith((" ",".")) for p in parts):
        raise Denied("unsafe_license_path_component")
    candidate=root
    for part in parts:
        candidate=candidate/part
        if candidate.is_symlink():
            raise Denied("license_path_symlink")
    if not candidate.is_file():
        raise Denied("missing_notice")
    if candidate.stat().st_size>2_000_000:
        raise Denied("notice_file_too_large")
    return candidate

def audit(root:Path,source_sha:str,require_complete=False):
    if not SHA40.fullmatch(source_sha):
        raise Denied("exact_head_not_provided")
    if root.is_symlink():
        raise Denied("license_root_symlink")
    root=root.resolve(strict=True)
    data=json.loads(safe_file(root,"inventory.json").read_text(encoding="utf-8"))
    if (data.get("schema")!="lu.ide.p04.verification-licenses.v1" or
            data.get("sourceSha")!=source_sha or data.get("releaseApproved") is not False or
            data.get("licenseLegalReviewApproved") is not False):
        raise Denied("untrusted_notice_inventory")
    rows,upstream=data.get("resolvedPackages"),data.get("upstreamLicenses")
    pending=data.get("unresolved")
    if not isinstance(rows,list) or len(rows)>4000 or data.get("packageCount")!=len(rows):
        raise Denied("resolved_dependency_count_mismatch")
    if not isinstance(upstream,list) or len(upstream)!=6 or not isinstance(pending,list):
        raise Denied("missing_upstream_or_unresolved_classification")
    expected={"inventory.json","README.txt"}
    def validate_mark(item,source):
        rel,digest=item.get("path"),item.get("sha256")
        if not isinstance(digest,str) or not SHA64.fullmatch(digest):
            raise Denied("unhashed_third_party_notice")
        if rel in expected or not isinstance(rel,str) or not rel.startswith(source):
            raise Denied("duplicated_or_invalid_notice")
        if sha(safe_file(root,rel))!=digest.lower():
            raise Denied("license_file_digest_drift")
        expected.add(rel)
    for item in upstream:
        if not isinstance(item,dict) or not isinstance(item.get("sourceRevision"),str):
            raise Denied("upstream_notice_provenance_missing")
        validate_mark(item,"upstream/")
    for row in rows:
        if not isinstance(row,dict) or row.get("manager") not in {"npm","cargo"} or not isinstance(row.get("name"),str) or not isinstance(row.get("version"),str):
            raise Denied("untrusted_resolved_package")
        notices=row.get("notices")
        if not isinstance(notices,list) or len(notices)>16:
            raise Denied("invalid_resolved_package_notices")
        for item in notices:
            if not isinstance(item,dict):
                raise Denied("invalid_resolved_package_notice")
            validate_mark(item,"resolved/"+row["manager"]+"/")
    actual=set()
    for folder,dirs,files in os.walk(root,followlinks=False):
        for d in dirs:
            if (Path(folder)/d).is_symlink():
                raise Denied("linked_license_directory")
        for name in files:
            item=Path(folder)/name
            if item.is_symlink():
                raise Denied("linked_license_file")
            actual.add(item.relative_to(root).as_posix())
    if actual!=expected:
        raise Denied("unlisted_or_missing_legal_bytes")
    if not (root/"README.txt").read_text(encoding="utf-8").startswith("Lu IDE verification-package"):
        raise Denied("wrong_notice_readme")
    if data.get("completeForReleaseReview") is not (not pending):
        raise Denied("falsely_classified_license_completeness")
    if require_complete and (pending or not rows):
        raise Denied("incomplete_resolved_dependency_license_inventory")
    return {"sourceSha":source_sha,"packages":len(rows),
            "notices":len(expected)-2,"unresolved":len(pending),
            "actualBytesVerified":True,"signedOrLegallyApproved":False}

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--license-root",type=Path,required=True)
    p.add_argument("--source-sha",required=True)
    p.add_argument("--artifact",type=Path)
    p.add_argument("--receipt",type=Path)
    p.add_argument("--require-complete",action="store_true")
    a=p.parse_args()
    try:
        result=audit(a.license_root,a.source_sha,a.require_complete)
        if a.artifact:
            if a.artifact.is_symlink() or not a.artifact.is_file() or a.artifact.stat().st_size>1024*1024*1024:
                raise Denied("built_installer_absent_or_linked")
            result["installerSha256"]=sha(a.artifact)
            result["installerName"]=a.artifact.name
        if a.receipt:
            if a.receipt.exists() or a.receipt.is_symlink() or not a.artifact:
                raise Denied("receipt_must_be_new_and_bound_to_package")
            a.receipt.write_text(json.dumps(result,sort_keys=True,indent=2)+"\n",encoding="utf-8")
        print(json.dumps(result,sort_keys=True))
    except (OSError,ValueError,TypeError) as e:
        print("DENIED: "+(str(e) if isinstance(e,Denied) else "license_inventory_invalid"),file=sys.stderr)
        return 2
    return 0
if __name__=="__main__":
    raise SystemExit(main())
