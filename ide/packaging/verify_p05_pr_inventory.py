#!/usr/bin/env python3
"""P05 local fail-closed verification of recorded GitHub PR/CI consolidation.

This verifies exact checkout bytes and the *claims* recorded in an audited
snapshot. It never substitutes for independently reviewing original PR
semantics or passing native Windows/Linux release gates.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

SHA40=re.compile(r"^[a-fA-F0-9]{40}$")
SOURCE_SCOPE=("ide/",".github/workflows/ide-")
EXPECTED_EXCEPTIONS={43,45,46,76}
GROUPS={"parallel_unported","historical_stack","phase_chain","deferred_do_not_merge"}
class Denied(ValueError):
    pass

def pin(value):
    return isinstance(value,str) and SHA40.fullmatch(value) is not None

def check_snapshot(prs:dict,workflows:dict):
    if prs.get("schema")!="lu.ide.p05.pr-source-audit.v1":
        raise Denied("invalid_pr_inventory_schema")
    if workflows.get("schema")!="lu.ide.p05.workflow-coverage.v1":
        raise Denied("invalid_ci_inventory_schema")
    base=prs.get("sourceCandidateSha")
    if not pin(base) or workflows.get("sourceCandidateSha")!=base:
        raise Denied("cross_inventory_source_sha_drift")
    if prs.get("releaseApproved") is not False or workflows.get("releaseApproved") is not False:
        raise Denied("unexpected_release_promotion")
    entries=prs.get("prEntries")
    if not isinstance(entries,list) or len(entries)!=30 or prs.get("actualOpenIdePrCount")!=30:
        raise Denied("missing_open_ide_prs_or_not_reconciled")
    expected={43,45,46,47,48,49,50,51,52,53,54,55,56,58,60,62,63,64,65,68,69,70,73,74,75,76,78,79,80,84}
    seen=set()
    omitted={}
    for row in entries:
        if not isinstance(row,dict) or type(row.get("number")) is not int:
            raise Denied("invalid_pr_item")
        n=row["number"]
        if n not in expected or n in seen:
            raise Denied("unknown_or_duplicate_pr")
        seen.add(n)
        if (not pin(row.get("headSha"))
                or not isinstance(row.get("baseBranchAtAudit"),str)
                or not row["baseBranchAtAudit"]
                or type(row.get("ideRelevantFileCount")) is not int
                or type(row.get("differentExistingBlobCount")) is not int
                or row["ideRelevantFileCount"] < 0
                or row["differentExistingBlobCount"] < 0
                or row["differentExistingBlobCount"]>row["ideRelevantFileCount"]):
            raise Denied("unknown_pr_source_or_comparison")
        if (row.get("disposition")!="retain_open"
                or row.get("closureAuthorized") is not False
                or row.get("protectedNativeAcceptance")!="pending"
                or row.get("group") not in GROUPS
                or not isinstance(row.get("reason"),str) or not row["reason"]):
            raise Denied("premature_pr_close_or_native_acceptance_promotion")
        if (n==76)!=(row["group"]=="deferred_do_not_merge"):
            raise Denied("deferred_wo10_must_remain_excluded")
        if (n in {43,45,46})!=(row["group"]=="parallel_unported"):
            raise Denied("parallel_b2_b3_source_waiver_not_authorized")
        items=row.get("missingCandidatePaths")
        if not isinstance(items,list) or len(items)!=len(set(items)):
            raise Denied("duplicate_or_invalid_missing_files")
        if bool(items)!=(n in EXPECTED_EXCEPTIONS):
            raise Denied("unported_or_deferred_source_missing_from_report")
        for item in items:
            if (not isinstance(item,str) or not item.startswith(SOURCE_SCOPE)
                    or item.startswith("/") or "\\" in item
                    or any(x in {"",".",".."} for x in item.split("/"))):
                raise Denied("unsafe_or_unrelated_source_path")
        omitted[n]=items
    if seen!=expected or prs.get("deferredWo10Pr")!=76:
        raise Denied("incomplete_ide_pr_or_deferred_wo10_inventory")
    pins=prs.get("uniqueUnportedBlobPins")
    if not isinstance(pins,dict) or len(pins)!=9:
        raise Denied("unported_b2_b3_blob_provenance_missing")
    if set(pins)!=(set(omitted[43])|set(omitted[45])|set(omitted[46])):
        raise Denied("unknown_or_missing_unported_source_pins")
    if not all(pin(sha) for sha in pins.values()):
        raise Denied("unverified_unported_git_blob")
    if (prs.get("closurePolicy")!="no_close_or_retarget_without_exact_source_coverage_and_native_evidence"):
        raise Denied("closure_policy_weakened")
    if workflows.get("nativeReleaseSecurityClosure")!="pending":
        raise Denied("native_security_workflow_replacement_not_accepted")
    rows=workflows.get("requiredWorkflows")
    if not isinstance(rows,list) or len(rows)!=38 or workflows.get("observedWorkflowCount")!=38:
        raise Denied("workflow_gate_coverage_incomplete")
    seen_paths=set()
    required={entry["path"] for entry in rows if isinstance(entry,dict) and "path" in entry}
    for item in rows:
        if not isinstance(item,dict):
            raise Denied("invalid_workflow_inventory_item")
        path=item.get("path")
        if (not isinstance(path,str) or not path.startswith(".github/workflows/ide-")
                or not path.endswith(".yml") or path in seen_paths or not pin(item.get("blobSha"))
                or item.get("disposition")!="retain_until_verified_replacement"
                or item.get("replacementAccepted") is not False
                or item.get("prTriggerPruningAuthorized") is not False
                or type(item.get("criticalNativeOrReleaseEvidence")) is not bool):
            raise Denied("missing_or_prematurely_disabled_ci_gate")
        seen_paths.add(path)
    for key in ("ide-p03-native-security-checks.yml","ide-wo6-native-os-acceptance.yml",
                "ide-e2-native-lsp.yml","ide-wo7-signed-release-preflight.yml",
                "ide-wo8-installed-windows.yml","ide-wo8-installed-linux.yml",
                "ide-wo9-installed-native.yml","ide-p04-source-acceptance.yml"):
        if ".github/workflows/"+key not in required:
            raise Denied("required_native_or_license_ci_removed")
    return {"prCount":30,"workflowCount":38,"uniqueParallelPRs":3,
            "deferredWO10":True,"approvedForClosure":0,"releaseAccepted":False}

def git_blob_sha(root:Path,relative:str):
    proc=subprocess.run(["git","ls-tree","HEAD","--",relative],
                        cwd=root,text=True,capture_output=True,timeout=12,check=False)
    if proc.returncode or not proc.stdout.strip():
        raise Denied("required_ci_file_missing_from_git_tree")
    line=proc.stdout.strip()
    if "\t" not in line:
        raise Denied("unreadable_trusted_git_tree_entry")
    pre,path=line.split("\t",1)
    args=pre.split(" ")
    if len(args)!=3 or args[1]!="blob" or path!=relative or not pin(args[2]):
        raise Denied("unexpected_workflow_git_blob")
    return args[2]

def check_checkout(root:Path,prs,workflows):
    root=root.resolve(strict=True)
    if not (root/".git").exists():
        raise Denied("real_git_checkout_required")
    for row in workflows["requiredWorkflows"]:
        path=row["path"]
        if not (root/path).is_file() or (root/path).is_symlink():
            raise Denied("required_workflow_missing_or_linked")
        if git_blob_sha(root,path)!=row["blobSha"]:
            raise Denied("reviewed_native_ci_workflow_changed:"+path)
    for row in prs["prEntries"]:
        for rel in row["missingCandidatePaths"]:
            if (root/rel).exists() or (root/rel).is_symlink():
                raise Denied("unreviewed_parallel_or_deferred_source_integrated:"+rel)
    # In particular, no bridge implementation may be accidentally enabled.
    for rel in ("ide/bridge-contract","ide/proto/lu/ide/v1/bridge.proto",
                "ide/host/src/bridge_wire.rs"):
        if (root/rel).exists():
            raise Denied("deferred_wo10_source_imported_without_authorization")
    return True

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",type=Path,required=True)
    p.add_argument("--pr-inventory",type=Path,required=True)
    p.add_argument("--workflow-inventory",type=Path,required=True)
    a=p.parse_args()
    try:
        prs=json.loads(a.pr_inventory.read_text(encoding="utf-8"))
        workflows=json.loads(a.workflow_inventory.read_text(encoding="utf-8"))
        result=check_snapshot(prs,workflows)
        check_checkout(a.repo_root,prs,workflows)
    except (OSError,ValueError,TypeError,KeyError,subprocess.TimeoutExpired) as exc:
        print("DENIED: "+(str(exc) if isinstance(exc,Denied) else "invalid_p05_source_snapshot"),file=sys.stderr)
        return 2
    print(json.dumps(result,sort_keys=True))
    return 0

if __name__=="__main__":
    raise SystemExit(main())
