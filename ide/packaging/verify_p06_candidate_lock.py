#!/usr/bin/env python3
"""For every unsigned verification package: pin the *resolved* OS lock bytes.

Reviewed hashes permit reproducible TEST dependency resolution, but do not
imply a signed commercial release or complete legal clearance.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tomllib

class LockDenied(ValueError):pass

def sha(data):
    return hashlib.sha256(data).hexdigest()

def check(root,platform,review=None):
    root=Path(root)
    if platform not in ("windows-2022","ubuntu-24.04"):
        raise LockDenied("unsupported_reviewed_os")
    if review is None:
        review=json.loads((root/"ide/packaging/p06-reviewed-lock-graphs.json").read_text(encoding="utf-8"))
    if (review.get("schema")!="lu.ide.p06.reviewed-lock-graph.v1"
            or review.get("sourceLockfilesCommitted") is not True
            or review.get("releaseApproved") is not False
            or review.get("licenseLegalReviewApproved") is not False
            or review.get("rawByteParity")!={"cargo":True,"npm":True}
            or review.get("semanticParity")!={"npm":True}):
        raise LockDenied("unreviewed_candidate_graph_manifest")
    if review.get("sourceLockBlobSha") != {
        "cargo":"87d0045a4be1c78651f080300aba71c14dbe1f30",
        "npm":"dfe4412bfacfc22ad1149f5213f4a26aa49b4067"}:
        raise LockDenied("unknown_committed_lockfile_source_blobs")
    names={"npm":root/"ide/app/package-lock.json","cargo":root/"ide/app/src-tauri/Cargo.lock"}
    raw={}
    for k,path in names.items():
        if path.is_symlink() or not path.is_file() or not 100<=path.stat().st_size<=12_000_000:
            raise LockDenied("required_resolved_lockfile_missing_or_unsafe:"+k)
        raw[k]=path.read_bytes()
    if sha(raw["cargo"])!=review["cargoRawSha256"]:
        raise LockDenied("candidate_cargo_lock_not_reviewed")
    if sha(raw["npm"])!=review["npmPlatformRawSha256"][platform]:
        raise LockDenied("candidate_os_npm_lock_not_reviewed")
    try:
        cargo=tomllib.loads(raw["cargo"].decode("utf-8"))
        npm=json.loads(raw["npm"])
        canonical=json.dumps(npm,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode("utf-8")
    except (ValueError,UnicodeError) as exc:
        raise LockDenied("unparseable_candidate_lockfile") from exc
    if sha(canonical)!=review["npmCanonicalJsonSha256"]:
        raise LockDenied("candidate_npm_semantic_graph_differs")
    if not isinstance(cargo.get("package"),list) or len(cargo["package"])!=review["cargoPackages"]:
        raise LockDenied("candidate_cargo_package_count_drift")
    if not isinstance(npm.get("packages"),dict) or len(npm["packages"])-1!=review["npmPackages"]:
        raise LockDenied("candidate_npm_package_count_drift")
    for name in ("tauri","tauri-runtime","tauri-runtime-wry"):
        versions=[r.get("version") for r in cargo["package"] if isinstance(r,dict) and r.get("name")==name]
        if len(versions)!=1 or not isinstance(versions[0],str) or not versions[0].startswith("2.12."):
            raise LockDenied("candidate_transitive_tauri_minor_drift:"+name)
    return {"platform":platform,"cargoLockSha256":sha(raw["cargo"]),
            "npmLockSha256":sha(raw["npm"]),
            "npmSemanticSha256":sha(canonical),
            "resolvedGraphEqualsCrossOSReview":True,
            "sourceLockfilesCommitted":True,
            "signedOrLegallyApproved":False,
            "releaseApproved":False}

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",type=Path,required=True)
    p.add_argument("--platform",choices=("windows-2022","ubuntu-24.04"),required=True)
    a=p.parse_args()
    try:result=check(a.repo_root,a.platform)
    except (ValueError,TypeError,KeyError,OSError) as e:
        print("DENIED: "+str(e),file=sys.stderr);return 2
    print(json.dumps(result,sort_keys=True))
    return 0
if __name__=="__main__":raise SystemExit(main())
