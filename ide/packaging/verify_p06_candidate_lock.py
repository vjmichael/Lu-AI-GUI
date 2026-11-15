#!/usr/bin/env python3
"""For every unsigned verification package: pin the *resolved* OS lock bytes.

Reviewed hashes permit reproducible TEST dependency resolution, but do not
imply a signed commercial release or complete legal clearance.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path
import sys
import tomllib

class LockDenied(ValueError):pass

def sha(data):
    return hashlib.sha256(data).hexdigest()

# This opt-in installed-IDE successor review does NOT change or replace
# the original P06 lockfile review. Every field is compiled into this
# verifier, so a changed JSON review cannot self-authorize dependency drift.
IDE05_SUCCESSOR = {
    "schema": "lu.ide.p06.ide05-successor.v1",
    "purpose": "unsigned-installed-ci-only",
    "previousReviewCargoSha256": "d4312b60209f89181766b60c2982fb8b199e8ed9d6be1a9dcbdcb3dd07abf6b4",
    "previousReviewCargoBlob": "87d0045a4be1c78651f080300aba71c14dbe1f30",
    "newCargoSha256": "c7995fb9957bee2fb0f8247e232c5f7f9db3b3ef5d77d3afcbdc09f1ed9b36aa",
    "newCargoBlob": "69bc41f0cf739c3d77989531b14b28a472b16e4a",
    "unchangedCargoOutsideSupervisorSha256": "cbe657f1a2b150d0a45744e4a855a36382f6572f2fb9f6893e49758cc705d2e7",
    "newSupervisorBlockSha256": "c3db915c6dc08447057ccb628ecac1825489c594f6238a8393ecd01073a73dac",
    "changedLocalPackage": "lu-ide-execution-supervisor@0.1.0",
    "addedExistingDependencies": [
        "base64 0.22.1",
        "toml 0.8.2"
    ],
    "cargoPackages": 500,
    "npmRawSha256": "3c137aede8749007c062b80e53bef0c83241b73d21e4c692ae1718882e81a316",
    "reviewedNativeCompileHead": "b7fa7dc2f1d52a558653674f86e167a9e4a95f5d",
    "reviewedNativeRunId": 42742430229,
    "externalDependencyChanges": 0,
    "releaseApproved": False,
    "licenseLegalReviewApproved": False,
    "independentHumanApproval": False
}
IDE05_SUPERVISOR_DEPS = [
    "base64 0.22.1", "cap-fs-ext", "cap-std 4.0.3",
    "libc", "lu-ide-workspace", "rand", "serde", "serde_json",
    "sha2", "toml 0.8.2", "windows-sys 0.61.2",
]

# Independently pinned IDE-MERGE-02 source-only delta after the original
# P06 and IDE05 review. This does not change either historical baseline.
IDE_MERGE02_SUCCESSOR = {
    "schema": "lu.ide.p06.ide-merge02-cargo-successor.v1",
    "purpose": "unsigned-integrated-native-source-only",
    "previousCargoBlob": "69bc41f0cf739c3d77989531b14b28a472b16e4a",
    "previousCargoSha256": "c7995fb9957bee2fb0f8247e232c5f7f9db3b3ef5d77d3afcbdc09f1ed9b36aa",
    "newCargoBlob": "ec7461e0a64a8fa121cffda7ecb74333c6f6686c",
    "newCargoSha256": "b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5ac",
    "newDesktopBlockSha256": "a5e4a1867eeb99c821b9e0a144aae01fe2895c49717b37756f7519f5b72594e3c",
    "unchangedOutsideDesktopSha256": "73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4c",
    "changedLocalPackage": "lu-ide-desktop@0.1.0-alpha.1",
    "addedExistingDependencies": [
        "rand"
    ],
    "cargoPackages": 500,
    "externalDependencyChanges": 0,
    "sourceAuditHead": "38dc57d9a779879e9b4c361418352ff10e240fee",
    "releaseApproved": False,
    "licenseLegalReviewApproved": False,
    "independentlyAuthorizedMainMerge": False
}

def reviewed_ide05_successor(root, current_cargo, original_review, successor):
    if successor != IDE05_SUCCESSOR:
        raise LockDenied("ide05_successor_not_independently_pinned")
    if (original_review["cargoRawSha256"] != successor["previousReviewCargoSha256"]
            or original_review["sourceLockBlobSha"]["cargo"] != successor["previousReviewCargoBlob"]
            or original_review["npmPlatformRawSha256"]["ubuntu-24.04"] != successor["npmRawSha256"]
            or original_review["npmPlatformRawSha256"]["windows-2022"] != successor["npmRawSha256"]
            or original_review["cargoPackages"] != successor["cargoPackages"]):
        raise LockDenied("ide05_successor_prior_review_mismatch")
    # One and only one local-workspace dependency block may differ from
    # the 500-package P06 review. The other 499 complete blocks, including
    # all resolved registry versions and checksums, must remain byte-equal.
    pattern = re.compile(
        rb'\n\[\[package\]\]\nname = "lu-ide-execution-supervisor"\n.*?(?=\n\[\[package\]\]|\Z)',
        re.DOTALL,
    )
    blocks = list(pattern.finditer(current_cargo))
    if len(blocks) != 1:
        raise LockDenied("ide05_successor_local_package_not_unique")
    block = blocks[0].group(0)
    unchanged = current_cargo[:blocks[0].start()] + current_cargo[blocks[0].end():]
    if (sha(current_cargo) != successor["newCargoSha256"]
            or sha(block) != successor["newSupervisorBlockSha256"]
            or sha(unchanged) != successor["unchangedCargoOutsideSupervisorSha256"]):
        raise LockDenied("ide05_successor_lock_byte_delta_unreviewed")
    try:
        decoded = tomllib.loads(current_cargo.decode("utf-8"))
        rows = [r for r in decoded["package"]
                if r.get("name") == "lu-ide-execution-supervisor"]
        local = tomllib.loads(
            (root / "ide/supervisor/Cargo.toml").read_text(encoding="utf-8")
        )
    except (UnicodeError, ValueError, TypeError, KeyError, OSError) as exc:
        raise LockDenied("ide05_successor_local_manifest_unreadable") from exc
    if (len(decoded["package"]) != 500 or len(rows) != 1
            or rows[0].get("version") != "0.1.0"
            or rows[0].get("dependencies") != IDE05_SUPERVISOR_DEPS
            or local["dependencies"]["base64"].get("version") != "=0.22.1"
            or local["dependencies"]["toml"].get("version") != "=0.8.2"
            or local["features"]["package-uv-worker"] !=
                ["multilang-worker", "dep:toml", "dep:base64"]):
        raise LockDenied("ide05_successor_local_dependencies_unreviewed")
    return successor["newCargoSha256"]

def reviewed_merge02_successor(root, current_cargo, original_review, successor):
    """Verify a single local-desktop dependency delta on top of reviewed IDE05."""
    if successor != IDE_MERGE02_SUCCESSOR:
        raise LockDenied("merge02_successor_not_independently_pinned")
    if (successor["previousCargoSha256"] != IDE05_SUCCESSOR["newCargoSha256"]
            or successor["previousCargoBlob"] != IDE05_SUCCESSOR["newCargoBlob"]
            or sha(current_cargo) != successor["newCargoSha256"]):
        raise LockDenied("merge02_source_lock_bytes_unreviewed")
    pattern = re.compile(
        rb'\n\[\[package\]\]\nname = "lu-ide-desktop"\n.*?(?=\n\[\[package\]\]|\Z)',
        re.DOTALL,
    )
    blocks = list(pattern.finditer(current_cargo))
    if len(blocks) != 1:
        raise LockDenied("merge02_desktop_package_not_unique")
    match = blocks[0]
    block = match.group(0)
    other = current_cargo[:match.start()] + current_cargo[match.end():]
    if (sha(block) != successor["newDesktopBlockSha256"]
            or sha(other) != successor["unchangedOutsideDesktopSha256"]
            or block.count(b'\n "rand",') != 1):
        raise LockDenied("merge02_desktop_delta_unreviewed")
    # Removing precisely the one added local dependency must reproduce the
    # complete, byte-identical independently reviewed IDE05 Cargo graph.
    earlier = (current_cargo[:match.start()]
               + block.replace(b'\n "rand",', b'', 1)
               + current_cargo[match.end():])
    if sha(earlier) != successor["previousCargoSha256"]:
        raise LockDenied("merge02_external_dependency_or_other_delta")
    try:
        prior = json.loads((root / "ide/packaging/ide05-reviewed-lock-successor.json")
                           .read_text(encoding="utf-8"))
        reviewed_ide05_successor(root, earlier, original_review, prior)
        graph = tomllib.loads(current_cargo.decode("utf-8"))
        manifest = tomllib.loads(
            (root / "ide/app/src-tauri/Cargo.toml").read_text(encoding="utf-8")
        )
        desktop = [row for row in graph["package"]
                   if row.get("name") == "lu-ide-desktop"]
        rand = [row for row in graph["package"] if row.get("name") == "rand"]
    except (UnicodeError, ValueError, TypeError, KeyError, OSError) as exc:
        raise LockDenied("merge02_source_graph_or_manifest_unreadable") from exc
    expected = [
        "lu-ide-execution-supervisor", "lu-ide-workspace", "rand",
        "serde", "serde_json", "sha2", "tauri", "tauri-build",
        "tauri-plugin-dialog",
    ]
    if (len(graph["package"]) != successor["cargoPackages"]
            or len(desktop) != 1 or len(rand) != 1
            or desktop[0].get("version") != "0.1.0-alpha.1"
            or desktop[0].get("dependencies") != expected
            or manifest["dependencies"].get("rand") != "0.8"):
        raise LockDenied("merge02_local_desktop_dependency_unreviewed")
    return successor["newCargoSha256"]

def check(root,platform,review=None,ide05_successor=None,ide_merge02_successor=None):
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
    if ide05_successor is not None and ide_merge02_successor is not None:
        raise LockDenied("conflicting_scoped_reviews")
    if ide_merge02_successor is not None:
        expected_cargo = reviewed_merge02_successor(
            root, raw["cargo"], review, ide_merge02_successor)
    elif ide05_successor is not None:
        expected_cargo = reviewed_ide05_successor(root, raw["cargo"], review, ide05_successor)
    else:
        expected_cargo = review["cargoRawSha256"]
    if sha(raw["cargo"])!=expected_cargo:
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
            "ide05ScopedSuccessorAccepted":ide05_successor is not None or ide_merge02_successor is not None,
            "ideMerge02ScopedSuccessorAccepted":ide_merge02_successor is not None,
            "independentHumanApproval":False,
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
    p.add_argument("--ide05-successor",action="store_true",
                   help="explicit pinned IDE-05 successor; original P06 review stays unchanged")
    p.add_argument("--ide-merge02-successor",action="store_true",
                   help="additional pinned local-desktop delta, no legal/release approval")
    a=p.parse_args()
    try:
        successor=(json.loads((a.repo_root/"ide/packaging/ide05-reviewed-lock-successor.json")
                   .read_text(encoding="utf-8")) if a.ide05_successor else None)
        merge02=(json.loads((a.repo_root/"ide/packaging/ide-merge02-reviewed-lock-successor.json")
                  .read_text(encoding="utf-8")) if a.ide_merge02_successor else None)
        result=check(a.repo_root,a.platform,ide05_successor=successor,
                     ide_merge02_successor=merge02)
    except (ValueError,TypeError,KeyError,OSError) as e:
        print("DENIED: "+str(e),file=sys.stderr);return 2
    print(json.dumps(result,sort_keys=True))
    return 0
if __name__=="__main__":raise SystemExit(main())
