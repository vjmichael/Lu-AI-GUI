#!/usr/bin/env python3
"""Inventory-specific unresolved NOTICE review queue: no automatic legal clearance."""
import argparse
import hashlib
import json
from pathlib import Path
from collect_verification_licenses import OBJC2_UPSTREAM_POLICY,EXTERNAL_NATIVE_SDK_SOURCE_RULES,WEBVIEW_EXACT_CRATES
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

# Embedded prebuilt protoc executables require a distinct upstream
# redistribution/binary provenance review, separate from the Rust wrapper MIT.
PROTOC_EMBEDDED_BINARY_FAMILY = ["protoc-bin-vendored","protoc-bin-vendored-linux-aarch_64","protoc-bin-vendored-linux-ppcle_64","protoc-bin-vendored-linux-s390_64","protoc-bin-vendored-linux-x86_32","protoc-bin-vendored-linux-x86_64","protoc-bin-vendored-macos-aarch_64","protoc-bin-vendored-macos-x86_64","protoc-bin-vendored-win32"]

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
    embedded=[]
    for name in PROTOC_EMBEDDED_BINARY_FAMILY:
        row=pkgs.get(("cargo",name,"3.3.0"))
        if row is not None:
            embedded.append({
                "manager":"cargo","name":name,"version":"3.3.0",
                "rustWrapperDeclaredLicense":row.get("declaredLicense"),
                "binaryOrigin":"Google-built protoc executable",
                "reviewRequired":"Verify embedded upstream protoc binary version, Google copyright and license, redistribution obligations, actual shipped binary closure, and target architecture.",
                "targetSelectionIsNotShipmentProof":True,
                "disposition":"open","legalReviewApproved":False,
                "reviewedBy":None})
    apple=[]
    for rule in OBJC2_UPSTREAM_POLICY:
        row=pkgs.get(("cargo",rule["name"],rule["version"]))
        if row is not None:
            apple.append({
                "manager":"cargo","name":rule["name"],"version":rule["version"],
                "upstreamTag":rule["tag"],"declaredLicense":row.get("declaredLicense"),
                "upstreamPolicyNotCompleteTerms":True,
                "reviewRequired":"Apple Xcode SDK-derived work terms, full linked MIT/Apache/Zlib text and OS binary shipment require independent approval.",
                "disposition":"open","legalReviewApproved":False,"reviewedBy":None})
    external=[]
    for rule in EXTERNAL_NATIVE_SDK_SOURCE_RULES:
        row=pkgs.get(("cargo",rule["name"],rule["version"]))
        if row is not None:
            external.append({"manager":"cargo","name":rule["name"],"version":rule["version"],
                "upstreamTag":rule["tag"],"declaredLicense":row.get("declaredLicense"),
                "externalNativeDependency":rule["external"],"disposition":"open",
                "reviewRequired":"Verify actual distributed native SDK/system libraries and binaries, license obligations, compiled target closure independently of Rust source notices.",
                "legalReviewApproved":False,"reviewedBy":None})
    webview=[]
    for name,version in WEBVIEW_EXACT_CRATES:
        row=pkgs.get(("cargo",name,version))
        if row is not None:
            webview.append({"manager":"cargo","name":name,"version":version,
                "rustWrapperDeclaredLicense":row.get("declaredLicense"),
                "separateMicrosoftRuntimeLicensingRequired":True,
                "reviewRequired":"Check actual Microsoft WebView2 Runtime distribution, prerequisites, royalty/licensing, installer payload versus OS installation, and precise package closure independently of the MIT Rust bindings.",
                "disposition":"open","legalReviewApproved":False,"reviewedBy":None})
    return {"schema":"lu.ide.p06.license-triage.v1","sourceSha":data["sourceSha"],
            "inventorySha256":hashlib.sha256(source_bytes).hexdigest(),
            "totalResolvedPackages":len(pkgs),"unresolvedCount":len(tasks),
            "allUnresolvedRemainOpen":True,"upstreamReceiptPresent":True,
            "legalReviewApproved":False,"releaseApproved":False,"tasks":tasks,
            "specialLicenseReviewCount":len(flags),"specialLicenseReviews":flags,
            "embeddedCompilerReviewCount":len(embedded),
            "embeddedCompilerReviews":embedded,
            "embeddedCompilerLegalReviewApproved":False,
            "appleSdkReviewCount":len(apple),"appleSdkReviews":apple,
            "appleSdkLegalReviewApproved":False,
            "externalNativeReviewCount":len(external),"externalNativeReviews":external,
            "externalNativeLegalReviewApproved":False,
            "webviewRuntimeReviewCount":len(webview),
            "webviewRuntimeReviews":webview,
            "webviewRuntimeLegalReviewApproved":False}

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

