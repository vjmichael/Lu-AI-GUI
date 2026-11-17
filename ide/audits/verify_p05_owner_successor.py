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
    ".github/workflows/ide-wo6-native-os-acceptance.yml": ("d7627899b3a6cafc7b383f00abbb2b9e7107fe2d", "ac7992aac3a9e17bda4d44d450bcd199d5a5d874"),
    ".github/workflows/ide-wo8-installed-windows.yml": ("8fadc3f1829e62123b9da6a5e6bbd49a3f3ec36f", "95138087b419a4b1f9baa052e717c34fc069ae44"),
    ".github/workflows/ide-wo8-signed-protected-e2e.yml": ("08e2ab350b9e0c9e88958a40a3eeca59a3a66df5", "3cbaf9efc3836bfbe3a978cdd5511daac04f2380"),
}
USER_DECISION = (237, 6921053701)

class Denied(ValueError):
    pass


# These five changes are NOT retroactive owner approvals. They are individually
# reviewed current-source dispositions, independently pinned to both Git blobs.
# The historical P05 38/8 contract and its native/negative gates remain intact.
SCOPED_SOURCE_ONLY = {
    ".github/workflows/ide-e2-native-lsp.yml": (
        "8ca27a7f666460e021b45c2e5469d532d0a82071",
        "b0e8017157e9727f7c5a3fe474f7510db08b97fe",
        "measured_clangd_bundle_binding", 13),
    ".github/workflows/ide-e2-supervised-lsp.yml": (
        "c2be7149dc3424d7c296aae2f98a6fd37b99c77b",
        "a29c605710c16b9fe8678f4072f79f12a4797d98",
        "exact_sha_checkout_and_real_llvm18_oracle", 18),
    ".github/workflows/ide-e2-unsigned-installed-smoke.yml": (
        "6d8761823e67575cb979b15570f8fb44d571d4d5",
        "29b34141b0557ddc4a341ebf5ce5f64f16c69ab1",
        "reviewed_p06_lock_successor", 6),
    ".github/workflows/ide-wo8-installed-linux.yml": (
        "cbb33c566a492ed176d7af8ef381019c5753605e",
        "a40f2ba4867acd24e94ccc73a86cd6e4580735b3",
        "reviewed_p06_lock_successor", 9),
    ".github/workflows/ide-wo9-installed-native.yml": (
        "60c1bf3d20fb08dcdcfd183a8ffec76599fa6c52",
        "6c6e8f89a9a60d8e9703dc54af91e6bc690024b8",
        "reviewed_p06_lock_successor", 8),
}
LOCK_INVOKES = {
    ".github/workflows/ide-e2-unsigned-installed-smoke.yml": 2,
    ".github/workflows/ide-wo8-installed-linux.yml": 2,
    ".github/workflows/ide-wo9-installed-native.yml": 4,
}

def verify_source_only_patch(path, original, current):
    """Require an exact additive, test-preserving patch, not step-count alone."""
    old_names = re.findall(r"(?m)^\s*- name:\s*(.+)$", original)
    new_names = re.findall(r"(?m)^\s*- name:\s*(.+)$", current)
    if old_names != new_names[:len(old_names)] and path != ".github/workflows/ide-e2-supervised-lsp.yml":
        raise Denied("p05_source_only_original_named_step_removed:" + path)
    mode = SCOPED_SOURCE_ONLY[path][2]
    if mode == "measured_clangd_bundle_binding":
        added = (
            "          # Only the just-audited, self-measured native CI bundle is admitted\n"
            "          # by the later fixed AppContainer acceptance host. Never use PATH.\n"
            "          Add-Content -LiteralPath $env:GITHUB_ENV -Value ('LU_IDE_E2_PINNED_LSP_BUNDLE='+$bundle)\n"
        )
        if current.count(added) != 1:
            raise Denied("p05_clangd_measured_bundle_binding_changed")
        normalized = current.replace(added, "", 1)
    elif mode == "exact_sha_checkout_and_real_llvm18_oracle":
        checkout = (
            "        with:\n"
            "          ref: ${{ github.event.pull_request.head.sha || github.sha }}\n"
            "          persist-credentials: false\n"
            "          fetch-depth: 0\n"
        )
        oracle = (
            "      - name: Provide authentic LLVM18 source-line oracle for Linux packaging regression\n"
            "        if: runner.os == 'Linux'\n"
            "        shell: bash\n"
            "        run: |\n"
            "          set -euo pipefail\n"
            "          sudo apt-get update -qq\n"
            "          sudo apt-get install -y --no-install-recommends llvm-18 clang\n"
            "          test -x /usr/lib/llvm-18/bin/llvm-symbolizer\n"
            "          echo '/usr/lib/llvm-18/bin' >> \"$GITHUB_PATH\"\n"
        )
        if current.count(checkout) != 3 or current.count(oracle) != 1:
            raise Denied("p05_source_checkout_or_real_llvm_oracle_changed")
        normalized = current.replace(checkout, "", 3).replace(oracle, "", 1)
        if new_names != old_names[:5] + [
            "Provide authentic LLVM18 source-line oracle for Linux packaging regression"
        ] + old_names[5:]:
            raise Denied("p05_original_e2_named_steps_or_order_changed")
    else:
        suffix = " --ide-merge02-successor"
        if current.count(suffix) != LOCK_INVOKES[path]:
            raise Denied("p05_reviewed_lock_argument_count_changed")
        for line in current.splitlines():
            if suffix in line and (
                "verify_p06_candidate_lock.py" not in line
                or not line.endswith(suffix)
            ):
                raise Denied("p05_reviewed_lock_argument_moved")
        normalized = current.replace(suffix, "")
    if normalized.rstrip("\r\n") != original.rstrip("\r\n"):
        raise Denied("p05_source_only_patch_expands_original_test_or_trigger")
    return True

