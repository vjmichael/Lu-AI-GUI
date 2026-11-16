#!/usr/bin/env python3
"""Owner-authorized P05 current-stack audit; never a native/release pass.

The original 38-workflow P05 verifier is executed unchanged on the exact
last historically verified tree in CI. This successor refuses any unapproved
current-tree difference, altered historical baseline, missing negative check,
or missing worker feature-only tests. It does NOT assert native equivalence.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ORIGINAL = "8c61d570c04a5384772705d123f860b0e3484073"
P05_MANIFEST_BLOB = "5299b1ca6981f341d2855f549e567b8c2747dc13"
P05_VERIFIER_BLOB = "92299fb443f58e73d0600a1e2a1b84be5fcef5e7"
EXPECTED = {
    ".github/workflows/ide-d7-windows-worker.yml": ("518635e58bbac5b249bfeadba7e60c90b9d0010c", "6832a33bdab3539d85805ccd43a9aa479758d08d"),
    ".github/workflows/ide-d7b-native-acceptance.yml": ("f70955319c621cbd08cd2a7a6f7eac77caffc3a1", "c247635e41a4c325a76cc5223239f890898c92c1"),
    ".github/workflows/ide-d7d-protected-service.yml": ("ce56f987cb056c01ef7be58318d70c9e136aafad", "3450fba59185c88dc67ccc87627570db3a91b1b7"),
    ".github/workflows/ide-sandbox-windows.yml": ("4344a3563b9151e874443c5000639114342e66b5", "742a255acbbae975514ea468d5932c07021d7643"),
    ".github/workflows/ide-supervisor-core.yml": ("e0d26f80713cdf498c3bfdf82645498dddf34e16", "11203a4e5f8cfeecee9effa00c88c981492be527"),
    ".github/workflows/ide-wo6-native-os-acceptance.yml": ("d7627899b3a6cafc7b383f00abbb2b9e7107fe2d", "cea665eec68062b4071f57be206d56676a885ed3"),
    ".github/workflows/ide-wo8-installed-windows.yml": ("8fadc3f1829e62123b9da6a5e6bbd49a3f3ec36f", "95138087b419a4b1f9baa052e717c34fc069ae44"),
    ".github/workflows/ide-wo8-signed-protected-e2e.yml": ("08e2ab350b9e0c9e88958a40a3eeca59a3a66df5", "3cbaf9efc3836bfbe3a978cdd5511daac04f2380"),
}
USER_DECISION = (237, 6921053701)

class Denied(ValueError):
    pass


def original_blob_sha(data: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def compare_38_unchanged_and_replacements(baseline, current):
    if len(baseline) != 38 or set(baseline) != set(current):
        raise Denied("p05_expected_exact_38_original_workflows")
    changed = {p:(v,current[p]) for p,v in baseline.items() if current[p]!=v}
    if changed != EXPECTED:
        raise Denied("p05_unreviewed_or_unretained_native_workflow_change")
    return {"historicalExactWorkflows":38,"unchangedCurrentWorkflows":30,
            "ownerAuthorizedCurrentReplacements":8,"nativeAccepted":False,
            "releaseApproved":False}


def verify(root: Path):
    root = root.resolve(strict=True)
    def git(*args):
        try:
            return subprocess.run(
                ["git","-C",str(root),*args],check=True,timeout=20,
                capture_output=True,text=True,
            ).stdout.strip()
        except (subprocess.CalledProcessError, OSError, subprocess.TimeoutExpired) as exc:
            raise Denied("p05_required_unchanged_git_object_unavailable") from exc
    if git("rev-parse","--is-inside-work-tree")!="true":
        raise Denied("p05_real_git_worktree_required")
    if git("rev-parse",ORIGINAL+":ide/packaging/p05-workflow-coverage.json")!=P05_MANIFEST_BLOB:
        raise Denied("p05_original_38_manifest_blob_mismatch")
    if git("rev-parse",ORIGINAL+":ide/packaging/verify_p05_pr_inventory.py")!=P05_VERIFIER_BLOB:
        raise Denied("p05_original_verifier_blob_mismatch")
    if git("rev-parse","HEAD:ide/packaging/p05-workflow-coverage.json")!=P05_MANIFEST_BLOB:
        raise Denied("p05_original_manifest_overwritten")
    if git("rev-parse","HEAD:ide/packaging/verify_p05_pr_inventory.py")!=P05_VERIFIER_BLOB:
        raise Denied("p05_original_verifier_modified")
    manifest_path=root/"ide/packaging/p05-workflow-coverage.json"
    if manifest_path.is_symlink() or original_blob_sha(manifest_path.read_bytes())!=P05_MANIFEST_BLOB:
        raise Denied("p05_manifest_checkout_source_drift")
    manifest=json.loads(manifest_path.read_text(encoding="utf-8"))
    ledger=json.loads((root/"ide/audits/p05_owner_authorized_successor.json").read_text(encoding="utf-8"))
    if ledger.get("schema")!="lu.ide.merge02.p05.owner-authorized-current-stack.v1":
        raise Denied("p05_owner_successor_schema_changed")
    if (ledger.get("originalManifestBlob")!=P05_MANIFEST_BLOB or
        ledger.get("originalVerifierBlob")!=P05_VERIFIER_BLOB or
        ledger.get("historicalManifestExactTree")!=ORIGINAL or
        ledger.get("originalWorkflowCount")!=38):
        raise Denied("p05_owner_successor_historical_baseline_changed")
    decision=ledger.get("ownerDecision",{})
    if ((decision.get("issue"),decision.get("commentId"))!=USER_DECISION or
        decision.get("authorizedSevenChanges") is not True or
        decision.get("noSecondReviewerRequired") is not True):
        raise Denied("p05_missing_explicit_owner_authorization")
    for key in ("nativeWindowsAccepted","nativeLinuxAccepted","llvm18Accepted",
                "installedGuiAccepted","releaseApproved","mergeApproved",
                "historicalManifestRewritten","originalNegativeTestsDropped"):
        if ledger.get(key) is not False:
            raise Denied("p05_forged_native_or_release_acceptance:"+key)
    rows=manifest.get("requiredWorkflows")
    if not isinstance(rows,list) or len(rows)!=38:
        raise Denied("p05_original_required_workflows_missing")
    original={x["path"]:x["blobSha"] for x in rows}
    if len(original)!=38 or any(x.get("replacementAccepted") is not False for x in rows):
        raise Denied("p05_original_fingerprint_approval_falsified")
    current={}
    for path, original_sha in original.items():
        local=root/path
        if not local.is_file() or local.is_symlink():
            raise Denied("p05_workflow_missing_or_linked:"+path)
        if git("rev-parse",ORIGINAL+":"+path)!=original_sha:
            raise Denied("p05_historical_38_workflow_git_sha_drift:"+path)
        recorded=git("rev-parse","HEAD:"+path)
        if git("hash-object","--",path)!=recorded:
            raise Denied("p05_working_workflow_not_exact_head:"+path)
        current[path]=recorded
        historical=git("show",ORIGINAL+":"+path)
        recent=local.read_text(encoding="utf-8")
        oldsteps=re.findall(r"(?m)^\s*- name:\s*(.+)$",historical)
        recentsteps=re.findall(r"(?m)^\s*- name:\s*(.+)$",recent)
        if any(step not in recentsteps for step in oldsteps):
            raise Denied("p05_original_named_security_step_removed:"+path)
    result=compare_38_unchanged_and_replacements(original,current)
    listed={x.get("path"):(x.get("original"),x.get("replacement"))
            for x in ledger.get("workflows",[])}
    if len(ledger.get("workflows",[]))!=8 or listed!=EXPECTED or any(
        x.get("ownerAuthorized") is not True for x in ledger["workflows"]
    ):
        raise Denied("p05_owner_successor_changes_not_exactly_authorized")
    d7d=(root/".github/workflows/ide-d7d-protected-service.yml").read_text(encoding="utf-8")
    wo6=(root/".github/workflows/ide-wo6-native-os-acceptance.yml").read_text(encoding="utf-8")
    original_d7d=[
        "cargo check --manifest-path ide/supervisor/Cargo.toml --features protected-recovery-service --all-targets",
        "cargo test --manifest-path ide/supervisor/Cargo.toml --features protected-recovery-service --lib",
    ]
    original_wo6=[
        "cargo build --manifest-path ide/supervisor/Cargo.toml --features 'dap-transport,protected-recovery-service' --bin windows_recovery_service --bin windows_null_service_bootstrap --example wo6_native_dap_acceptance",
        "cargo build --manifest-path ide/supervisor/Cargo.toml --features linux-worker --example wo6_linux_native_build_acceptance",
    ]
    if not all(s in d7d for s in original_d7d):
        raise Denied("p05_d7d_original_feature_only_commands_removed")
    if not all(s in wo6 for s in original_wo6):
        raise Denied("p05_wo6_original_feature_only_commands_removed")
    sandbox=(root/".github/workflows/ide-sandbox-windows.yml").read_text(encoding="utf-8")
    if not ("historical standalone desktop worker-execution boundary" in sandbox
            and "CURRENT_DEFAULT_DISABLED_WORKER" in sandbox):
        raise Denied("p05_sandbox_original_and_current_default_negative_missing")
    for name in ("ide-d7-windows-worker.yml","ide-supervisor-core.yml"):
        current_text=(root/".github/workflows"/name).read_text(encoding="utf-8")
        if "workflow_dispatch:" not in current_text or "pull_request:" not in current_text:
            raise Denied("p05_current_d7_or_supervisor_pr_manual_gate_removed")
        if re.search(r"(?m)^  push:",current_text):
            raise Denied("p05_unreviewed_push_microjobs_reintroduced")
    return result


def main():
    p=argparse.ArgumentParser()
    p.add_argument("--repo-root",required=True,type=Path)
    args=p.parse_args()
    try:
        result=verify(args.repo_root)
    except (Denied, OSError, ValueError, TypeError, KeyError) as err:
        print("DENIED:"+str(err))
        return 2
    print(json.dumps(result,sort_keys=True))
    print("P05_OWNER_APPROVED_SOURCE_SUCCESSOR=PASS_NATIVE_AND_RELEASE_STILL_PENDING")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
