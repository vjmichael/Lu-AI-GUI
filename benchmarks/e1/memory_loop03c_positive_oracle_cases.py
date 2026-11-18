#!/usr/bin/env python3
"""Strict reader for 03C-G4 *test-only* positive original ANSF + Git witness corpus.

The Rust producer runs two separate native Git HEAD commands and stores an
APPLIED test receipt plus an independent SQLite journal record. It does NOT
exercise a Lu-reviewed application worker or prove live source/OS custody.
This parser cannot independently attest execution by merely reading JSON.
"""
from __future__ import annotations
import hashlib
import re
from typing import Any

SCHEMA = "lu-memory-loop03c-positive-direct-git-corpus-v1"
PROVENANCE = "TWO_NATIVE_GIT_HEAD_OS_OBSERVATIONS_WITH_ANSF_SQLITE_JOURNAL_TEST_ONLY"
PRODUCER = "lu-desktop::ansf_chat_store::memory_loop03c_positive_oracle_tests"
EFFECT = "TEST_ONLY_GIT_COMMAND_NOT_A_REVIEWED_LU_WORKER"
CAPSULE = "lu-memory-loop03b-original-procedural-belief-ccm-v1"
CASE_EXPECTATIONS = {
    # id: (status, preflight blocked, review, historically verified, kind)
    "historical_git_procedure": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE", False, False, True,
        "VERSION_PINNED_PROCEDURAL_MEMORY"),
    "historical_git_belief": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF", False, False, True,
        "ORIGINAL_SOURCE_BELIEF_REVISION"),
    "goal_blocks_historical_git": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE", True, True, True,
        "VERSION_PINNED_PROCEDURAL_MEMORY"),
    "git_tool_version_mismatch": (
        "NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE", False, True, False,
        "VERSION_PINNED_PROCEDURAL_MEMORY"),
    "human_invalidated_git_procedure": (
        "NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE", False, True, False,
        "VERSION_PINNED_PROCEDURAL_MEMORY"),
    "human_retracted_git_belief": (
        "RETRACTED_OR_SUPERSEDED_BELIEF", False, True, False,
        "ORIGINAL_SOURCE_BELIEF_REVISION"),
}
CAPSULE_SECURITY = {
    "schema": CAPSULE,
    "sourceScope": "EXPLICIT_CURRENT_HUMAN_ORIGINAL_WITHIN_HOST_PROFILE",
    "executionAuthority": "NOT_AUTHORIZED_FOR_EXECUTION",
    "priorGrantReusable": False,
    "automaticRetryAllowed": False,
    "modelMayOverrideHostDecision": False,
    "oldOsCustodyNotImplied": True,
    "currentSourceFreshness": "NOT_RECHECKED",
}


