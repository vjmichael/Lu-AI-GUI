#!/usr/bin/env python3
"""Strict source-contract validator for the G5 real supervised Git/FFprobe corpus.

Only the actual Rust desktop test can generate an admissible CI fixture.
This Python function inspects returned evidence; it is NOT an OS process,
authorization, signed attestation, or substitute for native test logs.
"""
from __future__ import annotations
import re
from typing import Any

SCHEMA = "lu-memory-loop03c-reviewed-git-ffprobe-native-v1"
PRODUCER = "lu-desktop::memory_loop03c_reviewed_worker_tests"
PROVENANCE = "REAL_REVIEWED_LU_SUPERVISED_GIT_FFPROBE_WORKER_INDEPENDENT_SOURCE_ORACLE_TEST"
CAPSULE_SCHEMA = "lu-memory-loop03b-original-procedural-belief-ccm-v1"
CASES = {
    # case: (expected status, memory kind, historically verified, needs human review)
    "protected_git_history": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE",
        "VERSION_PINNED_PROCEDURAL_MEMORY", True, False),
    "protected_ffprobe_history": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE",
        "VERSION_PINNED_PROCEDURAL_MEMORY", True, False),
    "protected_git_belief": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF",
        "ORIGINAL_SOURCE_BELIEF_REVISION", True, False),
    "changed_source_old_history_not_fresh": (
        "HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE",
        "VERSION_PINNED_PROCEDURAL_MEMORY", True, False),
    "new_worker_changed_ccm_review": (
        "ORIGINAL_CONTEXT_CHANGED_HUMAN_REVIEW",
        "ORIGINAL_TASK_CONTEXT_CHANGE", False, True),
    "old_git_belief_human_retracted": (
        "RETRACTED_OR_SUPERSEDED_BELIEF",
        "ORIGINAL_SOURCE_BELIEF_REVISION", False, True),
    "old_git_procedure_revoked": (
        "NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE",
        "VERSION_PINNED_PROCEDURAL_MEMORY", False, True),
    "restart_preserves_retracted_belief": (
        "RETRACTED_OR_SUPERSEDED_BELIEF",
        "ORIGINAL_SOURCE_BELIEF_REVISION", False, True),
}
ROOT = {
    "schema": SCHEMA, "producer": PRODUCER, "provenance": PROVENANCE,
    "modelGenerations": "SCRIPTED_ONLY_REAL_OS_WORKERS",
    "originalHumanAuthenticated": True,
    "sameGrantReused": False,
    "sourceRevisionDetectedOnlyWithFreshHumanGrant": True,
    "historicalPermissionNeverGrantsExecution": True,
    "osDescendantEscapeCustodyProven": False,
    "installedLu": False,
    "reviewedWorkerOperations": 3,
    "independentOutcomeVerifiedAtObservation": 3,
    "journalOnlyUncertainRecovery": True,
}
CAPSULE_SECURITY = {
    "schema": CAPSULE_SCHEMA,
    "sourceScope": "EXPLICIT_CURRENT_HUMAN_ORIGINAL_WITHIN_HOST_PROFILE",
    "executionAuthority": "NOT_AUTHORIZED_FOR_EXECUTION",
    "priorGrantReusable": False,
    "automaticRetryAllowed": False,
    "currentSourceFreshness": "NOT_RECHECKED",
    "modelMayOverrideHostDecision": False,
    "oldOsCustodyNotImplied": True,
    "currentTaskReviewBlockedByPreflight": False,
    "currentGoalGate": "CURRENT_GOAL_REVIEWABLE_ONLY",
    "historicalRecoveryGate": "NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW",
    "projectId": "lu", "taskId": "review",
}


def _matched(obj: dict, key: str, expected: Any) -> bool:
    return obj.get(key) == expected and type(obj.get(key)) is type(expected)


