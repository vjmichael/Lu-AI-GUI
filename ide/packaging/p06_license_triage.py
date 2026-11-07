#!/usr/bin/env python3
"""Inventory-specific unresolved NOTICE review queue: no automatic legal clearance."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

class TriageDenied(ValueError): pass

def classify(manager,name):
    if manager=="cargo" and name in {"lu-ide-desktop","lu-ide-workspace"}:
        return "first_party_apache_notice", "Match the checked-in package Apache license and ownership notices."
    if manager=="npm" and name.startswith("@esbuild/"):
        return "esbuild_platform_binary_notice", "Confirm exact esbuild upstream version and bundled MIT copyright."
    if name.startswith(("objc2","dispatch2","block2")) or name=="selectors":
        return "candidate_apple_target", "Check actual target-feature graph; obtain copyright/license bytes before distribution if included."
    if name.startswith(("ndk","jni-sys")):
        return "candidate_android_target", "Check actual target-feature graph; do not infer non-shipment from crate name."
    if name.startswith("protoc-bin-vendored"):
        return "vendored_protobuf_binary", "Verify compiler binary redistribution and embedded upstream copyright by exact target/version."
    if name.startswith(("webview2","winapi-","r-efi")):
        return "candidate_windows_target", "Verify actual compiled package closure and exact upstream notice."
    if name.startswith("libappindicator"):
        return "candidate_linux_target", "Verify GTK/Linux package closure and exact upstream notice."
    return "missing_or_ambiguous_package_notice", "Retrieve and review exact upstream copyright text and relevant binary/linkage scope."

def triage(data, source_bytes):
    if (not isinstance(data,dict) or data.get("schema")!="lu.ide.p04.verification-licenses.v1"
            or data.get("releaseApproved") is not False
            or data.get("licenseLegalReviewApproved") is not False
            or not isinstance(data.get("resolvedPackages"),list)
            or not isinstance(data.get("unresolved"),list)
            or not isinstance(data.get("sourceSha"),str)
            or len(data["sourceSha"])!=40
            or not isinstance(data.get("upstreamRetrieval"),dict)
            or data["upstreamRetrieval"].get("allSourcesRetrieved") is not True):
        raise TriageDenied("untrusted_complete_license_inventory")
    if len(set(data["unresolved"]))!=len(data["unresolved"]):
        raise TriageDenied("duplicate_unresolved_record")
    pkgs={}
    for row in data["resolvedPackages"]:
        if not isinstance(row,dict):
            raise TriageDenied("invalid_resolved_package")
        key=(row.get("manager"),row.get("name"),row.get("version"))
        if key in pkgs or not all(isinstance(v,str) for v in key):
            raise TriageDenied("invalid_or_duplicate_package")
        pkgs[key]=row
    tasks=[]
    for error in data["unresolved"]:
        if not isinstance(error,str) or not error:
            raise TriageDenied("invalid_unresolved_record")
        parts=error.split(":",3)
        if len(parts)==4 and parts[0]=="unresolved_notice_or_id":
            manager,name,version=parts[1:]
            pkg=pkgs.get((manager,name,version))
            if pkg is None:
                raise TriageDenied("unresolved_without_resolved_package")
            category,action=classify(manager,name)
            tasks.append({"packageManager":manager,"packageName":name,"version":version,
                "declaredLicense":pkg.get("declaredLicense"),"noticeFileCount":len(pkg.get("notices",[])),
                "classification":category,"remediation":action,
                "targetRelevance":"requires_independent_native_binary_check",
                "disposition":"open","reviewedBy":None})
        else:
            tasks.append({"inventoryError":error,"classification":"source_resolution_error",
                          "disposition":"open","reviewedBy":None,
                          "remediation":"Repair missing dependency metadata or lock evidence and rerun."})
    # SPDX metadata is not a commercial-license approval. A complete source
    # notice is also not proof that license choice and obligations are met.
    flags=[]
    for (manager,name,version),row in pkgs.items():
        license_id=row.get("declaredLicense")
        if not isinstance(license_id,str) or not license_id:
            route="unknown_license"
            explanation="No trustworthy declared license: obtain original source terms."
        elif "MPL-2.0" in license_id:
            route="mpl_2_file_level_conditions"
            explanation="Check covered-file source, changes, attribution and binary distribution obligations."
        elif any(token in license_id for token in ("AGPL-","GPL-","LGPL-","SSPL-")):
            route=("multi_license_choice_review" if " OR " in license_id
                   else "copyleft_distribution_review")
            explanation="Independently determine and record the exact licensing option and its obligations; never infer forced LGPL where MIT/Apache alternatives are offered."
        else:
            continue
        flags.append({"manager":manager,"name":name,"version":version,
                      "declaredLicense":license_id,"reviewRoute":route,
                      "requiredReview":explanation,"disposition":"open",
                      "selectedLicenseOption":None,"reviewedBy":None})
    flags.sort(key=lambda x:(x["manager"],x["name"],x["version"]))
    return {"schema":"lu.ide.p06.license-triage.v1","sourceSha":data["sourceSha"],
            "inventorySha256":hashlib.sha256(source_bytes).hexdigest(),
            "totalResolvedPackages":len(pkgs),"unresolvedCount":len(tasks),
            "allUnresolvedRemainOpen":True,"upstreamReceiptPresent":True,
            "legalReviewApproved":False,"releaseApproved":False,"tasks":tasks,
            "specialLicenseReviewCount":len(flags),"specialLicenseReviews":flags}

def enforce_known_risk_set(report, policy):
    if (not isinstance(policy,dict) or policy.get("schema")!="lu.ide.p06.known-license-risk.v1"
            or policy.get("releaseApproved") is not False
            or policy.get("allRequireIndependentReview") is not True
            or not isinstance(policy.get("expectedRiskPackages"),list)):
        raise TriageDenied("invalid_license_risk_policy")
    expected={(row["manager"],row["name"],row["version"],row["declaredLicense"])
              for row in policy["expectedRiskPackages"]}
    observed={(row["manager"],row["name"],row["version"],row["declaredLicense"])
              for row in report["specialLicenseReviews"]}
    if expected!=observed or len(expected)!=len(policy["expectedRiskPackages"]):
        raise TriageDenied("new_or_missing_copyleft_or_multi_license_review")
    return True

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--inventory",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--known-risk-policy",type=Path,required=True)
    a=p.parse_args()
    try:
        if a.output.exists() or a.output.is_symlink():raise TriageDenied("triage_output_must_be_new")
        if a.inventory.is_symlink() or a.inventory.stat().st_size>5_000_000:raise TriageDenied("unsafe_inventory")
        raw=a.inventory.read_bytes()
        result=triage(json.loads(raw),raw)
        enforce_known_risk_set(result,json.loads(a.known_risk_policy.read_text(encoding="utf-8")))
        a.output.parent.mkdir(parents=True,exist_ok=True)
        a.output.write_text(json.dumps(result,indent=2,sort_keys=True)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps({"unresolvedCount":result["unresolvedCount"],
                       "legalReviewApproved":False,"specialLicenseReviewCount":result["specialLicenseReviewCount"],"categories":sorted(set(x["classification"] for x in result["tasks"]))}))
    return 0
if __name__=="__main__":
    raise SystemExit(main())
