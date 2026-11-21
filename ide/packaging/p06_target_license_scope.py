#!/usr/bin/env python3
"""Scope unresolved third-party notices to a Cargo target graph, without waivers.

cargo metadata --filter-platform identifies a target dependency *resolution*
graph, NOT proof that code, a notice, or a binary was actually distributed.
All unresolved entries remain open until independently reviewed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

TARGETS = {
    "windows-2022": "x86_64-pc-windows-msvc",
    "ubuntu-24.04": "x86_64-unknown-linux-gnu",
}
HEX40 = re.compile(r"^[0-9a-f]{40}$", re.I)

class ScopeDenied(ValueError):
    pass

def scope(inventory, metadata, original_inventory_bytes, metadata_bytes,
          source_sha, platform, target_triple):
    if platform not in TARGETS or target_triple != TARGETS[platform]:
        raise ScopeDenied("unreviewed_cargo_target")
    if not isinstance(source_sha,str) or not HEX40.fullmatch(source_sha):
        raise ScopeDenied("unbound_exact_source")
    if (not isinstance(inventory,dict)
            or inventory.get("schema")!="lu.ide.p04.verification-licenses.v1"
            or inventory.get("sourceSha")!=source_sha
            or inventory.get("releaseApproved") is not False
            or inventory.get("licenseLegalReviewApproved") is not False
            or not isinstance(inventory.get("unresolved"),list)
            or not isinstance(inventory.get("resolvedPackages"),list)
            or not isinstance(inventory.get("upstreamRetrieval"),dict)
            or inventory["upstreamRetrieval"].get("allSourcesRetrieved") is not True):
        raise ScopeDenied("untrusted_unresolved_inventory")
    if not isinstance(metadata,dict) or not isinstance(metadata.get("packages"),list):
        raise ScopeDenied("invalid_native_cargo_metadata")
    resolved=metadata.get("resolve")
    if not isinstance(resolved,dict) or not isinstance(resolved.get("nodes"),list):
        raise ScopeDenied("native_cargo_resolve_graph_missing")
    packages={}
    mapping={}
    for pkg in metadata["packages"]:
        if not isinstance(pkg,dict) or not all(
                isinstance(pkg.get(field),str) and pkg[field]
                for field in ("id","name","version")):
            raise ScopeDenied("invalid_cargo_metadata_package")
        if pkg["id"] in packages:
            raise ScopeDenied("duplicate_cargo_metadata_id")
        packages[pkg["id"]]=pkg
        mapping.setdefault((pkg["name"],pkg["version"]),set()).add(pkg["id"])
    selected=set()
    for node in resolved["nodes"]:
        if not isinstance(node,dict) or not isinstance(node.get("id"),str):
            raise ScopeDenied("invalid_native_cargo_resolve_node")
        ident=node["id"]
        if ident in selected or ident not in packages:
            raise ScopeDenied("duplicate_or_unknown_cargo_resolve_node")
        selected.add(ident)
    if not selected:
        raise ScopeDenied("empty_native_cargo_resolve_graph")
    inventory_pkgs=set()
    for p in inventory["resolvedPackages"]:
        if not isinstance(p,dict) or any(not isinstance(p.get(k),str)
                 for k in ("manager","name","version")):
            raise ScopeDenied("untrusted_resolved_package_record")
        inventory_pkgs.add((p["manager"],p["name"],p["version"]))
    tasks=[]
    for error in inventory["unresolved"]:
        if not isinstance(error,str) or not error:
            raise ScopeDenied("invalid_unresolved_notice")
        parts=error.split(":",3)
        if len(parts)==4 and parts[0]=="unresolved_notice_or_id":
            manager,name,version=parts[1:]
            if (manager,name,version) not in inventory_pkgs:
                raise ScopeDenied("unresolved_notice_without_resolved_package")
            if manager=="cargo":
                candidate_ids=mapping.get((name,version),set())
                if len(candidate_ids)>1:
                    relation="ambiguous_multiple_cargo_sources"
                elif not candidate_ids:
                    relation="not_in_filtered_metadata_package_inventory"
                else:
                    relation=("selected_in_target_resolve_graph" if
                              next(iter(candidate_ids)) in selected else
                              "not_selected_in_target_resolve_graph")
            else:
                relation="not_cargo_package"
            tasks.append({"unresolvedRecord":error,"targetGraphRelation":relation,
                "disposition":"open","copyrightCleared":False,
                "binaryShipmentProven":False})
        else:
            tasks.append({"unresolvedRecord":error,
                "targetGraphRelation":"source_resolution_error",
                "disposition":"open","copyrightCleared":False,
                "binaryShipmentProven":False})
    tally={}
    for task in tasks:
        key=task["targetGraphRelation"]
        tally[key]=tally.get(key,0)+1
    return {"schema":"lu.ide.p06.target-license-scope.v1",
            "sourceSha":source_sha,"platform":platform,"targetTriple":target_triple,
            "inventorySha256":hashlib.sha256(original_inventory_bytes).hexdigest(),
            "cargoMetadataSha256":hashlib.sha256(metadata_bytes).hexdigest(),
            "targetResolvePackageCount":len(selected),
            "unresolvedCount":len(tasks),"targetGraphRelations":tally,
            "tasks":tasks,"allUnresolvedRemainOpen":True,
            "binaryShipmentProven":False,"legalReviewApproved":False,
            "releaseApproved":False}

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--inventory",type=Path,required=True)
    p.add_argument("--metadata",type=Path,required=True)
    p.add_argument("--source-sha",required=True)
    p.add_argument("--platform",choices=tuple(TARGETS),required=True)
    p.add_argument("--target-triple",required=True)
    p.add_argument("--output",type=Path,required=True)
    a=p.parse_args()
    try:
        if a.output.exists() or a.output.is_symlink():
            raise ScopeDenied("target_scope_output_must_be_new")
        for file in (a.inventory,a.metadata):
            if file.is_symlink() or not file.is_file() or not 100<=file.stat().st_size<=35_000_000:
                raise ScopeDenied("untrusted_metadata_source")
        raw=a.inventory.read_bytes()
        source=a.metadata.read_bytes()
        data=scope(json.loads(raw),json.loads(source),raw,source,
            a.source_sha,a.platform,a.target_triple)
        a.output.parent.mkdir(parents=True,exist_ok=True)
        a.output.write_text(json.dumps(data,indent=2,sort_keys=True)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as exc:
        print("DENIED: "+str(exc),file=sys.stderr)
        return 2
    print(json.dumps({"unresolvedCount":data["unresolvedCount"],
        "targetResolvePackageCount":data["targetResolvePackageCount"],
        "relations":data["targetGraphRelations"],"releaseApproved":False},sort_keys=True))
    return 0

if __name__=="__main__": raise SystemExit(main())
