#!/usr/bin/env python3
"""Bounded queue hygiene: request cancellation ONLY for superseded, replaceable
queued/pending IDE and Lu Core pull-request or push runs.

No in-progress/dispatch jobs, no unknown branches, no current branch head,
no stale workflow without a new-head matching workflow+event. A 202 is only
GitHub's acceptance of the cancellation request, not terminal proof.
Read-only by default. Runs inside dedicated GitHub Actions with actions:write.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import time
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

REPO = "vjmichael/Lu-AI-GUI"
API = "https://api.github.com"
ALLOWED_PREFIXES = ("lu/", "ide/")
EXCLUDED_BRANCH = "ide/queued-actions-prune-20261108"
CANCELLABLE = frozenset(("queued", "pending"))
ELIGIBLE_EVENTS = frozenset(("push", "pull_request"))
MIN_AGE_SECONDS = 300
MAX_PAGES = 25
PAGE_SIZE = 100
MAX_REQUESTS = 35
SHA40 = re.compile(r"^[0-9a-fA-F]{40}$")


def allowed_branch(name):
    return (isinstance(name, str) and
            name != EXCLUDED_BRANCH and
            any(name.startswith(prefix) for prefix in ALLOWED_PREFIXES) and
            len(name) <= 240 and "\x00" not in name)


def eligible(run, branch, head, now):
    if not isinstance(run, dict) or not allowed_branch(branch):
        return False
    if (run.get("head_branch") != branch or
            run.get("status") not in CANCELLABLE or
            run.get("event") not in ELIGIBLE_EVENTS):
        return False
    sha = run.get("head_sha")
    if not isinstance(sha, str) or not SHA40.fullmatch(sha) or sha == head:
        return False
    if not isinstance(run.get("id"), int) or run["id"] <= 0:
        return False
    if not isinstance(run.get("workflow_id"), int) or run["workflow_id"] <= 0:
        return False
    try:
        timestamp = datetime.fromisoformat(run["created_at"].replace("Z", "+00:00"))
    except (ValueError, AttributeError, KeyError, TypeError):
        return False
    return 0 <= (now - timestamp).total_seconds() and (
        now - timestamp).total_seconds() >= MIN_AGE_SECONDS


def replacement_exists(run, head_runs, branch, head):
    """Do not cancel a unique workflow/check not reissued on current head."""
    return any(other.get("head_branch") == branch and
               other.get("head_sha") == head and
               other.get("workflow_id") == run.get("workflow_id") and
               other.get("event") == run.get("event") and
               other.get("id") != run.get("id") and
               other.get("status") in ("queued", "pending", "in_progress",
                                       "completed", "waiting", "requested")
               for other in head_runs)


class Github:
    def __init__(self, token):
        self.token = token

    def request(self, method, route):
        if not route.startswith("/repos/" + REPO + "/"):
            raise RuntimeError("DENIED_OFF_SCOPE_ENDPOINT")
        headers = {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "lu-ide-lu-core-queue-hygiene",
        }
        if self.token:
            headers["Authorization"] = "Bearer " + self.token
        req = Request(API + route, headers=headers, method=method)
        try:
            with urlopen(req, timeout=20) as response:
                raw = response.read(4 * 1024 * 1024 + 1)
                status = response.status
        except HTTPError as exc:
            raise RuntimeError(f"GITHUB_HTTP_{exc.code}_{method}") from None
        except (URLError, TimeoutError) as exc:
            raise RuntimeError("GITHUB_CONNECTION_UNAVAILABLE") from exc
        if len(raw) > 4 * 1024 * 1024:
            raise RuntimeError("GITHUB_RESPONSE_TOO_LARGE")
        if not raw:
            return status, {}
        try:
            return status, json.loads(raw.decode("utf-8"))
        except (ValueError, UnicodeDecodeError):
            raise RuntimeError("INVALID_GITHUB_JSON") from None

    def pages(self, route, params):
        found = []
        for page in range(1, MAX_PAGES + 1):
            _, result = self.request("GET", route + "?" + urlencode({
                **params, "per_page": PAGE_SIZE, "page": page,
            }))
            batch = result.get("workflow_runs")
            if not isinstance(batch, list):
                raise RuntimeError("MALFORMED_WORKFLOW_RUNS")
            found.extend(batch)
            if len(batch) < PAGE_SIZE:
                return found
        raise RuntimeError("DENIED_QUEUE_EXCEEDS_BOUNDED_AUDIT")

    def branch_head(self, branch):
        _, result = self.request(
            "GET", "/repos/" + REPO + "/branches/" + quote(branch, safe=""))
        sha = result.get("commit", {}).get("sha")
        if not isinstance(sha, str) or not SHA40.fullmatch(sha):
            raise RuntimeError("BRANCH_HEAD_NOT_VERIFIED")
        return sha

    def current_runs(self, head):
        _, result = self.request(
            "GET", "/repos/" + REPO + "/actions/runs?" +
            urlencode({"head_sha": head, "per_page": 100}))
        runs = result.get("workflow_runs")
        if not isinstance(runs, list) or result.get("total_count", 101) > 100:
            raise RuntimeError("CURRENT_HEAD_CHECK_SET_NOT_EXHAUSTIVE")
        return runs


def audit(api, *, apply=False, now=None):
    now = now or datetime.now(timezone.utc)
    if apply and not api.token:
        raise RuntimeError("CANCELLATION_REQUIRES_ACTIONS_WRITE_TOKEN")
    runs = []
    for status in ("queued", "pending"):
        runs.extend(api.pages("/repos/" + REPO + "/actions/runs",
                              {"status": status}))
    by_id = {run["id"]: run for run in runs if
             isinstance(run, dict) and isinstance(run.get("id"), int)}
    runs = list(by_id.values())
    branches = sorted(set(run.get("head_branch") for run in runs
                          if allowed_branch(run.get("head_branch"))))
    heads, current, skipped = {}, {}, []
    for branch in branches:
        try:
            sha = api.branch_head(branch)
            heads[branch] = sha
            current[branch] = api.current_runs(sha)
        except RuntimeError as exc:
            skipped.append({"branch": branch, "reason": str(exc)})
    candidates = [
        run for run in runs if run.get("head_branch") in heads and
        eligible(run, run["head_branch"], heads[run["head_branch"]], now) and
        replacement_exists(run, current[run["head_branch"]],
                           run["head_branch"], heads[run["head_branch"]])
    ]
    # Oldest Lu Core runs first, then IDE. Revalidate at the moment of action.
    candidates.sort(key=lambda r: (
        not r["head_branch"].startswith("lu/"), r.get("created_at", ""), r["id"]))
    report = {
        "schemaVersion": 2, "repo": REPO, "observedAt": now.isoformat(),
        "apply": apply, "scope": "replaceable stale queued/pending lu/ and ide/ runs",
        "currentHeadsProtected": heads,
        "inProgressAndDispatchProtected": True,
        "matchingCurrentHeadWorkflowRequired": True,
        "queueRunsInspected": len(runs),
        "eligibleSupersededCount": len(candidates),
        "requestsCappedAt": MAX_REQUESTS,
        "deferredCandidates": max(0, len(candidates) - MAX_REQUESTS),
        "branchInspectionErrors": skipped,
        "candidates": [{
            "runId": r["id"], "branch": r["head_branch"],
            "staleSha": r["head_sha"], "workflowId": r["workflow_id"],
            "workflowName": str(r.get("name", ""))[:120],
            "event": r["event"], "statusAtDiscovery": r["status"],
        } for r in candidates[:MAX_REQUESTS]],
        "acceptedRequests": [], "terminalCancelledRuns": [],
        "terminalOtherOutcomes": [], "notTerminalAtAuditEnd": [],
        "outcomes": [], "errors": [],
    }
    if not apply:
        return report
    for candidate in report["candidates"]:
        rid, branch = candidate["runId"], candidate["branch"]
        try:
            # Re-fetch both branch head and run. The replacement must still
            # exist for the SAME workflow ID and event on the current SHA.
            head = api.branch_head(branch)
            _, live = api.request(
                "GET", f"/repos/{REPO}/actions/runs/{rid}")
            if (not eligible(live, branch, head, now) or
                    not replacement_exists(live, api.current_runs(head),
                                           branch, head)):
                outcome = "preserved: changed, running or no current-head replacement"
            else:
                status, _ = api.request(
                    "POST", f"/repos/{REPO}/actions/runs/{rid}/cancel")
                if status != 202:
                    raise RuntimeError("CANCELLATION_NOT_ACCEPTED")
                report["acceptedRequests"].append(rid)
                outcome = "github_202_cancel_requested_not_terminal"
            report["outcomes"].append({"runId": rid, "outcome": outcome})
        except RuntimeError as exc:
            report["errors"].append({"runId": rid, "reason": str(exc)})
    # GitHub 202 only acknowledges an asynchronous cancellation REQUEST.
    # Do not count it as a cancellation unless a subsequent independent GET
    # reports status=completed AND conclusion=cancelled for that exact run.
    pending = set(report["acceptedRequests"])
    for sweep in range(4):
        if not pending:
            break
        if sweep:
            time.sleep(2)
        for rid in sorted(pending):
            try:
                _, observed = api.request(
                    "GET", f"/repos/{REPO}/actions/runs/{rid}")
                if observed.get("id") != rid:
                    raise RuntimeError("CANCELLATION_RECEIPT_RUN_ID_MISMATCH")
                if observed.get("status") == "completed":
                    if observed.get("conclusion") == "cancelled":
                        report["terminalCancelledRuns"].append(rid)
                    else:
                        report["terminalOtherOutcomes"].append({
                            "runId": rid,
                            "conclusion": str(observed.get("conclusion", ""))[:40],
                        })
                    pending.remove(rid)
            except RuntimeError as exc:
                report["errors"].append({
                    "runId": rid, "reason": "terminal_verification_" + str(exc)
                })
                pending.remove(rid)
    report["terminalCancelledRuns"].sort()
    report["notTerminalAtAuditEnd"] = sorted(pending)
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--receipt", default="ide-queue-hygiene.json")
    args = parser.parse_args()
    result = audit(Github(os.environ.get("GITHUB_TOKEN", "")),
                   apply=args.apply)
    path = Path(args.receipt)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n",
                    encoding="utf-8")
    print(json.dumps({
        "queueRunsInspected": result["queueRunsInspected"],
        "eligibleSupersededCount": result["eligibleSupersededCount"],
        "cancelRequestsAccepted": len(result["acceptedRequests"]),
        "terminalCancelledVerified": len(result["terminalCancelledRuns"]),
        "cancellationStillUnverified": len(result["notTerminalAtAuditEnd"]),
        "terminalOtherOutcomes": len(result["terminalOtherOutcomes"]),
        "requestFailures": len(result["errors"]),
        "deferredCandidates": result["deferredCandidates"],
        "receipt": str(path),
    }, sort_keys=True))
    if result["errors"] or result["notTerminalAtAuditEnd"] or result["terminalOtherOutcomes"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
