#!/usr/bin/env python3
"""Read-only P06 native OS evidence snapshot from GitHub's actual Actions API.

NEVER promotes an IDE release, native worker authority, legal/signing review,
or an OS gate. Local source JSON, tool output, job names, old green runs, and
partial matrices are not acceptance. All live observations refer to one
literal full commit SHA and one public repository.
"""
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REPO = "vjmichael/Lu-AI-GUI"
SCHEMA = "lu.ide.p06.live-native-actions.v1"
MAX_PAGES = 5
PAGE_SIZE = 100

# Each row is an independent *observation*, never release authorization.
# Workflow paths/job names are pinned to the current standalone IDE source.
GATES = (
    ("P02_source", ".github/workflows/ide-p02-native-checks.yml",
     ("exact-head-native (ubuntu-24.04)", "exact-head-native (windows-2022)"),
     "native_source"),
    ("P03_debugger", ".github/workflows/ide-wo6-native-os-acceptance.yml",
     ("native-windows-fixed-debugger", "native-linux-delegated-fixed-debugger",
      "native-linux-protected-cargo"), "protected_native"),
    ("P04_windows_installed", ".github/workflows/ide-wo8-installed-windows.yml",
     ("installed-unsigned-native-ui",), "unsigned_installed"),
    ("P04_linux_installed", ".github/workflows/ide-wo8-installed-linux.yml",
     ("linux-installed-deb",), "unsigned_installed"),
    ("P06_dependency", ".github/workflows/ide-p06-dependency-evidence.yml",
     ("resolved-dependency-and-notice-evidence (ubuntu-24.04)",
      "resolved-dependency-and-notice-evidence (windows-2022)"),
     "source_dependency"),
)


class EvidenceError(ValueError):
    pass


def sha_check(sha):
    if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise EvidenceError("invalid_exact_source_sha")
    return sha


def _public_github_json(path):
    # Network access only to one public repository under the exact API origin.
    if not path.startswith("/repos/" + REPO + "/actions/"):
        raise EvidenceError("github_api_scope_denied")
    url = "https://api.github.com" + path
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "lu-ide-p06-readonly-native-evidence",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request(url, headers=headers, method="GET")
    # Follow no arbitrary redirects, no mutable remote input from source.
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            if response.geturl().split("?", 1)[0].startswith(
                "https://api.github.com/repos/" + REPO + "/actions/"
            ) is False:
                raise EvidenceError("github_api_redirect_denied")
            data = response.read(6_000_001)
            if len(data) > 6_000_000:
                raise EvidenceError("github_api_response_oversize")
            result = json.loads(data)
    except (OSError, ValueError, urllib.error.URLError) as exc:
        raise EvidenceError("github_api_unavailable_or_invalid") from exc
    if not isinstance(result, dict):
        raise EvidenceError("github_api_not_object")
    return result


def _all_pages(resource, key):
    results = []
    seen = set()
    total = None
    for page in range(1, MAX_PAGES + 1):
        separator = "&" if "?" in resource else "?"
        obj = _public_github_json(
            resource + separator + f"per_page={PAGE_SIZE}&page={page}"
        )
        n = obj.get("total_count")
        items = obj.get(key)
        if isinstance(n, bool) or not isinstance(n, int) or n < 0 or (
            not isinstance(items, list)
        ):
            raise EvidenceError("github_api_invalid_pagination")
        if total is None:
            total = n
        elif n != total:
            raise EvidenceError("github_api_changed_during_snapshot")
        for item in items:
            if not isinstance(item, dict):
                raise EvidenceError("github_api_invalid_item")
            ident = item.get("id")
            if isinstance(ident, bool) or not isinstance(ident, int) or ident <= 0:
                raise EvidenceError("github_api_invalid_id")
            if ident in seen:
                raise EvidenceError("github_api_duplicate_item")
            seen.add(ident)
            results.append(item)
        if len(results) >= total:
            if len(results) != total:
                raise EvidenceError("github_api_inconsistent_total")
            return results
        if not items:
            raise EvidenceError("github_api_truncated_collection")
    raise EvidenceError("github_api_page_limit")


def get_runs(sha):
    sha_check(sha)
    # Github's documented list workflow runs filter by exact commit.
    return _all_pages("/repos/" + REPO + "/actions/runs?head_sha=" + sha,
                      "workflow_runs")


def get_jobs(run_id):
    if isinstance(run_id, bool) or not isinstance(run_id, int) or run_id <= 0:
        raise EvidenceError("invalid_run_id")
    return _all_pages(f"/repos/{REPO}/actions/runs/{run_id}/jobs", "jobs")


