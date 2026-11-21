#!/usr/bin/env python3
"""WO7 positive technical closure: real signed candidate + independent pinned review.

This is not legal advice, code-signing custody, installed OS acceptance or
release approval. The protected GitHub environment must supply the digest of
an independent review file; it is deliberately NOT a workflow-dispatch input.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

from verify_wo7_nsis_release import Denied, safe_asset, digest, native_signature
from verify_wo7_candidate import audit_candidate
from verify_p04_license_bundle import audit as audit_licenses
from verify_wo7_pe_imports import audit_staged_pe

SHA40 = re.compile(r"[0-9a-fA-F]{40}\Z")
SHA64 = re.compile(r"[0-9a-fA-F]{64}\Z")
EXPECTED_REVIEW = "lu.ide.wo7.independent-bundle-review.v1"
EXPECTED_DYNAMIC = "lu.ide.wo7.dynamic-import-review.v1"


def _json(root: Path, name: str) -> tuple[Path, dict]:
    path = safe_asset(root, name)
    if path.stat().st_size > 2_000_000:
        raise Denied("oversized_independent_review")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise Denied("review_record_not_object")
    return path, value


def _canon_sha(value: object) -> str:
    if not isinstance(value, str) or not SHA64.fullmatch(value):
        raise Denied("reviewed_digest_missing")
    return value.lower()


def audit(candidate_root: Path, review_root: Path, source_sha: str,
          protected_review_sha: str, candidate_run_id: int, review_run_id: int,
          reviewer_login: str, signature=native_signature) -> dict:
    if not isinstance(source_sha, str) or not SHA40.fullmatch(source_sha):
        raise Denied("invalid_source_sha")
    # The exact review digest MUST originate in the protected environment,
    # not from an operator-selectable workflow input or the review itself.
    pinned = _canon_sha(protected_review_sha)
    if (type(candidate_run_id) is not int or candidate_run_id <= 0 or
            type(review_run_id) is not int or review_run_id <= 0 or
            candidate_run_id == review_run_id):
        raise Denied("independent_custody_run_ids_required")
    if (not isinstance(reviewer_login, str) or
            not re.fullmatch(r"[A-Za-z0-9-]{1,39}", reviewer_login)):
        raise Denied("protected_reviewer_login_missing")
    if candidate_root.is_symlink() or review_root.is_symlink():
        raise Denied("review_root_symlink")
    candidate_root = candidate_root.resolve(strict=True)
    review_root = review_root.resolve(strict=True)
    if candidate_root == review_root:
        raise Denied("review_and_candidate_not_independent")
    manifest = safe_asset(candidate_root, "wo7-release.manifest.json")
    candidate = audit_candidate(manifest, candidate_root, source_sha, signature=signature)
    legal_root = candidate_root / "THIRD_PARTY_LICENSES"
    if legal_root.is_symlink() or not legal_root.is_dir():
        raise Denied("candidate_legal_bundle_missing")
    legal = audit_licenses(legal_root, source_sha, require_complete=True, require_upstream=True)
    if (not legal.get("actualBytesVerified") or legal.get("unresolved") != 0 or
            not legal.get("upstreamRetrievalVerified") or legal.get("signedOrLegallyApproved") is not False):
        raise Denied("technical_license_provenance_incomplete")
    staged_root = review_root / "staged-payload"
    if staged_root.is_symlink() or not staged_root.is_dir():
        raise Denied("independent_staged_payload_missing")
    static = audit_staged_pe(staged_root)
    if (static.get("staticImportClosure") is not True or
            static.get("fullReviewedPrivateDllChain") is not True or
            static.get("dynamicImportAuditStillRequired") is not True):
        raise Denied("real_static_import_closure_incomplete")
    stage_path, stage = _json(staged_root, "resources/wo7-staged-tree.json")
    if stage.get("schema") != "lu.ide.wo7.staged-tree.v1" or stage.get("sourceSha") != source_sha:
        raise Denied("reviewed_stage_wrong_source")
    rows = stage.get("files")
    if not isinstance(rows, list) or len(rows) < 4:
        raise Denied("reviewed_stage_inventory_incomplete")
    expected_files = {"resources/wo7-staged-tree.json",
                      "resources/wo7-service-policy.json",
                      "resources/installer/wo7-service-lifecycle.ps1"}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("destination"), str):
            raise Denied("reviewed_stage_row_invalid")
        expected_files.add(row["destination"])
        for notice in row.get("notices", []):
            if not isinstance(notice, dict) or not isinstance(notice.get("path"), str):
                raise Denied("reviewed_notice_invalid")
            expected_files.add(notice["path"])
    actual_files = set()
    for directory, subdirs, names in os.walk(staged_root, followlinks=False):
        for name in subdirs + names:
            stage_entry = Path(directory) / name
            if stage_entry.is_symlink():
                raise Denied("staged_payload_contains_link")
        for name in names:
            actual_files.add((Path(directory) / name).relative_to(staged_root).as_posix())
    if actual_files != expected_files:
        raise Denied("staged_payload_has_unlisted_or_missing_bytes")
    installed_manifest = json.loads(manifest.read_text(encoding="utf-8"))
    if installed_manifest.get("releaseApproved") is not False:
        raise Denied("signed_candidate_incorrectly_claimed_release")
    for role in ("desktop", "service"):
        found = [row for row in rows if isinstance(row, dict) and row.get("role") == role]
        if len(found) != 1 or _canon_sha(found[0].get("sha256")) != _canon_sha(installed_manifest[role]["sha256"]):
            raise Denied("signed_candidate_stage_binary_mismatch:" + role)
        if digest(safe_asset(staged_root, found[0]["destination"])) != _canon_sha(found[0]["sha256"]):
            raise Denied("staged_binary_bytes_changed:" + role)
    dynamic_path, dynamic = _json(review_root, "dynamic-import-review.json")
    if (dynamic.get("schema") != EXPECTED_DYNAMIC or dynamic.get("sourceSha") != source_sha or
            dynamic.get("closureReviewed") is not True or dynamic.get("usesRealNativeBinaryInspection") is not True or
            not isinstance(dynamic.get("reviewMethod"), str) or len(dynamic["reviewMethod"].strip()) < 16):
        raise Denied("independent_dynamic_import_review_incomplete")
    # The review must bind EACH actual native file in the static import inventory,
    # not simply state that some imports were inspected.
    reported = dynamic.get("inspectedSha256ByDestination")
    expected = {row["destination"]: _canon_sha(row["sha256"]) for row in rows}
    if (not isinstance(reported, dict) or reported != expected or
            len(reported) != len(expected)):
        raise Denied("dynamic_review_does_not_cover_exact_binary_tree")
    review_path, review = _json(review_root, "independent-review.json")
    if digest(review_path) != pinned:
        raise Denied("independent_review_not_pinned_by_protected_environment")
    if (review.get("schema") != EXPECTED_REVIEW or review.get("sourceSha") != source_sha or
            review.get("candidateRunId") != candidate_run_id or
            review.get("reviewRunId") != review_run_id or
            review.get("legalReviewApproved") is not True or
            review.get("dynamicImportReviewApproved") is not True or
            review.get("reviewerIdentity") != reviewer_login):
        raise Denied("independent_legal_or_import_review_missing")
    actual = {
        "licenseInventorySha256": digest(safe_asset(legal_root, "inventory.json")),
        "stageAttestationSha256": digest(stage_path),
        "dynamicReviewSha256": digest(dynamic_path),
        "staticImportReportSha256": hashlib.sha256(json.dumps(static, sort_keys=True,
                                                separators=(",", ":")).encode("utf-8")).hexdigest(),
        "installerSha256": _canon_sha(installed_manifest["installer"]["sha256"]),
        "desktopSha256": _canon_sha(installed_manifest["desktop"]["sha256"]),
        "serviceSha256": _canon_sha(installed_manifest["service"]["sha256"]),
    }
    for key, observed in actual.items():
        if _canon_sha(review.get(key)) != observed:
            raise Denied("independent_review_digest_mismatch:" + key)
    return {
        "schema": "lu.ide.wo7.positive-bundle-audit.v1",
        "sourceSha": source_sha,
        "candidateRunId": candidate_run_id,
        "independentReviewRunId": review_run_id,
        "protectedReviewSha256": pinned,
        "actualSignedBinaryCount": len(candidate["verifiedSignatures"]),
        "auditedNativeBinaryCount": len(expected),
        "completeLegalBytesAndOriginalSourcesVerified": True,
        "reviewedStagedStaticImportClosureVerified": True,
        "independentDynamicAndLegalReviewDigestMatched": True,
        "releaseAccepted": False,
        "installedSCMJobAcceptance": False,
        "independentGitHubAndPhysicalWitnessReviewStillRequired": True,
    }


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--candidate-root", type=Path, required=True)
    p.add_argument("--review-root", type=Path, required=True)
    p.add_argument("--source-sha", required=True)
    p.add_argument("--protected-review-sha256", required=True)
    p.add_argument("--candidate-run-id", type=int, required=True)
    p.add_argument("--review-run-id", type=int, required=True)
    p.add_argument("--protected-reviewer-login", required=True)
    args = p.parse_args()
    try:
        result = audit(args.candidate_root, args.review_root, args.source_sha,
                       args.protected_review_sha256, args.candidate_run_id,
                       args.review_run_id, args.protected_reviewer_login)
    except (OSError, TypeError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_bundle_evidence"), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
