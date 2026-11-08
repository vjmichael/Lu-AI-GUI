#!/usr/bin/env python3
"""Conservatively request cancellation of superseded queued IDE workflow runs.

This script NEVER touches an in-progress job, the current head of an IDE
branch, a foreign project branch, or a run younger than the safety floor.
A 202 response means "GitHub accepted a cancellation request", NOT that
the run is already cancelled or that runner capacity is restored.

Requires GITHUB_TOKEN with actions:write when --apply is specified.
By default this program is read-only. Only the explicitly configured IDE
branches are in scope. It creates a structured receipt for audit.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

REPO = "vjmichael/Lu-AI-GUI"
BRANCHES = (
    "ide/b2-persistent-draft-recovery-20261108",
    "ide/b3-saved-project-search-port-20261108",
    "ide/b4-split-editor-current-stack-20261108",
    "ide/b5-large-project-editor-working-set-20261108",
)
CANCELLABLE_STATES = frozenset(("queued", "pending"))
MIN_AGE_SECONDS = 300
MAX_TOTAL_CANCELLATIONS = 50


def request(method: str, route: str, *, token: str, payload=None):
    if not route.startswith("/repos/" + REPO + "/"):
        raise RuntimeError("DENIED: off-scope GitHub endpoint")
    url = "https://api.github.com" + route
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "lu-ide-superseded-queue-reaper",
    }
    if token:
        headers["Authorization"] = "Bearer " + token
    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    if body is not None:
        headers["Content-Type"] = "application/json"
    req = Request(url, data=body, headers=headers, method=method)
    try:
        with urlopen(req, timeout=18) as response:
            raw = response.read(4 * 1024 * 1024 + 1)
            if len(raw) > 4 * 1024 * 1024:
                raise RuntimeError("DENIED: oversized GitHub API response")
            status = response.status
    except HTTPError as error:
        # Do not expose tokens, response body, or arbitrary external content.
        raise RuntimeError(
            f"GITHUB_API_{method}_HTTP_{error.code}_{route.split('?')[0]}"
        ) from None
    except (URLError, TimeoutError) as error:
        raise RuntimeError("GITHUB_API_CONNECTION_UNAVAILABLE") from error
    if not raw:
        return status, {}
    try:
        return status, json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError):
        raise RuntimeError("DENIED: invalid GitHub API JSON") from None


def current_branch_sha(branch: str, token: str):
    _, data = request(
        "GET", f"/repos/{REPO}/branches/{quote(branch, safe='')}", token=token
    )
    sha = data.get("commit", {}).get("sha")
    if not isinstance(sha, str) or len(sha) != 40:
        raise RuntimeError("DENIED: cannot verify branch head SHA")
    return sha


def eligible_run(run, branch, head_sha, now):
    if not isinstance(run, dict):
        return False
    if run.get("head_branch") != branch or run.get("head_sha") == head_sha:
        return False
    if run.get("status") not in CANCELLABLE_STATES:
        return False
    if not isinstance(run.get("head_sha"), str) or len(run["head_sha"]) != 40:
        return False
    timestamp = run.get("created_at")
    if not isinstance(timestamp, str):
        return False
    try:
        created = datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
    except ValueError:
        return False
    return (now - created).total_seconds() >= MIN_AGE_SECONDS


def list_runs(branch: str, status: str, token: str):
    found = []
    for page in range(1, 6):
        params = urlencode({
            "branch": branch, "status": status, "per_page": 100,
            "page": page,
        })
        _, page_obj = request(
            "GET", f"/repos/{REPO}/actions/runs?{params}", token=token
        )
        batch = page_obj.get("workflow_runs", [])
        if not isinstance(batch, list):
            raise RuntimeError("DENIED: malformed workflow list")
        found.extend(batch)
        if len(batch) < 100:
            break
        if page == 5:
            raise RuntimeError("DENIED: too many queued runs for bounded audit")
    return found


def audit(*, token: str, apply: bool, now=None):
    now = now or datetime.now(timezone.utc)
    if apply and not token:
        raise RuntimeError("DENIED: no authenticated token for workflow cancellation")
    report = {
        "schemaVersion": 1, "repo": REPO, "apply": apply,
        "observedAt": now.isoformat(),
        "scope": "superseded queued/pending IDE branch SHAs only",
        "currentHeadRunsProtected": True, "inProgressRunsProtected": True,
        "maxCancelRequests": MAX_TOTAL_CANCELLATIONS,
        "branches": [], "candidates": [], "acceptedRequests": [],
        "errors": [],
    }
    candidates = {}
    for branch in BRANCHES:
        head = current_branch_sha(branch, token)
        runs = []
        for status in ("queued", "pending"):
            runs.extend(list_runs(branch, status, token))
        branch_stats = {
            "branch": branch, "headSha": head,
            "inspected": len(runs), "superseded": 0,
        }
        for run in runs:
            run_id = run.get("id")
            if not isinstance(run_id, int) or run_id <= 0:
                continue
            if eligible_run(run, branch, head, now):
                candidates[run_id] = {
                    "runId": run_id,
                    "branch": branch,
                    "staleSha": run["head_sha"],
                    "workflowName": str(run.get("name") or "")[:140],
                    "statusAtDiscovery": run["status"],
                }
                branch_stats["superseded"] += 1
        report["branches"].append(branch_stats)
    if len(candidates) > MAX_TOTAL_CANCELLATIONS:
        raise RuntimeError("DENIED: stale run count exceeds safe cancellation cap")
    report["candidates"] = list(candidates.values())
    if not apply:
        return report

    for item in report["candidates"]:
        run_id = item["runId"]
        branch = item["branch"]
        try:
            # Compare the current branch head AGAIN before each write.
            head = current_branch_sha(branch, token)
            if head == item["staleSha"]:
                raise RuntimeError("DENIED: candidate became the current branch head")
            _, latest = request(
                "GET", f"/repos/{REPO}/actions/runs/{run_id}", token=token
            )
            if not eligible_run(latest, branch, head, now):
                # Its job may have been picked up, finished, or cancelled.
                item["outcome"] = "skipped_no_longer_stale_and_queued"
                continue
            if latest.get("id") != run_id:
                raise RuntimeError("DENIED: workflow run id mismatch")
            status, _ = request(
                "POST", f"/repos/{REPO}/actions/runs/{run_id}/cancel",
                token=token,
            )
            if status != 202:
                raise RuntimeError("DENIED: cancellation not accepted")
            item["outcome"] = "github_cancel_request_accepted_not_yet_verified"
            report["acceptedRequests"].append(run_id)
        except RuntimeError as error:
            item["outcome"] = "not_cancelled"
            report["errors"].append({
                "runId": run_id, "reason": str(error)[:240]
            })
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument(
        "--receipt", default=os.environ.get(
            "RUNNER_TEMP", "/tmp") + "/ide-queued-actions-prune.json",
    )
    args = parser.parse_args()
    result = audit(
        token=os.environ.get("GITHUB_TOKEN", ""),
        apply=args.apply,
    )
    path = Path(args.receipt)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, sort_keys=True, indent=2) + "\n",
                    encoding="utf-8")
    summary = {
        "apply": args.apply,
        "supersededCandidates": len(result["candidates"]),
        "githubAcceptedCancelRequests": len(result["acceptedRequests"]),
        "failures": len(result["errors"]),
        "protectedCurrentHeads": [r["headSha"] for r in result["branches"]],
        "receipt": str(path),
    }
    print(json.dumps(summary, sort_keys=True))
    if result["errors"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
