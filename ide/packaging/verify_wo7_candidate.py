#!/usr/bin/env python3
"""WO7 signed CANDIDATE preflight for real installed smoke before release approval.

Avoid circular dependency: installed acceptance is collected AFTER candidate
signature verification. This never approves the release or native Run/Debug.
"""
import argparse
import json
import os
from pathlib import Path
import sys

from verify_wo7_nsis_release import Denied, HEX40, HEX64, safe_asset, digest, native_signature


def audit_candidate(manifest, root, source_sha, signature=native_signature):
    data=json.loads(manifest.read_text(encoding="utf-8"))
    if data.get("schema") != "lu.ide.wo7.signed-nsis.v1":
        raise Denied("unsupported_candidate_schema")
    if not HEX40.fullmatch(source_sha) or data.get("sourceSha") != source_sha:
        raise Denied("candidate_wrong_source_sha")
    if data.get("licensesReviewed") is not True:
        raise Denied("candidate_licenses_unreviewed")
    root=root.resolve(strict=True)
    records=[]
    for role in ("installer","service","desktop"):
        row=data.get(role)
        if not isinstance(row,dict):
            raise Denied("candidate_missing_" + role)
        sha,thumb=row.get("sha256"),row.get("signerThumbprint")
        if not isinstance(sha,str) or not HEX64.fullmatch(sha):
            raise Denied("candidate_missing_sha")
        if not isinstance(thumb,str) or not HEX40.fullmatch(thumb):
            raise Denied("candidate_missing_signer")
        path=safe_asset(root,row.get("path"))
        if digest(path).lower() != sha.lower():
            raise Denied("candidate_changed_" + role)
        result, signer=signature(path)
        if result != "Valid" or signer.lower() != thumb.lower():
            raise Denied("candidate_signer_or_chain_invalid")
        records.append({"role":role,"sha256":sha.lower(),"signerThumbprint":thumb.lower()})
    return {"sourceSha":source_sha,"verifiedSignatures":records,
            "stage":"signed_candidate_for_native_installed_testing",
            "releaseAccepted":False,"protectedRunTestDebugAccepted":False}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--manifest",type=Path,required=True)
    parser.add_argument("--artifact-root",type=Path,required=True)
    parser.add_argument("--source-sha",required=True)
    args=parser.parse_args()
    if os.name!="nt":
        print("DENIED: genuine Windows signature verification required",file=sys.stderr)
        return 1
    try:
        print(json.dumps(audit_candidate(args.manifest,args.artifact_root,args.source_sha),sort_keys=True))
        return 0
    except (ValueError,TypeError,OSError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc,Denied) else "invalid_candidate"),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