def _eligible(run, sha, workflow):
    if run.get("head_sha") != sha or run.get("path") != workflow:
        return False
    repository = run.get("repository")
    head_repo = run.get("head_repository")
    if not isinstance(repository, dict) or repository.get("full_name") != REPO:
        return False
    if not isinstance(head_repo, dict) or head_repo.get("full_name") != REPO:
        return False
    if run.get("event") not in ("push", "pull_request", "workflow_dispatch"):
        return False
    return True


def _rank(run):
    attempt = run.get("run_attempt", 0)
    number = run.get("run_number", 0)
    if not all(isinstance(x, int) and not isinstance(x, bool) and x >= 0
               for x in (attempt, number)):
        raise EvidenceError("invalid_actions_run_attempt")
    return (run.get("created_at", ""), number, attempt, run["id"])


def assess(sha, runs, jobs_for_run):
    """Pure, testable classifier. jobs_for_run(id) is live API in production."""
    sha_check(sha)
    if not isinstance(runs, list):
        raise EvidenceError("invalid_run_list")
    if len({g[0] for g in GATES}) != len(GATES):
        raise EvidenceError("duplicate_gate")
    observations = []
    for gate_id, workflow, required_jobs, kind in GATES:
        candidates = [r for r in runs if isinstance(r, dict)
                      and _eligible(r, sha, workflow)]
        if not candidates:
            observations.append({
                "gate": gate_id, "kind": kind,
                "workflow": workflow, "state": "missing",
                "runId": None, "jobsObserved": 0,
                "requiredJobs": list(required_jobs),
            })
            continue
        run = max(candidates, key=_rank)
        if not isinstance(run["id"], int) or isinstance(run["id"], bool):
            raise EvidenceError("invalid_actions_run_id")
        state = "pending"
        names_seen = set()
        jobs = jobs_for_run(run["id"])
        if not isinstance(jobs, list):
            raise EvidenceError("invalid_job_list")
        required = set(required_jobs)
        matched = {}
        for job in jobs:
            if not isinstance(job, dict):
                raise EvidenceError("invalid_job")
            name = job.get("name")
            if name in required:
                if name in names_seen:
                    state = "invalid_duplicate_job"
                    break
                names_seen.add(name)
                if job.get("head_sha") not in (None, sha):
                    state = "invalid_job_sha"
                    break
                matched[name] = job
        else:
            if run.get("status") == "completed":
                if run.get("conclusion") != "success":
                    state = "failed_or_cancelled"
                elif set(matched) != required:
                    state = "incomplete_jobs"
                elif all(j.get("status") == "completed" and
                         j.get("conclusion") == "success"
                         for j in matched.values()):
                    state = "observed_success"
                else:
                    state = "failed_or_incomplete_jobs"
            elif run.get("status") in ("queued", "in_progress", "requested",
                                       "waiting", "pending"):
                state = "pending"
            else:
                state = "invalid_run_state"
        observations.append({
            "gate": gate_id, "kind": kind, "workflow": workflow,
            "state": state, "runId": run["id"],
            "jobsObserved": len(matched),
            "requiredJobs": list(required_jobs),
        })
    # Explicitly never issue permissions or imply signed installed release.
    return {
        "schema": SCHEMA, "repository": REPO, "sourceSha": sha,
        "source": "github_actions_read_only_public_api",
        "observations": observations,
        "allObservedGatesSuccessful": all(
            item["state"] == "observed_success" for item in observations
        ),
        "releaseApproved": False,
        "signingIdentityReviewed": False,
        "licenseLegalReviewApproved": False,
        "protectedExecutionAccepted": False,
        "nativeInstalledReleaseAccepted": False,
        "gatePromotionAuthority": "none_read_only_snapshot",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        sha = sha_check(args.sha)
        report = assess(sha, get_runs(sha), get_jobs)
        # GitHub observation is external data, not acceptance or a credential.
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temp = args.output.with_name(args.output.name + ".tmp")
        temp.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n",
                        encoding="utf-8")
        temp.replace(args.output)
    except (EvidenceError, OSError, TypeError, KeyError) as exc:
        # Never write a fake or stale PASS report when GitHub is unavailable.
        print("P06_LIVE_GITHUB_EVIDENCE=DENIED:" + type(exc).__name__,
              file=sys.stderr)
        return 2
    print("P06_LIVE_GITHUB_EVIDENCE=OBSERVED_NOT_RELEASE_AUTHORIZATION")
    for row in report["observations"]:
        print("P06_GATE_" + row["gate"] + "=" + row["state"])
    print("P06_NATIVE_PROTECTED_AND_RELEASE_ADMISSION=NOT_APPROVED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