def compare_38_with_separate_scoped_source_dispositions(baseline, current, review):
    if len(baseline) != 38 or set(baseline) != set(current):
        raise Denied("p05_expected_exact_38_original_workflows")
    if not isinstance(review, dict) or (
        review.get("schema") != "lu.ide.merge02.p05.additional-current-source-dispositions.v1"
        or review.get("historicalTree") != ORIGINAL
        or review.get("immutableHistoricalWorkflowCount") != 38
        or review.get("historicalOwnerAuthorizedCount") != 8
        or review.get("scopedCurrentSourceOnlyCount") != 5
        or review.get("scope") != "exact current source correspondence only; not retroactive owner authorization"
    ):
        raise Denied("p05_source_disposition_historical_scope_changed")
    for field in (
        "historicalP05Rewritten", "priorOwnerDecisionModified",
        "nativeWindowsAccepted", "nativeLinuxAccepted", "llvm18Accepted",
        "installedGuiAccepted", "releaseApproved", "mergeApproved",
    ):
        if review.get(field) is not False:
            raise Denied("p05_source_disposition_forged_acceptance:" + field)
    for field in (
        "originalTriggersRetained", "originalJobsAndNamedStepsRetained",
        "originalNativeNegativeTestsRetained",
    ):
        if review.get(field) is not True:
            raise Denied("p05_source_disposition_security_assertions_removed:" + field)
    rows = review.get("workflows")
    if not isinstance(rows, list) or len(rows) != 5:
        raise Denied("p05_source_disposition_exact_five_required")
    seen = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "path", "original", "current", "mode", "originalNamedStepCount",
            "disposition", "historicalOwnerAuthorization",
            "originalTriggerAndTestsRetained", "nativeAccepted", "reason"
        }:
            raise Denied("p05_source_disposition_row_shape_changed")
        path = row["path"]
        if path not in SCOPED_SOURCE_ONLY or path in seen:
            raise Denied("p05_source_disposition_unknown_or_duplicate_workflow")
        seen.add(path)
        expected = SCOPED_SOURCE_ONLY[path]
        if (tuple(row[k] for k in ("original","current","mode","originalNamedStepCount")) != expected
            or row["disposition"] != "source_reviewed_not_native_accepted"
            or row["historicalOwnerAuthorization"] is not False
            or row["originalTriggerAndTestsRetained"] is not True
            or row["nativeAccepted"] is not False
            or not isinstance(row["reason"], str)
            or not 16 <= len(row["reason"]) <= 220):
            raise Denied("p05_source_disposition_provenance_or_approval_changed")
    if seen != set(SCOPED_SOURCE_ONLY):
        raise Denied("p05_source_disposition_missing_workflow")
    changed = {p:(v,current[p]) for p,v in baseline.items() if current[p] != v}
    expected_changes = dict(EXPECTED)
    expected_changes.update({p:(data[0],data[1]) for p,data in SCOPED_SOURCE_ONLY.items()})
    if changed != expected_changes:
        raise Denied("p05_unreviewed_or_unretained_native_workflow_change")
    return {
        "historicalExactWorkflows":38,
        "unchangedCurrentWorkflows":25,
        "ownerAuthorizedHistoricalReplacements":8,
        "separatelyReconciledSourceOnlyDifferences":5,
        "historicalApprovalRewritten":False,
        "nativeAccepted":False,
        "releaseApproved":False,
    }


