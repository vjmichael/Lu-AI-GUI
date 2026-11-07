#!/usr/bin/env python3
"""P06 hashed resolved dependency evidence, NOT a signed release claim."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib
from verify_p04_license_bundle import audit as audit_licenses

class Denied(ValueError): pass

def load_lock(path, kind):
    if path.is_symlink() or not path.is_file() or not 100 <= path.stat().st_size <= 12*1024*1024:
        raise Denied("missing_or_unsafe_lockfile")
    raw=path.read_bytes()
    try:
        data=json.loads(raw) if kind=="npm" else tomllib.loads(raw.decode("utf-8"))
    except (ValueError,UnicodeError) as exc:
        raise Denied("malformed_lockfile") from exc
    if not isinstance(data,dict):
        raise Denied("invalid_lockfile_root")
    return data,hashlib.sha256(raw).hexdigest()

def inspect(root,source_sha,license_root):
    root=Path(root)
    if not isinstance(source_sha,str) or re.fullmatch(r"[0-9a-fA-F]{40}",source_sha) is None:
        raise Denied("exact_source_sha_required")
    npm,nsha=load_lock(root/"ide/app/package-lock.json","npm")
    cargo,csha=load_lock(root/"ide/app/src-tauri/Cargo.lock","cargo")
    try:
        package=json.loads((root/"ide/app/package.json").read_text(encoding="utf-8"))
        manifest=tomllib.loads((root/"ide/app/src-tauri/Cargo.toml").read_text(encoding="utf-8"))
        api=package["dependencies"]["@tauri-apps/api"]
        rust=manifest["dependencies"]["tauri"]["version"]
    except (OSError,TypeError,KeyError,ValueError) as exc:
        raise Denied("unreadable_source_dependency_manifest") from exc
    if (not isinstance(api,str) or not isinstance(rust,str)
            or not re.fullmatch(r"\d+\.\d+\.\d+",api)
            or not re.fullmatch(r"=\d+\.\d+\.\d+",rust)):
        raise Denied("unpinned_direct_tauri_dependency")
    minor=".".join(rust[1:].split(".")[:2])
    if ".".join(api.split(".")[:2])!=minor:
        raise Denied("cross_language_tauri_mismatch")
    crates=cargo.get("package")
    if cargo.get("version") not in (3,4) or not isinstance(crates,list):
        raise Denied("invalid_cargo_lock_schema")
    for component in ("tauri","tauri-runtime","tauri-runtime-wry"):
        versions=[p.get("version") for p in crates if isinstance(p,dict) and p.get("name")==component]
        if len(versions)!=1 or not isinstance(versions[0],str):
            raise Denied("unreviewed_runtime_resolution:"+component)
        if ".".join(versions[0].split(".")[:2])!=minor:
            raise Denied("resolved_tauri_runtime_minor_drift:"+component)
        if component=="tauri" and versions[0]!=rust[1:]:
            raise Denied("resolved_tauri_core_patch_drift")
    for entry in crates:
        if not isinstance(entry,dict) or not isinstance(entry.get("name"),str):
            raise Denied("invalid_resolved_cargo_package")
        source=entry.get("source")
        if source is not None and (not isinstance(source,str) or not source.startswith("registry+https://github.com/rust-lang/crates.io-index")):
            raise Denied("unreviewed_nonregistry_cargo_dependency")
        if source is not None and re.fullmatch(r"[a-fA-F0-9]{64}",str(entry.get("checksum",""))) is None:
            raise Denied("registry_crate_integrity_absent")
    packages=npm.get("packages")
    if npm.get("lockfileVersion")!=3 or not isinstance(packages,dict):
        raise Denied("invalid_npm_lock_schema")
    if packages.get("",{}).get("dependencies",{}).get("@tauri-apps/api")!=api:
        raise Denied("npm_root_not_matching_source")
    if packages.get("node_modules/@tauri-apps/api",{}).get("version")!=api:
        raise Denied("resolved_js_api_version_drift")
    for key,entry in packages.items():
        if key=="":
            continue
        if not isinstance(key,str) or ".." in key.split("/") or not isinstance(entry,dict):
            raise Denied("invalid_npm_package")
        if entry.get("resolved") is not None and not str(entry.get("integrity","")).startswith(("sha512-","sha384-","sha256-")):
            raise Denied("npm_resolution_integrity_missing")
    notices=audit_licenses(Path(license_root),source_sha,require_upstream=True)
    if not notices["actualBytesVerified"] or not notices["upstreamRetrievalVerified"]:
        raise Denied("upstream_license_bytes_not_verified")
    return {
        "schema":"lu.ide.p06.resolved-dependency-evidence.v1",
        "sourceSha":source_sha,"rustTauri":rust,"javascriptTauri":api,
        "npmLockSha256":nsha,"cargoLockSha256":csha,
        "npmPackages":len(packages)-1,"cargoPackages":len(crates),
        "upstreamSourcesVerified":True,
        "unresolvedLicenseEntries":notices["unresolved"],
        "lockfilesCommittedAndReviewed":True,
        "nativeInstalledAccepted":False,"signedPayloadAccepted":False,
        "licenseLegalReviewApproved":False,"releaseApproved":False
    }

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",type=Path,required=True)
    p.add_argument("--source-sha",required=True)
    p.add_argument("--license-root",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    args=p.parse_args()
    try:
        if args.output.exists() or args.output.is_symlink():
            raise Denied("receipt_must_be_new")
        record=inspect(args.repo_root,args.source_sha,args.license_root)
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(record,sort_keys=True,indent=2)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps(record,sort_keys=True))
    return 0
if __name__=="__main__": raise SystemExit(main())