def validate_positive_corpus(document: Any) -> list[dict]:
    if not isinstance(document, dict) or set(document) != {
        "schema", "producer", "provenance", "nativeEffectExecution",
        "positiveIndependentOracleCasesIncluded", "realSourceAtObservationSha256",
        "observedGitHead", "currentSourceFreshnessProven",
        "osDescendantCustodyProven", "cases",
    }:
        raise ValueError("Not exact test-only native Git/ANSF corpus schema")
    for key, value in {
        "schema": SCHEMA, "producer": PRODUCER, "provenance": PROVENANCE,
        "nativeEffectExecution": EFFECT,
        "positiveIndependentOracleCasesIncluded": True,
        "currentSourceFreshnessProven": False,
        "osDescendantCustodyProven": False,
    }.items():
        if document.get(key) != value or type(document.get(key)) is not type(value):
            raise ValueError("Untrusted test-only Git fixture declaration: " + key)
    git_head = document["observedGitHead"]
    observed_sha = document["realSourceAtObservationSha256"]
    if not isinstance(git_head, str) or not re.fullmatch("[a-f0-9]{40}", git_head):
        raise ValueError("Actual Git HEAD is missing or malformed")
    if not isinstance(observed_sha, str) or (
        observed_sha != hashlib.sha256(git_head.encode("utf8")).hexdigest()
    ):
        raise ValueError("Independent observed Git HEAD SHA-256 does not match")
    rows = document.get("cases")
    if not isinstance(rows, list) or len(rows) != len(CASE_EXPECTATIONS):
        raise ValueError("Missing exact G4 test-only cases")
    seen: set[str] = set()
    turns: set[str] = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "caseId", "originalHumanPrompt", "originalHumanTurnId",
            "expectedStatus", "expectedPreflightBlocked",
            "expectedHumanReview", "expectedHistoricalVerified", "capsule",
        }:
            raise ValueError("Invalid positive native row shape")
        case_id = row["caseId"]
        if not isinstance(case_id, str) or case_id not in CASE_EXPECTATIONS or case_id in seen:
            raise ValueError("Duplicate or unknown G4 case")
        seen.add(case_id)
        expected_status, blocked, review, verified, kind = CASE_EXPECTATIONS[case_id]
        capsule = row["capsule"]
        if not isinstance(capsule, dict):
            raise ValueError("Missing original host capsule")
        for key, value in CAPSULE_SECURITY.items():
            if capsule.get(key) != value or type(capsule.get(key)) is not type(value):
                raise ValueError("Unsafe host capsule field: " + key)
        for key, value in {
            "status": expected_status,
            "kind": kind,
            "humanReviewRequired": review,
            "historicallyVerifiedDataAvailable": verified,
            "currentTaskReviewBlockedByPreflight": blocked,
            "projectId": "lu", "taskId": "review",
        }.items():
            if capsule.get(key) != value or type(capsule.get(key)) is not type(value):
                raise ValueError("Host scenario/expected outcome mismatch: " + key)
        for key, value in {
            "expectedStatus": expected_status,
            "expectedPreflightBlocked": blocked,
            "expectedHumanReview": review,
            "expectedHistoricalVerified": verified,
        }.items():
            if row.get(key) != value or type(row.get(key)) is not type(value):
                raise ValueError("Native independent expectation mismatch: " + key)
        turn = row["originalHumanTurnId"]
        prompt = row["originalHumanPrompt"]
        if not isinstance(turn, str) or not re.fullmatch(r"g-[a-f0-9]{32}", turn) or turn in turns:
            raise ValueError("Original authenticated source ID absent or repeated")
        turns.add(turn)
        if not isinstance(prompt, str) or not (10 <= len(prompt) <= 512) or (
            not prompt.startswith("Recall procedure project lu task review ") and
            not prompt.startswith("Recall belief project lu task review ")
        ):
            raise ValueError("Current human selector is not the original bounded form")
        detail = capsule.get("historicalEvidence")
        if not isinstance(detail, dict):
            raise ValueError("Missing native source-linked evidence detail")
        if verified and kind == "VERSION_PINNED_PROCEDURAL_MEMORY":
            records = detail.get("historicallyRevalidatedProcedures")
            if not isinstance(records, list) or len(records) != 1:
                raise ValueError("Positive procedure has no independently checked original record")
            record = records[0]
            if record.get("historicEvidence") != "INDEPENDENTLY_VERIFIED_AT_OBSERVATION" or (
                record.get("toolId") != "lu.reviewed.git.head"
            ) or record.get("toolVersion") != "1.0" or (
                record.get("applicationVersion") != "0.1"
            ) or record.get("priorGrantReusable") is not False or (
                record.get("mayExecuteOrReplay") is not False
            ):
                raise ValueError("Claimed positive historical procedure lacks checked evidence")
            if not re.fullmatch("[a-f0-9]{64}", record.get("historicReceiptSha256", "")):
                raise ValueError("Positive procedure lacks source-bound receipt digest")
        elif verified and kind == "ORIGINAL_SOURCE_BELIEF_REVISION":
            claims = detail.get("historicalAssessedClaims")
            if not isinstance(claims, list) or len(claims) != 1 or (
                claims[0].get("status") != "independently_verified"
            ) or claims[0].get("reportedValue") != git_head:
                raise ValueError("Positive belief does not match independently observed Git HEAD")
        elif case_id == "human_retracted_git_belief":
            claims = detail.get("historicalAssessedClaims")
            if not isinstance(claims, list) or not any(
                claim.get("status") == "retracted_not_current" for claim in claims
            ):
                raise ValueError("Human-retracted claim is not actually retracted")
        elif case_id == "git_tool_version_mismatch":
            if detail.get("exactToolVersion") != "2.0" or detail.get(
                "historicallyRevalidatedProcedures"
            ) != []:
                raise ValueError("Mismatched version returned a verified historic procedure")
        elif case_id == "human_invalidated_git_procedure":
            if detail.get("historicallyRevalidatedProcedures") != []:
                raise ValueError("Human-invalidated procedure remained reusable")
        if case_id == "goal_blocks_historical_git":
            if capsule.get("currentGoalGate") != "CURRENT_GOAL_CLARIFICATION_PENDING":
                raise ValueError("Historical evidence overrode clarification")
        elif capsule.get("currentGoalGate") != "CURRENT_GOAL_REVIEWABLE_ONLY":
            raise ValueError("Unexpected injected current-goal gate")
        if capsule.get("historicalRecoveryGate") != "NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW":
            raise ValueError("Unexpected injected historical OS gate")
    if seen != set(CASE_EXPECTATIONS):
        raise ValueError("Incomplete G4 positive native corpus")
    return rows