def original_blob_sha(data: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


# This is a separately reviewed current source repair of ONE previously
# authorized successor, not a change to the owner's original eight approvals.
# Exact byte normalization proves that only the reviewed source-pinned
# transfer and Linux breakpoint test additions differ from that successor.
WO6_PATH = ".github/workflows/ide-wo6-native-os-acceptance.yml"
WO6_PRIOR_OWNER_BLOB = "ac7992aac3a9e17bda4d44d450bcd199d5a5d874"
WO6_CURRENT_SOURCE_BLOB = "0c617c002bc86ccf702319d4515ebbb86bfe9262"
WO6_SOURCE_ONLY_DELTAS = (
    (b"      - 'ide/llvm18_owner/**'\n      - '.github/workflows/ide-dev04-5d5-owner-event-native.yml'\n", b""),
    (b"    # Reuse the exact-head pinned LLVM build rather than rebuilding it or\n    # pretending the runner's unmodified LLVM is AppContainer-compatible.\n    # Only this consumer job may read the source workflow's artifacts.\n    permissions:\n      contents: read\n      actions: read\n    timeout-minutes: 180\n", b"    timeout-minutes: 60\n"),
    (b"      - name: Admit the same-head source-pinned LLDB for real protected WO6\n        shell: pwsh\n        env:\n          GH_TOKEN: ${{ github.token }}\n        run: |\n          # Above stock runner probes are retained as diagnostics, not native\n          # acceptance. The original protected normal/cancel/PE/PDB and all\n          # independent SCM/Job assertions below use the reviewed repaired\n          # artifact. No binary from another source head is admitted.\n          & ide/llvm18_owner/Transfer-PinnedOwnerBundle.ps1 -Mode Receive\n", b""),
    (b"          cargo test --manifest-path ide/host/Cargo.toml --features dev045d3-ci-oracle --lib ci_source_trap_launch\n", b""),
)


def project_reviewed_wo6_current_source(blobs, prior, current):
    """Return a historical projection ONLY after verifying both exact blobs.

    Keep the old classifier strict and never grant native/release/merge approval.
    """
    if (EXPECTED[WO6_PATH][1] != WO6_PRIOR_OWNER_BLOB
            or blobs.get(WO6_PATH) != WO6_CURRENT_SOURCE_BLOB
            or original_blob_sha(prior) != WO6_PRIOR_OWNER_BLOB
            or original_blob_sha(current) != WO6_CURRENT_SOURCE_BLOB):
        raise Denied("p05_wo6_current_source_blob_not_exact")
    normalized = current
    for added, before in WO6_SOURCE_ONLY_DELTAS:
        if normalized.count(added) != 1:
            raise Denied("p05_wo6_current_source_delta_not_exact")
        normalized = normalized.replace(added, before, 1)
    if normalized != prior:
        raise Denied("p05_wo6_current_source_diff_not_exact")
    projected = dict(blobs)
    projected[WO6_PATH] = WO6_PRIOR_OWNER_BLOB
    return projected


def project_checked_out_wo6_source(root, blobs):
    def read_blob(sha):
        try:
            return subprocess.run(
                ["git", "-C", str(root), "cat-file", "blob", sha],
                check=True, timeout=20, capture_output=True,
            ).stdout
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
            raise Denied("p05_wo6_source_blob_unavailable") from exc
    return project_reviewed_wo6_current_source(
        blobs, read_blob(WO6_PRIOR_OWNER_BLOB), read_blob(WO6_CURRENT_SOURCE_BLOB)
    )


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
    # The historical 38/8 classifier above remains strict and unchanged.
    # Only a separately documented, exact original-to-current source review
    # may explain the five additional current source differences.
    scoped_path = root / "ide/audits/p05_additional_current_source_dispositions.json"
    if scoped_path.is_symlink():
        raise Denied("p05_source_disposition_file_is_linked")
    scoped = json.loads(scoped_path.read_text(encoding="utf-8"))
    projected = project_checked_out_wo6_source(root, current)
    result=compare_38_with_separate_scoped_source_dispositions(original,projected,scoped)
    result["wo6CurrentSourceRepairVerified"] = True
    result["wo6CurrentSourceRepairOwnerAuthorizationInferred"] = False
    for path in SCOPED_SOURCE_ONLY:
        historical=git("show",ORIGINAL+":"+path)
        current_text=(root/path).read_text(encoding="utf-8")
        verify_source_only_patch(path,historical,current_text)
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
