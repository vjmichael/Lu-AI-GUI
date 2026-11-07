#!/usr/bin/env python3
"""P04 *verification-package* resolved license/notice inventory.

Local, deterministic, no downloads. Reads actual installed npm packages and
Cargo metadata/crate files (not merely top-level package declarations).
Copies complete accompanying notices plus reviewed upstream core license texts.
Missing or ambiguous license bytes are explicitly UNRESOLVED, never silently
declared cleared. --require-complete rejects all unresolved records.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

LICENSE_RE = re.compile(r"^(?:license|licence|copying|copyright|notice)(?:[._ -].*)?$", re.I)
MAX_TEXT = 2_000_000
MAX_PACKAGES = 4000
UPSTREAM = {
    "LLVM-18.1.8-LICENSE.txt": "llvmorg-18.1.8",
    "Python-3.10.11-LICENSE.txt": "v3.10.11",
    "Tauri-LICENSE-MIT.txt": "tauri-2e23492ab9bcec06bab041e0e47447aaa0da298c",
    "Tauri-LICENSE-APACHE-2.0.txt": "tauri-2e23492ab9bcec06bab041e0e47447aaa0da298c",
    "CodeMirror-6-LICENSE-MIT.txt": "codemirror-view-6.0.0",
    "esbuild-0.25.9-LICENSE-MIT.txt": "esbuild-v0.25.9",
}
SHA40 = re.compile(r"^[0-9a-f]{40}$", re.I)

class Denied(ValueError):
    pass

def digest(path):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for blob in iter(lambda: stream.read(1024*1024), b""):
            h.update(blob)
    return h.hexdigest()

def files_for(root):
    # Never follow vendor-created links, and never glob outside an actual
    # package's own top level. Preserve multiple copyright/NOTICE files.
    out=[]
    if not root.is_dir() or root.is_symlink():
        return out
    for child in sorted(root.iterdir(), key=lambda p:p.name.casefold()):
        if child.is_symlink() or not child.is_file():
            continue
        if LICENSE_RE.fullmatch(child.name) and 0<child.stat().st_size<=MAX_TEXT:
            out.append(child)
    return out[:16]

def npm_packages(root):
    node=root/"ide"/"app"/"node_modules"
    if not node.is_dir():
        return [], ["npm_node_modules_not_installed"]
    rows=[]
    for base,dirs,names in os.walk(node, followlinks=False):
        here=Path(base)
        dirs[:]=sorted([d for d in dirs if d not in {".bin", ".cache"} and not (here/d).is_symlink()])
        if "package.json" not in names or here == node:
            continue
        manifest=here/"package.json"
        if manifest.is_symlink() or manifest.stat().st_size>1024*1024:
            raise Denied("untrusted_npm_manifest")
        try:
            data=json.loads(manifest.read_text(encoding="utf-8"))
        except (OSError,UnicodeDecodeError,ValueError) as err:
            raise Denied("malformed_installed_npm_manifest") from err
        if not isinstance(data,dict):
            raise Denied("invalid_npm_manifest_object")
        name,version=data.get("name"),data.get("version")
        if not isinstance(name,str) or not name or not isinstance(version,str) or not version:
            continue # enclosing non-package workspaces are not resolved crates
        rows.append(("npm",name,version,data.get("license"),here))
        if len(rows)>MAX_PACKAGES:
            raise Denied("unbounded_npm_inventory")
    return rows, []

def cargo_packages(root):
    manifest=root/"ide"/"app"/"src-tauri"/"Cargo.toml"
    try:
        out=subprocess.run(
            ["cargo","metadata","--offline","--format-version","1",
             "--manifest-path",str(manifest)],
            text=True,encoding="utf-8",errors="strict",
            capture_output=True,timeout=90,check=False,
        )
        if out.returncode or not isinstance(out.stdout,str) or len(out.stdout)>25*1024*1024:
            return [], ["cargo_offline_metadata_unavailable"]
        data=json.loads(out.stdout)
    except (OSError,ValueError,subprocess.TimeoutExpired):
        return [], ["cargo_offline_metadata_unavailable"]
    rows=[]
    for pkg in data.get("packages",[]):
        name,version,origin=pkg.get("name"),pkg.get("version"),pkg.get("manifest_path")
        if not all(isinstance(x,str) and x for x in (name,version,origin)):
            raise Denied("invalid_cargo_resolved_package")
        folder=Path(origin).parent
        if folder.is_symlink() or not folder.is_dir():
            raise Denied("invalid_cargo_package_location")
        rows.append(("cargo",name,version,pkg.get("license"),folder))
    if len(rows)>MAX_PACKAGES:
        raise Denied("unbounded_cargo_inventory")
    return rows,[]

def bundle(repo_root:Path, output:Path, source_sha:str, require_complete=False):
    if not SHA40.fullmatch(source_sha):
        raise Denied("source_sha_missing")
    repo_root=repo_root.resolve(strict=True)
    if not (repo_root/"ide/app/package.json").is_file():
        raise Denied("wrong_lu_ide_source_tree")
    if output.exists() or output.is_symlink():
        raise Denied("output_must_be_new")
    packages,errors=npm_packages(repo_root)
    cargo,cargo_errors=cargo_packages(repo_root)
    packages+=cargo
    errors+=cargo_errors
    if not packages:
        errors.append("no_resolved_runtime_components")
    # Dedupe Cargo workspace references sharing an identical resolved source.
    unique={}
    for manager,name,version,declared,folder in packages:
        key=(manager,name,version,str(folder.resolve()))
        unique[key]=(manager,name,version,declared,folder)
    if len(unique)>MAX_PACKAGES:
        raise Denied("unbounded_resolved_dependencies")
    sorted_rows=sorted(unique.values(),key=lambda r:(r[0],r[1].casefold(),r[2],str(r[4])))
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".lu-ide-license-stage-",dir=output.parent) as tmp:
        stage=Path(tmp)/"notices"
        stage.mkdir()
        copied=[]
        upstream_dir=repo_root/"ide"/"packaging"/"upstream-licenses"
        for name,source in UPSTREAM.items():
            path=upstream_dir/name
            if path.is_symlink() or not path.is_file() or not 100<=path.stat().st_size<=MAX_TEXT:
                raise Denied("required_upstream_license_missing:"+name)
            target=stage/"upstream"/name
            target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,target,follow_symlinks=False)
            copied.append({"path":target.relative_to(stage).as_posix(),
                           "sha256":digest(target),"sourceRevision":source})
        results=[]
        for index,(manager,name,version,declared,folder) in enumerate(sorted_rows):
            files=files_for(folder)
            notices=[]
            for no,src in enumerate(files):
                if b"\x00" in src.read_bytes()[:4096]:
                    continue
                relative=f"resolved/{manager}/{index:04d}-{no:02d}.txt"
                target=stage/relative
                target.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(src,target,follow_symlinks=False)
                notices.append({"path":relative,"sha256":digest(target),
                                "originalName":src.name})
            license_id=declared if isinstance(declared,str) and declared.strip() else None
            # The package name/metadata is evidence of resolution, not a legal
            # clearance. Missing text or identifier blocks complete status.
            if not notices or not license_id:
                errors.append(f"unresolved_notice_or_id:{manager}:{name}:{version}")
            results.append({"manager":manager,"name":name,"version":version,
                            "declaredLicense":license_id,"notices":notices})
        locks=[]
        for manager,relative in (
            ("npm","ide/app/package-lock.json"),
            ("cargo","ide/app/src-tauri/Cargo.lock"),
        ):
            src=repo_root/relative
            if src.is_symlink() or not src.is_file() or not 100<=src.stat().st_size<=8*1024*1024:
                errors.append("resolved_lockfile_missing_or_untrusted:"+manager)
                continue
            target=stage/"dependency-locks"/(manager+".lock")
            target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(src,target,follow_symlinks=False)
            locks.append({"manager":manager,"sourcePath":relative,
                          "path":target.relative_to(stage).as_posix(),
                          "sha256":digest(target)})
        report={"schema":"lu.ide.p04.verification-licenses.v1",
                "sourceSha":source_sha,
                "packageCount":len(results),
                "upstreamLicenses":copied,
                "dependencyLocks":locks,
                "resolvedPackages":results,
                "unresolved":sorted(set(errors)),
                "completeForReleaseReview":not errors,
                "releaseApproved":False,
                "licenseLegalReviewApproved":False}
        (stage/"inventory.json").write_text(json.dumps(report,indent=2,sort_keys=True)+"\n",encoding="utf-8")
        (stage/"README.txt").write_text(
            "Lu IDE verification-package third-party license inventory\n"
            "These notices accompany unsigned Windows/Linux test installers.\n"
            "Source SHA: "+source_sha+"\n"
            "Resolved npm/Cargo package notices copied from local installed sources.\n"
            "Unresolved count: "+str(len(set(errors)))+"\n"
            "Release approved: NO. Independently review the final exact binary,\n"
            "dynamic native libraries, additional required notices and applicable\n"
            "licenses before any distribution approval.\n",encoding="utf-8")
        if require_complete and errors:
            raise Denied("unreviewed_resolved_dependency_notices")
        os.replace(stage,output)
    return report

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--source-sha",required=True)
    p.add_argument("--require-complete",action="store_true")
    a=p.parse_args()
    try:
        report=bundle(a.repo_root,a.output,a.source_sha,a.require_complete)
    except (ValueError,TypeError,OSError) as exc:
        print("DENIED: "+(str(exc) if isinstance(exc,Denied) else "source_inventory_unavailable"),file=sys.stderr)
        return 1
    print(json.dumps({"packageCount":report["packageCount"],
                      "unresolved":len(report["unresolved"]),
                      "bundledForNativeVerification":True,
                      "releaseApproved":False},sort_keys=True))
    return 0
if __name__=="__main__":
    raise SystemExit(main())
