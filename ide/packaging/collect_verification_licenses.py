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
from upstream_notice_receipt import SOURCES

LICENSE_RE = re.compile(r"^(?:license|licence|copying|copyright|notice)(?:[._ -].*)?$", re.I)
MAX_TEXT = 2_000_000
MAX_PACKAGES = 4000
UPSTREAM = {
    "LLVM-18.1.8-LICENSE.txt": "llvmorg-18.1.8",
    "Python-3.10.11-LICENSE.txt": "v3.10.11",
    "Tauri-LICENSE-MIT.txt": "tauri-v2.12.5",
    "Tauri-LICENSE-APACHE-2.0.txt": "tauri-v2.12.5",
    "CodeMirror-6-LICENSE-MIT.txt": "6.0.0",
    "esbuild-0.25.9-LICENSE-MIT.txt": "v0.25.9",
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

# r-efi's exact released original licensing/copyright notice is AUTHORS,
# not LICENSE.*. Compare the actual Cargo-locked .crate AUTHORS file with the
# pinned upstream release bytes and their Git source blob identities. A
# different version, expression, missing file or changed copyright never
# inherits an MIT notice. This retrieves documentary evidence, not a legal
# election of MIT/Apache/LGPL and not an acceptance of binary shipment.
R_EFI_AUTHOR_GIT_BLOBS = {
    "5.3.0": "7397b2bd6a8580d51fe7b23783f8f1a05fa70291",
    "6.0.0": "79c8b7c4a1dabcfbc718b7fe6d43b1892f9850cd",
}
R_EFI_LICENSE = "MIT OR Apache-2.0 OR LGPL-2.1-or-later"

def git_blob_sha(content):
    return hashlib.sha1(b"blob " + str(len(content)).encode() + bytes([0]) + content).hexdigest()

def verified_r_efi_authors(manager, name, version, declared, folder, repo_root):
    if manager != "cargo" or name != "r-efi" or version not in R_EFI_AUTHOR_GIT_BLOBS:
        return None
    if declared != R_EFI_LICENSE:
        raise Denied("r_efi_exact_license_expression_changed")
    local = folder / "AUTHORS"
    reviewed = repo_root / "ide/packaging/upstream-crate-notices" / ("r-efi-"+version+"-AUTHORS.txt")
    if not local.exists():
        return None
    for candidate in (local,reviewed):
        if (candidate.is_symlink() or not candidate.is_file()
                or not 100 <= candidate.stat().st_size <= MAX_TEXT):
            raise Denied("untrusted_r_efi_original_authors")
        data = candidate.read_bytes()
        if git_blob_sha(data) != R_EFI_AUTHOR_GIT_BLOBS[version]:
            raise Denied("r_efi_source_notice_differs_from_exact_upstream_tag")
    if local.read_bytes() != reviewed.read_bytes():
        raise Denied("r_efi_crate_notice_differs_from_reviewed_source")
    return local

# These ONLY cover exact package versions from reviewed upstream release tags.
# The canonical original license is archived verbatim in this repository.
# No SPDX declaration alone can substitute for byte-proven provenance.
CURATED_MISSING_NOTICES = {
    "alloc-stdlib:0.3.0": {
        "declaredLicense": "BSD-3-Clause",
        "files": [
            {
                "checkedIn": "ide/packaging/upstream-crate-notices/alloc-stdlib-0.3.0-LICENSE.txt",
                "originalName": "LICENSE",
                "url": "https://raw.githubusercontent.com/dropbox/rust-alloc-no-stdlib/0.3.0/LICENSE",
                "blobSha": "cd496ba72eb37e83a44958358d1f89a8a28cbc15"
            }
        ]
    },
    "tauri-plugin:2.7.2": {
        "declaredLicense": "Apache-2.0 OR MIT",
        "files": [
            {
                "checkedIn": "ide/packaging/upstream-crate-notices/tauri-plugin-2.7.2-LICENSE-MIT.txt",
                "originalName": "LICENSE-MIT",
                "url": "https://raw.githubusercontent.com/tauri-apps/tauri/tauri-plugin-v2.7.2/LICENSE-MIT",
                "blobSha": "b08530d59433b3060f4fad8751ac2610c0b0aff3"
            },
            {
                "checkedIn": "ide/packaging/upstream-crate-notices/tauri-plugin-2.7.2-LICENSE-APACHE-2.0.txt",
                "originalName": "LICENSE-APACHE-2.0",
                "url": "https://raw.githubusercontent.com/tauri-apps/tauri/tauri-plugin-v2.7.2/LICENSE-APACHE-2.0",
                "blobSha": "f433b1a53f5b830a205fd2df78e2b34974656c7b"
            }
        ]
    }
}

# MIT applies to the Rust crates; it is NOT evidence of legal status of
# the bundled Google-built protoc executable. A separate binary review stays
# required even if the complete original crate LICENSE.txt is recovered.
PROTOC_UPSTREAM_FAMILY = (["protoc-bin-vendored","protoc-bin-vendored-linux-aarch_64","protoc-bin-vendored-linux-ppcle_64","protoc-bin-vendored-linux-s390_64","protoc-bin-vendored-linux-x86_32","protoc-bin-vendored-linux-x86_64","protoc-bin-vendored-macos-aarch_64","protoc-bin-vendored-macos-x86_64","protoc-bin-vendored-win32"])
PROTOC_ORIGINAL_LICENSE = {"checkedIn":"ide/packaging/upstream-crate-notices/protoc-bin-vendored-3.3.0-LICENSE.txt","originalName":"LICENSE.txt","url":"https://raw.githubusercontent.com/stepancheg/rust-protoc-bin-vendored/v3.3.0/LICENSE.txt","blobSha":"acce6396f3cd1990471e8415ddf190780fa255b5"}
for _protoc_name in PROTOC_UPSTREAM_FAMILY:
    if _protoc_name+":3.3.0" in CURATED_MISSING_NOTICES:
        raise Denied("duplicate_protoc_notice_rule")
    CURATED_MISSING_NOTICES[_protoc_name+":3.3.0"] = {
        "declaredLicense":"MIT",
        "files":[PROTOC_ORIGINAL_LICENSE],
    }


# MIT original for three exact Rust bindings; NOT Microsoft WebView2 runtime rights.
WEBVIEW_ORIGINAL_LICENSE = {"checkedIn":"ide/packaging/upstream-crate-notices/webview2-rs-0.39.1-LICENSE.txt","originalName":"LICENSE","url":"https://raw.githubusercontent.com/wravery/webview2-rs/edc2caf886175ccaebe86078c9cfe1ae2a187328/LICENSE","blobSha":"de9a7e660e173136b351982a34f715af424ff812"}
WEBVIEW_EXACT_CRATES = [["webview2-com","0.39.1"],["webview2-com-sys","0.39.1"],["webview2-com-macros","0.8.1"]]
for _name,_version in WEBVIEW_EXACT_CRATES:
    _key=_name+":"+_version
    if _key in CURATED_MISSING_NOTICES:
        raise Denied("duplicate_webview_original_notice")
    CURATED_MISSING_NOTICES[_key]={"declaredLicense":"MIT","files":[WEBVIEW_ORIGINAL_LICENSE]}

# Original upstream licensing policy, NOT complete terms or Apple SDK rights.
OBJC2_UPSTREAM_POLICY = [{"name":"block2","version":"0.6.2","declared":"MIT","tag":"block2-0.6.2"},{"name":"dispatch2","version":"0.3.1","declared":"Zlib OR Apache-2.0 OR MIT","tag":"dispatch2-0.3.1"},{"name":"objc2","version":"0.6.5","declared":"MIT","tag":"objc2-0.6.5"},{"name":"objc2-encode","version":"4.1.0","declared":"MIT","tag":"objc2-encode-4.1.0"},{"name":"objc2-exception-helper","version":"0.1.1","declared":"Zlib OR Apache-2.0 OR MIT","tag":"objc2-exception-helper-0.1.1"},{"name":"objc2-app-kit","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-cloud-kit","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-data","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-foundation","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-graphics","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-image","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-location","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-core-text","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-foundation","version":"0.3.2","declared":"MIT","tag":"frameworks-0.3.2"},{"name":"objc2-io-surface","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-quartz-core","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-ui-kit","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-user-notifications","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"},{"name":"objc2-web-kit","version":"0.3.2","declared":"Zlib OR Apache-2.0 OR MIT","tag":"frameworks-0.3.2"}]
for _rule in OBJC2_UPSTREAM_POLICY:
    _key=_rule["name"]+":"+_rule["version"]
    if _key in CURATED_MISSING_NOTICES:
        raise Denied("duplicate_objc2_rule")
    CURATED_MISSING_NOTICES[_key]={
        "declaredLicense":_rule["declared"],
        "files":[{
            "checkedIn":"ide/packaging/upstream-crate-notices/objc2-family-LICENSE.md",
            "originalName":"LICENSE.md",
            "url":"https://raw.githubusercontent.com/madsmtm/objc2/"+_rule["tag"]+"/LICENSE.md",
            "blobSha":"540ae4c81adb5f64a9f61e222b688880da24300e"}],
    }

# Exact upstream Rust texts do not grant external native SDK or system lib rights.
EXTERNAL_NATIVE_SDK_SOURCE_RULES = [{"name":"libappindicator-sys","version":"0.9.0","declared":"Apache-2.0 OR MIT","tag":"libappindicator-sys-v0.9.0","external":"Linux system libappindicator shared library","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/libappindicator-sys-0.9.0-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/tauri-apps/libappindicator-rs/libappindicator-sys-v0.9.0/LICENSE-MIT","blobSha":"50e79e21ccd1454f00695108ae01eaff9569689a"},{"checkedIn":"ide/packaging/upstream-crate-notices/libappindicator-sys-0.9.0-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/tauri-apps/libappindicator-rs/libappindicator-sys-v0.9.0/LICENSE-APACHE","blobSha":"16fe87b06e802f094b3fbb0894b137bca2b16ef1"}]},{"name":"jni-sys-macros","version":"0.4.1","declared":"MIT OR Apache-2.0","tag":"v0.4.1-macros","external":"Java JNI native interface","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/jni-sys-macros-0.4.1-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/jni-rs/jni-sys/v0.4.1-macros/LICENSE-MIT","blobSha":"2008402b8a1299cf118ea38e02116bfcc72e53c3"},{"checkedIn":"ide/packaging/upstream-crate-notices/jni-sys-macros-0.4.1-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/jni-rs/jni-sys/v0.4.1-macros/LICENSE-APACHE","blobSha":"8f71f43fee3f78649d238238cbde51e6d7055c82"}]},{"name":"ndk-context","version":"0.1.1","declared":"MIT OR Apache-2.0","tag":"ndk-context-0.1.1","external":"Android NDK API integration","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-context-0.1.1/LICENSE-MIT","blobSha":"9cf106272ac3b56b0c4c80218e8fc10a664ca5f4"},{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-context-0.1.1/LICENSE-APACHE","blobSha":"261eeb9e9f8b2b4b0d119366dda99c6fd7d35c64"}]},{"name":"ndk-sys","version":"0.6.0+11769913","declared":"MIT OR Apache-2.0","tag":"ndk-sys-0.6.0","external":"Android NDK API integration","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-sys-0.6.0/LICENSE-MIT","blobSha":"9cf106272ac3b56b0c4c80218e8fc10a664ca5f4"},{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-sys-0.6.0/LICENSE-APACHE","blobSha":"261eeb9e9f8b2b4b0d119366dda99c6fd7d35c64"}]},{"name":"ndk","version":"0.9.0","declared":"MIT OR Apache-2.0","tag":"ndk-0.9.0","external":"Android NDK API integration","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-0.9.0/LICENSE-MIT","blobSha":"9cf106272ac3b56b0c4c80218e8fc10a664ca5f4"},{"checkedIn":"ide/packaging/upstream-crate-notices/rust-mobile-ndk-original-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/rust-mobile/ndk/ndk-0.9.0/LICENSE-APACHE","blobSha":"261eeb9e9f8b2b4b0d119366dda99c6fd7d35c64"}]},{"name":"winapi-i686-pc-windows-gnu","version":"0.4.0","declared":"MIT/Apache-2.0","tag":"0.3.9","external":"Windows GNU import libraries","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/winapi-gnu-0.4.0-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/retep998/winapi-rs/0.3.9/LICENSE-MIT","blobSha":"6f1b4c850cd2bd0d165f122c0274afc95e10a851"},{"checkedIn":"ide/packaging/upstream-crate-notices/winapi-gnu-0.4.0-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/retep998/winapi-rs/0.3.9/LICENSE-APACHE","blobSha":"8dada3edaf50dbc082c9a125058f25def75e625a"}]},{"name":"winapi-x86_64-pc-windows-gnu","version":"0.4.0","declared":"MIT/Apache-2.0","tag":"0.3.9","external":"Windows GNU import libraries","files":[{"checkedIn":"ide/packaging/upstream-crate-notices/winapi-gnu-0.4.0-LICENSE-MIT.txt","originalName":"LICENSE-MIT","url":"https://raw.githubusercontent.com/retep998/winapi-rs/0.3.9/LICENSE-MIT","blobSha":"6f1b4c850cd2bd0d165f122c0274afc95e10a851"},{"checkedIn":"ide/packaging/upstream-crate-notices/winapi-gnu-0.4.0-LICENSE-APACHE.txt","originalName":"LICENSE-APACHE","url":"https://raw.githubusercontent.com/retep998/winapi-rs/0.3.9/LICENSE-APACHE","blobSha":"8dada3edaf50dbc082c9a125058f25def75e625a"}]}]
for _r in EXTERNAL_NATIVE_SDK_SOURCE_RULES:
    _key=_r['name']+':'+_r['version']
    if _key in CURATED_MISSING_NOTICES:
        raise Denied('duplicate_external_sdk_notice')
    CURATED_MISSING_NOTICES[_key]={'declaredLicense':_r['declared'],'files':_r['files']}