def enforce_embedded_binary_review(report,policy):
    required=policy.get("expectedEmbeddedCompilerPackages")
    if (not isinstance(required,list) or not all(isinstance(x,dict) for x in required)):
        raise TriageDenied("missing_embedded_compiler_review_policy")
    expected={(item.get("manager"),item.get("name"),item.get("version")) for item in required}
    observed={(item["manager"],item["name"],item["version"]) for item in report["embeddedCompilerReviews"]}
    known={("cargo",name,"3.3.0") for name in PROTOC_EMBEDDED_BINARY_FAMILY}
    if expected!=known or observed!=expected or len(expected)!=len(required):
        raise TriageDenied("unreviewed_embedded_protoc_binary_family")
    if any(x.get("disposition")!="open" or x.get("legalReviewApproved") is not False
           for x in report["embeddedCompilerReviews"]):
        raise TriageDenied("invalid_embedded_binary_clearance")
    return True

def enforce_apple_sdk_review(report,policy):
    expected={(r["name"],r["version"],r["tag"],r["declared"]) for r in OBJC2_UPSTREAM_POLICY}
    provided=policy.get("expectedAppleSdkPackages")
    if not isinstance(provided,list) or len(provided)!=len(expected):
        raise TriageDenied("missing_apple_sdk_review_policy")
    records={(x.get("name"),x.get("version"),x.get("tag"),x.get("declared"))
             for x in provided}
    observed={(x["name"],x["version"],x["upstreamTag"],x["declaredLicense"])
              for x in report["appleSdkReviews"]}
    if records!=expected or observed!=expected:
        raise TriageDenied("apple_sdk_review_obligation_drift")
    if any(x.get("disposition")!="open" or x.get("legalReviewApproved") is not False
           for x in report["appleSdkReviews"]):
        raise TriageDenied("false_apple_sdk_legal_approval")
    return True

def enforce_external_native_review(report,policy):
    expected={(r["name"],r["version"],r["tag"],r["declared"]) for r in EXTERNAL_NATIVE_SDK_SOURCE_RULES}
    supplied=policy.get("expectedExternalNativePackages")
    if not isinstance(supplied,list) or len(supplied)!=len(expected):
        raise TriageDenied("missing_external_native_policy")
    ground={(r.get("name"),r.get("version"),r.get("tag"),r.get("declared")) for r in supplied}
    actual={(r["name"],r["version"],r["upstreamTag"],r["declaredLicense"]) for r in report["externalNativeReviews"]}
    if ground!=expected or actual!=expected or any(r.get("disposition")!="open" or r.get("legalReviewApproved") is not False for r in report["externalNativeReviews"]):
        raise TriageDenied("external_native_redistribution_review_mismatch")
    return True

def enforce_webview_runtime_review(report,policy):
    expected=set(WEBVIEW_EXACT_CRATES)
    configured=policy.get("expectedWebviewBindingPackages")
    if not isinstance(configured,list) or len(configured)!=len(expected):
        raise TriageDenied("missing_webview_runtime_policy")
    given={(x.get("name"),x.get("version")) for x in configured}
    actual={(x["name"],x["version"]) for x in report["webviewRuntimeReviews"]}
    if (given!=expected or actual!=expected
            or any(x.get("disposition")!="open" or x.get("legalReviewApproved") is not False
                   for x in report["webviewRuntimeReviews"])):
        raise TriageDenied("webview_runtime_rights_unreviewed_or_misclassified")
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
        policy=json.loads(a.known_risk_policy.read_text(encoding="utf-8"))
        enforce_known_risk_set(result,policy)
        enforce_embedded_binary_review(result,policy)
        enforce_apple_sdk_review(result,policy)
        enforce_external_native_review(result,policy)
        enforce_webview_runtime_review(result,policy)
        a.output.parent.mkdir(parents=True,exist_ok=True)
        a.output.write_text(json.dumps(result,indent=2,sort_keys=True)+"\n",encoding="utf-8")
    except (OSError,ValueError,TypeError,KeyError) as e:
        print("DENIED: "+str(e),file=sys.stderr)
        return 2
    print(json.dumps({"unresolvedCount":result["unresolvedCount"],
                       "legalReviewApproved":False,"specialLicenseReviewCount":result["specialLicenseReviewCount"],"embeddedCompilerReviewCount":result["embeddedCompilerReviewCount"],"appleSdkReviewCount":result["appleSdkReviewCount"],"externalNativeReviewCount":result["externalNativeReviewCount"],"webviewRuntimeReviewCount":result["webviewRuntimeReviewCount"],"categories":sorted(set(x["classification"] for x in result["tasks"]))}))
    return 0
if __name__=="__main__":
    raise SystemExit(main())