def validate_reviewed_worker_corpus(document: Any) -> list[dict]:
    if not isinstance(document, dict) or set(document) != (
        set(ROOT) | {"originalGitHead", "authorizedRecheckGitHead", "cases"}
    ):
        raise ValueError("Wrong reviewed worker test evidence schema")
    for key, expected in ROOT.items():
        if not _matched(document, key, expected):
            raise ValueError("Untrusted reviewed worker provenance / report: " + key)
    start, after = (document[k] for k in ("originalGitHead", "authorizedRecheckGitHead"))
    if not all(isinstance(x, str) and re.fullmatch(r"[0-9a-f]{40}", x) for x in (start, after)):
        raise ValueError("Missing actual Git commit identities")
    if start == after:
        raise ValueError("Source revision was not independently changed")
    rows = document["cases"]
    if not isinstance(rows, list) or len(rows) != len(CASES):
        raise ValueError("Incomplete real worker memory corpus")
    seen: set[str] = set()
    turns: set[str] = set()
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "caseId", "originalHumanPrompt", "originalHumanTurnId", "expectedStatus",
            "expectedHistoricalVerified", "expectedHumanReview", "capsule"
        }:
            raise ValueError("Invalid original host evidence row")
        case_id = row["caseId"]
        if not isinstance(case_id, str) or case_id not in CASES or case_id in seen:
            raise ValueError("Duplicate or unknown real-worker case")
        seen.add(case_id)
        status, kind, verified, review = CASES[case_id]
        if not all(_matched(row, k, v) for k, v in {
            "expectedStatus": status,
            "expectedHistoricalVerified": verified,
            "expectedHumanReview": review
        }.items()):
            raise ValueError("Independent case expectations have been transformed")
        turn, prompt = row["originalHumanTurnId"], row["originalHumanPrompt"]
        if not isinstance(turn, str) or not re.fullmatch(r"g-[0-9a-f]{32}", turn) or turn in turns:
            raise ValueError("Untrusted, duplicate or nonoriginal human turn")
        turns.add(turn)
        if not isinstance(prompt, str) or not 1 <= len(prompt) <= 512:
            raise ValueError("Missing bounded human original")
        expected_prefix = {
            "VERSION_PINNED_PROCEDURAL_MEMORY": "Recall procedure project lu task review ",
            "ORIGINAL_SOURCE_BELIEF_REVISION": "Recall belief project lu task review ",
            "ORIGINAL_TASK_CONTEXT_CHANGE": "Resume project lu task review",
        }[kind]
        if not prompt.startswith(expected_prefix):
            raise ValueError("Wrong original human selector for evidence kind")
        capsule = row["capsule"]
        if not isinstance(capsule, dict):
            raise ValueError("Model-facing capsule absent")
        for key, val in CAPSULE_SECURITY.items():
            if not _matched(capsule, key, val):
                raise ValueError("Memory cannot confer current authority/freshness: " + key)
        for key, val in {
            "kind": kind,
            "status": status,
            "humanReviewRequired": review,
            "historicallyVerifiedDataAvailable": verified,
        }.items():
            if not _matched(capsule, key, val):
                raise ValueError("Host memory status disagrees with independent expectation")
        evidence = capsule.get("historicalEvidence")
        if not isinstance(evidence, dict):
            raise ValueError("Missing original source evidence detail")
        if verified and kind == "VERSION_PINNED_PROCEDURAL_MEMORY":
            records = evidence.get("historicallyRevalidatedProcedures")
            if not isinstance(records, list) or len(records) != 1:
                raise ValueError("No independently verified historical procedure")
            record = records[0]
            tool = "lu.reviewed.media.facts" if case_id == "protected_ffprobe_history" else (
                "lu.reviewed.git.head"
            )
            if (record.get("toolId") != tool
                or record.get("historicEvidence") != "INDEPENDENTLY_VERIFIED_AT_OBSERVATION"
                or record.get("priorGrantReusable") is not False
                or record.get("mayExecuteOrReplay") is not False
                or not isinstance(record.get("historicReceiptSha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", record["historicReceiptSha256"])):
                raise ValueError("Forged independent reviewed worker procedure")
        if case_id == "protected_git_belief":
            claims = evidence.get("historicalAssessedClaims")
            if not isinstance(claims, list) or len(claims) != 1 or (
                claims[0].get("status") != "independently_verified"
                or claims[0].get("reportedValue") != start
            ):
                raise ValueError("Verified original Git belief differs from observed HEAD")
        if case_id in ("old_git_belief_human_retracted", "restart_preserves_retracted_belief"):
            claims = evidence.get("historicalAssessedClaims")
            if not isinstance(claims, list) or not any(
                c.get("status") == "retracted_not_current" for c in claims
            ):
                raise ValueError("Human retraction did not survive source/restart")
        if case_id == "old_git_procedure_revoked":
            if evidence.get("historicallyRevalidatedProcedures") != []:
                raise ValueError("Invalidated procedure remained callable from memory")
        if case_id == "new_worker_changed_ccm_review":
            ccm = evidence.get("contextChange")
            if not isinstance(ccm, dict) or ccm.get("needsReview") is not True or (
                ccm.get("sourceEvidenceChanged") is not True
            ) or not ccm.get("addedBlockers"):
                raise ValueError("Changed source/CCM did not demand human review")
    if seen != set(CASES):
        raise ValueError("Missing real-worker evidence cases")
    return rows
