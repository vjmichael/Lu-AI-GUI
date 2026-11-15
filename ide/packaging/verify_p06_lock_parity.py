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
def verify(review,evidence,npm_raw,cargo_raw,platform,scoped_reviewed_cargo_sha=None):
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
    expected_cargo = (review["cargoRawSha256"] if scoped_reviewed_cargo_sha is None
                      else scoped_reviewed_cargo_sha)
    if sha(cargo_bytes)!=expected_cargo or sha(cargo_bytes)!=evidence["cargoLockSha256"]:
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
            "cargoReviewedSha256":expected_cargo,
            "originalP06CargoReviewSha256":review["cargoRawSha256"],
            "localSuccessorOnly":scoped_reviewed_cargo_sha is not None,
            "npmCanonicalSha256":review["npmCanonicalJsonSha256"],
            "crossOSSemanticReviewPassed":True,
            "sourceLockfilesCommitted":True,
            "releaseApproved":False}
def main():
    p=argparse.ArgumentParser()
    for a in ("review","evidence","npm-lock","cargo-lock","platform"):
        p.add_argument("--"+a,required=True)
    p.add_argument("--receipt",type=Path,required=True)
    p.add_argument("--merge02-successor",action="store_true")
    p.add_argument("--repo-root",type=Path)
    a=p.parse_args()
    try:
        if a.receipt.exists() or a.receipt.is_symlink():
            raise Denied("receipt_must_be_new")
        doc=json.loads(Path(a.review).read_text(encoding="utf-8"))
        proof=json.loads(Path(a.evidence).read_text(encoding="utf-8"))
        scoped = None
        if a.merge02_successor:
            if a.repo_root is None:
                raise Denied("merge02_source_root_required")
            from verify_p06_candidate_lock import check as check_candidate
            root = a.repo_root.resolve(strict=True)
            source_cargo = root / "ide/app/src-tauri/Cargo.lock"
            source_npm = root / "ide/app/package-lock.json"
            if (Path(a.cargo_lock).resolve(strict=True) != source_cargo
                    or Path(a.npm_lock).resolve(strict=True) != source_npm):
                raise Denied("merge02_untrusted_lock_paths")
            original_review = json.loads(
                (root/"ide/packaging/p06-reviewed-lock-graphs.json").read_text(encoding="utf-8"))
            if doc != original_review:
                raise Denied("merge02_original_review_modified")
            successor = json.loads(
                (root/"ide/packaging/ide-merge02-reviewed-lock-successor.json")
                .read_text(encoding="utf-8"))
            scoped = check_candidate(root, a.platform, original_review,
                                     ide_merge02_successor=successor)["cargoLockSha256"]
        result=verify(doc,proof,Path(a.npm_lock).read_bytes(),Path(a.cargo_lock).read_bytes(),a.platform,
                      scoped_reviewed_cargo_sha=scoped)
        a.receipt.write_text(json.dumps(result,sort_keys=True,indent=2)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps(result,sort_keys=True))
    return 0
if __name__=="__main__": raise SystemExit(main())
