#!/usr/bin/env python3
"""IDE-PRECORE-CLOSEOUT-R3 read-only release-candidate inventory.

This reconciles exact source and staged package metadata; it never signs,
installs, starts workers, verifies Windows kernel Job custody, or grants release.
The native Windows observation script is a separate, necessary (not sufficient)
step. A JSON claim and a passing unit test are not OS attestation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib

SHA40 = re.compile(r"[0-9a-fA-F]{40}\Z")
SHA64 = re.compile(r"[0-9a-fA-F]{64}\Z")
BASE = "f1f0ed4bc1e740d2a9f85615b988b779a46651db"
REPO = "vjmichael/Lu-AI-GUI"
ROLES = ("installer", "service", "desktop")
FIXED_ASSETS = {"installer": "lu-ide-signed-setup.exe",
                "service": "windows_recovery_service.exe",
                "desktop": "lu-ide-desktop.exe"}
CUSTODY = ("scmTerminal", "jobEmpty", "originalCancel", "revocation", "restartRecovery")
# R4: preserve the original R2 verification receipt as evidence. Its desktop
# lock sha256 field is accidentally truncated to 38 hex characters. The full
# 64-hex source-owned pin exists in the reviewed R2 source-lock successor.
R2_RECEIPT_TRUNCATED_DESKTOP = "77e317747fdf2743f55363c9809f866a770255"
R2_REVIEWED_DESKTOP_SHA256 = "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"


class Denied(ValueError):
    pass


def read_json(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(data, dict):
        raise Denied("non_object_evidence")
    return data


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def exact_hex(value: object, pattern: re.Pattern[str]) -> bool:
    return isinstance(value, str) and bool(pattern.fullmatch(value))


def clean_relative(root: Path, rel: object) -> Path:
    if root.is_symlink() or not root.is_dir():
        raise Denied("linked_or_missing_candidate_root")
    if not isinstance(rel, str) or not rel or len(rel) > 240 or "\\" in rel or ":" in rel:
        raise Denied("unsafe_package_path")
    parts = rel.split("/")
    if any(x in ("", ".", "..") or x.endswith((" ", ".")) or
           x.split(".", 1)[0].lower() in {"con", "nul", "aux", "prn",
                                         "com1", "com2", "lpt1", "lpt2"} for x in parts):
        raise Denied("unsafe_package_path")
    current = root.resolve(strict=True)
    for part in parts:
        current = current / part
        if current.is_symlink():
            raise Denied("linked_package_asset")
    if not current.is_file() or not current.resolve(strict=True).is_relative_to(root.resolve(strict=True)):
        raise Denied("missing_package_asset")
    return current


def git_output(repo: Path, *args: str) -> str:
    result = subprocess.run(["git", "-C", str(repo), *args],
                            capture_output=True, text=True, timeout=20)
    if result.returncode != 0:
        raise Denied("git_evidence_unavailable")
    return result.stdout.strip()


def source_identity(repo: Path, candidate_sha: str, r2: dict) -> dict:
    if not exact_hex(candidate_sha, SHA40):
        raise Denied("candidate_sha_unpinned")
    if git_output(repo, "rev-parse", "HEAD").lower() != candidate_sha.lower():
        raise Denied("source_head_drift")
    if git_output(repo, "status", "--porcelain", "--untracked-files=all"):
        raise Denied("source_tree_dirty")
    ancestor = subprocess.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", BASE,
                              candidate_sha], capture_output=True, timeout=20)
    if ancestor.returncode != 0:
        raise Denied("original_ide_pr_not_ancestor")
    # Never use the malformed historical receipt as an authoritative digest.
    try:
        reviewed = read_json(repo / "ide/packaging/ide-precore-r2-ipc-lock-successor.json")
    except (OSError, ValueError) as exc:
        raise Denied("r2_reviewed_lock_manifest_missing_or_invalid") from exc
    if (reviewed.get("desktopSha256") != R2_REVIEWED_DESKTOP_SHA256 or
            reviewed.get("hostSha256") != r2["host_lock_successor"]["sha256"]):
        raise Denied("r2_reviewed_lock_manifest_drift")
    if r2["desktop_lock_successor"]["sha256"] not in (
            R2_RECEIPT_TRUNCATED_DESKTOP, R2_REVIEWED_DESKTOP_SHA256):
        raise Denied("r2_receipt_lock_unknown_identity")
    pins = {
        "ide/host/Cargo.lock": reviewed["hostSha256"],
        "ide/app/src-tauri/Cargo.lock": reviewed["desktopSha256"],
    }
    for path, expected in pins.items():
        if not exact_hex(expected, SHA64) or sha256(repo / path) != expected.lower():
            raise Denied("r2_lock_bytes_drift:" + path)
    config = read_json(repo / "ide/app/src-tauri/tauri.conf.json")
    cargo = tomllib.loads((repo / "ide/app/src-tauri/Cargo.toml").read_text(encoding="utf-8"))
    if (config.get("identifier") != "org.lu.ide" or
            config.get("productName") != "Lu IDE" or
            config.get("version") != cargo.get("package", {}).get("version") or
            cargo.get("package", {}).get("name") != "lu-ide-desktop"):
        raise Denied("desktop_identity_mismatch")
    if config.get("bundle", {}).get("active") is not False:
        raise Denied("default_bundle_release_enabled")
    if cargo.get("features", {}).get("default") != []:
        raise Denied("protected_features_enabled_by_default")
    return {"commit": candidate_sha.lower(), "appId": config["identifier"],
            "version": config["version"], "defaultProtectedFeatures": False,
            "sourceAndR2LockBytesChecked": True}


def reconcile(record: dict, r2: dict, patch: Path, wo7: dict, bundle: dict,
              wo7_evidence: dict, wo8: dict, wo9: dict, *, checkout: Path | None = None,
              candidate_root: Path | None = None) -> dict:
    """Reject contradictory claims; report blockers without self-attesting native results."""
    if record.get("schema") != "lu.ide.precore.r3.admission-register.v1":
        raise Denied("r3_register_schema")
    if (record.get("repository") != REPO or record.get("originalPr") != 236 or
            record.get("originalHead") != BASE or
            r2.get("task") != "IDE-PRECORE-CLOSEOUT-R2" or
            r2.get("original_pr_sha") != BASE or
            r2.get("gh_source_pushed") is not False or
            r2.get("cumulative_applies_clean_exact_original") is not True):
        raise Denied("r2_original_provenance_mismatch")
    digest = sha256(patch)
    if (digest != r2.get("cumulative_sha256") or
            digest != record.get("r2CumulativeSha256")):
        raise Denied("r2_cumulative_patch_changed")
    receipt_desktop = r2.get("desktop_lock_successor", {}).get("sha256")
    if (record.get("r2HostLockSha256") != r2.get("host_lock_successor", {}).get("sha256") or
            record.get("r2DesktopLockSha256") != R2_REVIEWED_DESKTOP_SHA256 or
            receipt_desktop not in (R2_RECEIPT_TRUNCATED_DESKTOP, R2_REVIEWED_DESKTOP_SHA256)):
        raise Denied("r2_lock_pin_mismatch")

    blockers: list[str] = []
    # Historical receipt is immutable evidence, not a valid 64-byte source pin.
    # Keep a blocker until checked-out Git bytes independently substantiate the
    # full reviewed source pin; do not silently repair or fabricate the receipt.
    if receipt_desktop == R2_RECEIPT_TRUNCATED_DESKTOP and checkout is None:
        blockers.append("r2_truncated_receipt_requires_independent_source_lock_verification")
    candidate = record.get("candidateSha")
    if candidate is None:
        blockers.append("candidate_commit_not_published_or_pinned")
    elif not exact_hex(candidate, SHA40) or candidate.lower() == BASE:
        raise Denied("invalid_successor_candidate_commit")
    if checkout is None:
        blockers.append("clean_exact_candidate_checkout_not_verified")
        source = None
    elif candidate is None:
        blockers.append("checkout_cannot_be_verified_without_candidate_sha")
        source = None
    else:
        source = source_identity(checkout, candidate, r2)

    expected = {
        "wo7": (wo7, "lu.ide.wo7.signed-nsis.v1"),
        "bundle": (bundle, "lu.ide.wo7.bundle.v1"),
        "wo7Evidence": (wo7_evidence, "lu.ide.wo7.evidence.v1"),
        "wo8Evidence": (wo8, "lu.ide.wo8.installed.v1"),
        "wo9Evidence": (wo9, "lu.ide.wo9.native-release.v1"),
    }
    for key, (doc, schema) in expected.items():
        if not isinstance(doc, dict) or doc.get("schema") != schema:
            raise Denied("wrong_source_manifest_schema:" + key)
        doc_sha = doc.get("sourceSha")
        if doc_sha is not None and (not exact_hex(doc_sha, SHA40) or candidate is None or doc_sha.lower() != candidate.lower()):
            raise Denied("cross_manifest_source_drift:" + key)
    if wo7.get("releaseApproved") is True or record.get("releaseApproved") is True:
        raise Denied("unearned_release_approval_claim")
    if wo7.get("releaseApproved") is not False or record.get("releaseApproved") is not False:
        raise Denied("release_flag_not_explicitly_false")
    if record.get("physicalWindowsAccepted") is not False:
        raise Denied("physical_acceptance_flag_not_explicitly_false")

    if wo7.get("licensesReviewed") is not True:
        blockers.append("independent_shipping_license_review_missing")
    elif not exact_hex(wo7.get("licenseInventorySha256"), SHA64):
        raise Denied("reviewed_license_inventory_digest_missing")
    if bundle.get("approved") is not True:
        blockers.append("reviewed_wo7_payload_and_import_closure_missing")
    if wo7_evidence.get("cleanSource") is not True or not wo7_evidence.get("acceptance"):
        blockers.append("native_ci_acceptance_missing")
    if not wo8.get("observations"):
        blockers.append("wo8_installed_gui_and_recovery_receipts_missing")
    if not wo9.get("observations"):
        blockers.append("wo9_protected_workflow_receipts_missing")
    signing = record.get("signingTool")
    if not isinstance(signing, dict) or set(signing) != {"path", "sha256"}:
        raise Denied("signing_tool_register_invalid")
    if signing["path"] is None and signing["sha256"] is None:
        blockers.append("reviewed_native_signtool_pin_missing")
    elif not (isinstance(signing["path"], str) and signing["path"].strip() and
              exact_hex(signing["sha256"], SHA64)):
        raise Denied("incomplete_signing_tool_pin")
    publisher = record.get("publisherThumbprint")
    if publisher is None:
        blockers.append("reviewed_publisher_certificate_not_provisioned")
    elif not exact_hex(publisher, SHA40):
        raise Denied("invalid_publisher_thumbprint")

    signed_rows = {}
    for role in ROLES:
        row = wo7.get(role)
        if not isinstance(row, dict):
            raise Denied("wo7_missing_role:" + role)
        fields = (row.get("path"), row.get("sha256"), row.get("signerThumbprint"))
        if all(x is None for x in fields):
            blockers.append("signed_" + role + "_not_provisioned")
            continue
        if row.get("path") != FIXED_ASSETS[role]:
            raise Denied("unreviewed_candidate_binary_path:" + role)
        if (not exact_hex(row.get("sha256"), SHA64) or
                not exact_hex(row.get("signerThumbprint"), SHA40)):
            raise Denied("incomplete_signed_role_pin:" + role)
        if candidate_root is None:
            blockers.append("signed_" + role + "_bytes_not_locally_remeasured")
        else:
            actual = clean_relative(candidate_root, row.get("path"))
            if sha256(actual) != row["sha256"].lower():
                raise Denied("signed_role_digest_mismatch:" + role)
        if publisher is not None and row["signerThumbprint"].lower() != publisher.lower():
            raise Denied("unreviewed_publisher_for_role:" + role)
        signed_rows[role] = {"sha256": row["sha256"].lower(),
                             "signerThumbprint": row["signerThumbprint"].lower()}
    # Signatures and SCM/Job/cancellation/restart receipts require independent
    # native OS witnesses. No positive JSON field can replace that evidence.
    blockers.append("native_windows_authenticode_chain_not_independently_verified")
    custody = record.get("custodyReceipts")
    if not isinstance(custody, dict) or set(custody) != set(CUSTODY):
        raise Denied("custody_register_incomplete")
    seen_receipts = set()
    for key in CUSTODY:
        row = custody[key]
        if row is None:
            blockers.append("protected_worker_" + key + "_receipt_missing")
        elif (not isinstance(row, dict) or not exact_hex(row.get("sha256"), SHA64)
              or row.get("origin") != "independent-installed-windows"
              or not exact_hex(row.get("sourceSha"), SHA40) or candidate is None
              or row.get("sourceSha", "").lower() != candidate.lower()
              or not isinstance(row.get("originalOperationId"), str)
              or not row["originalOperationId"].strip()):
            raise Denied("invalid_custody_receipt_metadata:" + key)
        else:
            if row["sha256"].lower() in seen_receipts:
                raise Denied("reused_custody_receipt_digest")
            seen_receipts.add(row["sha256"].lower())
    blockers.append("independent_native_custody_receipt_authentication_pending")
    blockers.append("physical_windows_install_restart_upgrade_power_loss_not_accepted")
    blockers.append("human_release_and_merge_authorization_not_granted")
    return {
        "task": "IDE-PRECORE-CLOSEOUT-R3", "originalPr": 236,
        "base": BASE, "candidateSha": candidate,
        "r2CumulativeSha256": digest,
        "source": source, "signedBytesReconciled": signed_rows,
        "staticMetadataConsistent": True, "blocked": True,
        "physicalAdmissionApproved": False, "protectedExecutionAccepted": False,
        "releaseApproved": False, "blockers": blockers,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--register", type=Path, required=True)
    ap.add_argument("--r2-receipt", type=Path, required=True)
    ap.add_argument("--r2-patch", type=Path, required=True)
    ap.add_argument("--wo7", type=Path, required=True)
    ap.add_argument("--bundle", type=Path, required=True)
    ap.add_argument("--wo7-evidence", type=Path, required=True)
    ap.add_argument("--wo8-evidence", type=Path, required=True)
    ap.add_argument("--wo9-evidence", type=Path, required=True)
    ap.add_argument("--checkout", type=Path)
    ap.add_argument("--candidate-root", type=Path)
    ap.add_argument("--require-admission", action="store_true")
    args = ap.parse_args()
    try:
        result = reconcile(read_json(args.register), read_json(args.r2_receipt),
                           args.r2_patch, read_json(args.wo7), read_json(args.bundle),
                           read_json(args.wo7_evidence), read_json(args.wo8_evidence),
                           read_json(args.wo9_evidence),
                           checkout=args.checkout, candidate_root=args.candidate_root)
    except (Denied, OSError, ValueError, subprocess.SubprocessError, KeyError, TypeError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_r3_evidence"), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, indent=2))
    return 1 if args.require_admission else 0


if __name__ == "__main__":
    raise SystemExit(main())
