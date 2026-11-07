#!/usr/bin/env python3
"""P06 cross-OS hash-pinned committed npm/Cargo graph; not release approval."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
class Denied(ValueError):pass
HEX64=re.compile(r"[0-9a-f]{64}\Z")
HEX40=re.compile(r"[0-9a-f]{40}\Z")
def sha(b):return hashlib.sha256(b).hexdigest()
def verify(review,evidence,npm_raw,cargo_raw,platform):
    if (review.get("schema")!="lu.ide.p06.reviewed-lock-graph.v1" or
        not HEX40.fullmatch(str(review.get("originSourceSha",""))) or
        review.get("releaseApproved") is not False or
        review.get("licenseLegalReviewApproved") is not False or
        review.get("sourceLockfilesCommitted") is not True or
        review.get("rawByteParity")!={"cargo":True,"npm":True} or
        review.get("semanticParity")!={"npm":True}):
        raise Denied("untrusted_native_lock_review")
    if platform not in ("windows-2022","ubuntu-24.04"):
        raise Denied("unsupported_native_platform")
    if evidence.get("schema")!="lu.ide.p06.resolved-dependency-evidence.v1":
        raise Denied("untrusted_resolved_graph_evidence")
    if evidence.get("rustTauri")!="=2.12.5" or evidence.get("javascriptTauri")!="2.12.4":
        raise Denied("unexpected_runtime_version")
    if (evidence.get("releaseApproved") is not False or
        evidence.get("lockfilesCommittedAndReviewed") is not True or
        evidence.get("unresolvedLicenseEntries",0)<0 or
        evidence.get("upstreamSourcesVerified") is not True):
        raise Denied("invalid_native_evidence_promotion")
    npm_bytes=bytes(npm_raw)
    cargo_bytes=bytes(cargo_raw)
    if sha(cargo_bytes)!=review["cargoRawSha256"] or sha(cargo_bytes)!=evidence["cargoLockSha256"]:
        raise Denied("resolved_cargo_graph_differs_from_reviewed_heads")
    if sha(npm_bytes)!=review["npmPlatformRawSha256"][platform] or sha(npm_bytes)!=evidence["npmLockSha256"]:
        raise Denied("native_platform_npm_byte_drift")
    try:
        doc=json.loads(npm_bytes)
        canon=json.dumps(doc,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode("utf-8")
    except (UnicodeError,ValueError) as exc:
        raise Denied("unparseable_npm_lock_graph") from exc
    if sha(canon)!=review["npmCanonicalJsonSha256"]:
        raise Denied("npm_semantic_graph_differs_cross_platform")
    if evidence.get("cargoPackages")!=review.get("cargoPackages") or evidence.get("npmPackages")!=review.get("npmPackages"):
        raise Denied("unexpected_resolved_package_counts")
    return {"sourceSha":evidence["sourceSha"],"platform":platform,
            "cargoReviewedSha256":review["cargoRawSha256"],
            "npmCanonicalSha256":review["npmCanonicalJsonSha256"],
            "crossOSSemanticReviewPassed":True,
            "sourceLockfilesCommitted":True,
            "releaseApproved":False}
def main():
    p=argparse.ArgumentParser()
    for a in ("review","evidence","npm-lock","cargo-lock","platform"):
        p.add_argument("--"+a,required=True)
    p.add_argument("--receipt",type=Path,required=True)
    a=p.parse_args()
    try:
        if a.receipt.exists() or a.receipt.is_symlink():
            raise Denied("receipt_must_be_new")
        doc=json.loads(Path(a.review).read_text(encoding="utf-8"))
        proof=json.loads(Path(a.evidence).read_text(encoding="utf-8"))
        result=verify(doc,proof,Path(a.npm_lock).read_bytes(),Path(a.cargo_lock).read_bytes(),a.platform)
        a.receipt.write_text(json.dumps(result,sort_keys=True,indent=2)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps(result,sort_keys=True))
    return 0
if __name__=="__main__": raise SystemExit(main())