def verified_curated_notices(repo_root,manager,name,version,declared):
    if manager != "cargo":
        return []
    rule = CURATED_MISSING_NOTICES.get(name + ":" + version)
    if rule is None:
        return []
    if declared != rule["declaredLicense"]:
        raise Denied("curated_notice_exact_license_expression_drift")
    originals=[]
    for item in rule["files"]:
        file=repo_root/item["checkedIn"]
        if (file.is_symlink() or not file.is_file()
                or not 100 <= file.stat().st_size <= MAX_TEXT):
            raise Denied("missing_or_unsafe_curated_source_notice")
        if git_blob_sha(file.read_bytes()) != item["blobSha"]:
            raise Denied("curated_notice_differs_from_exact_original_release")
        originals.append((file,item))
    return originals

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

def bundle(repo_root:Path, output:Path, source_sha:str, require_complete=False, upstream_receipt=None):
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
        retrieval = None
        if upstream_receipt is not None:
            receipt_path = Path(upstream_receipt)
            if receipt_path.is_symlink() or not receipt_path.is_file() or receipt_path.stat().st_size > 50000:
                raise Denied("invalid_upstream_fetch_receipt")
            fetched = json.loads(receipt_path.read_text(encoding="utf-8"))
            sources = fetched.get("sources", []) if isinstance(fetched, dict) else []
            if (fetched.get("schema") != "lu.ide.upstream-notice-fetch.v1"
                    or fetched.get("sourceSha") != source_sha
                    or fetched.get("allSourcesRetrieved") is not True
                    or fetched.get("releaseApproved") is not False
                    or fetched.get("licenseLegalReviewApproved") is not False
                    or not isinstance(sources, list)
                    or len(sources) != len(UPSTREAM)
                    or {row.get("name") for row in sources} != set(UPSTREAM)):
                raise Denied("unverified_upstream_license_receipt")
            verified = {row["name"]: row for row in sources}
            for row in copied:
                name = Path(row["path"]).name
                item = verified[name]
                source_rev, source_url = SOURCES[name]
                if (item.get("sourceRevision") != source_rev
                        or row.get("sourceRevision") != source_rev
                        or item.get("sourceUrl") != source_url
                        or item.get("matchesUpstream") is not True
                        or item.get("bundledSourceSha256") != row["sha256"]
                        or not isinstance(item.get("sourceSha256"), str)
                        or len(item["sourceSha256"]) != 64):
                    raise Denied("unverified_upstream_notice_source:" + name)
            target = stage / "upstream-fetch.json"
            shutil.copyfile(receipt_path, target, follow_symlinks=False)
            retrieval = {"path": "upstream-fetch.json", "sha256": digest(target),
                         "sourceCount": len(UPSTREAM), "allSourcesRetrieved": True}
        results=[]
        for index,(manager,name,version,declared,folder) in enumerate(sorted_rows):
            files=files_for(folder)
            curated={}
            if not files:
                for original,origin in verified_curated_notices(
                        repo_root,manager,name,version,declared):
                    files.append(original)
                    curated[original]=origin
            authors=verified_r_efi_authors(manager,name,version,declared,folder,repo_root)
            if authors is not None and authors not in files:
                files.append(authors)
            notices=[]
            for no,src in enumerate(files):
                if b"\x00" in src.read_bytes()[:4096]:
                    continue
                relative=f"resolved/{manager}/{index:04d}-{no:02d}.txt"
                target=stage/relative
                target.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(src,target,follow_symlinks=False)
                notice={"path":relative,"sha256":digest(target),
                                "originalName":src.name}
                if src in curated:
                    origin=curated[src]
                    notice["originalName"]=origin["originalName"]
                    notice["upstreamSource"]=origin["url"]
                    notice["upstreamGitBlobSha"]=origin["blobSha"]
                    for _r in EXTERNAL_NATIVE_SDK_SOURCE_RULES:
                        if name==_r['name'] and version==_r['version']:
                            notice['externalNativeBinaryReviewRequired']=True
                            notice['externalNativeDependency']=_r['external']
                    if any(r["name"]==name and r["version"]==version for r in OBJC2_UPSTREAM_POLICY):
                        notice["upstreamLegalTextCompleteness"]="policy_links_external_full_terms"
                        notice["appleSdkDerivativeReviewRequired"]=True
                if src.name=="AUTHORS" and manager=="cargo" and name=="r-efi" and version in R_EFI_AUTHOR_GIT_BLOBS:
                    notice["upstreamSource"]="https://raw.githubusercontent.com/r-efi/r-efi/v"+version+"/AUTHORS"
                    notice["upstreamGitBlobSha"]=R_EFI_AUTHOR_GIT_BLOBS[version]
                notices.append(notice)
            # The platform package @esbuild/<target> is built from the exact
            # esbuild source release, but its npm tarball has no LICENSE file.
            # Reuse only the verified, pinned upstream MIT text for that same
            # esbuild version. Never borrow it for other packages or versions.
            if (not notices and retrieval is not None and manager=="npm"
                    and name in {"@esbuild/linux-x64","@esbuild/win32-x64"}
                    and version=="0.25.9" and declared=="MIT"):
                reviewed=upstream_dir/"esbuild-0.25.9-LICENSE-MIT.txt"
                relative=f"resolved/npm/{index:04d}-00.txt"
                target=stage/relative
                target.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(reviewed,target,follow_symlinks=False)
                notices.append({"path":relative,"sha256":digest(target),
                                "originalName":"verified-esbuild-v0.25.9-upstream-LICENSE.md",
                                "upstreamSource":SOURCES["esbuild-0.25.9-LICENSE-MIT.txt"][1]})
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
                "upstreamRetrieval":retrieval,
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
    p.add_argument("--upstream-receipt",type=Path)
    a=p.parse_args()
    try:
        report=bundle(a.repo_root,a.output,a.source_sha,a.require_complete,a.upstream_receipt)
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
