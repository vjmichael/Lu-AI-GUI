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
# Frozen 63-workflow parent cohort with exact workflow/job identities, including
# both DEV-01 installed jobs and the independently failing B4/B5/WO9 gates.
# The catalog is review-controlled source, not inferred from whatever is green.
CATALOG = json.loads(Path(__file__).with_name("p06-observer-cohort.json").read_text(encoding="utf-8"))
GATE_RECORDS = {row["gate"]: row for row in CATALOG["workflows"]}
GATES = tuple((row["gate"], row["workflow"], tuple(job["name"] for job in row["jobs"]),
               row["jobs"][0]["scope"] if len({job["scope"] for job in row["jobs"]}) == 1 else "mixed_hosted")
              for row in CATALOG["workflows"])


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
    # Do not follow redirects with a repository Actions read token; a new
    # domain or path must never receive the Authorization header.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, request, fp, code, msg, headers, newurl):
            raise EvidenceError("github_api_redirect_denied")

    opener = urllib.request.build_opener(NoRedirect)
    try:
        with opener.open(request, timeout=15) as response:
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


def get_jobs(run_id, attempt):
    if any(isinstance(v, bool) or not isinstance(v, int) or v <= 0 for v in (run_id, attempt)):
        raise EvidenceError("invalid_run_or_attempt_id")
    return _all_pages(f"/repos/{REPO}/actions/runs/{run_id}/attempts/{attempt}/jobs", "jobs")


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
    if not all(isinstance(x, int) and not isinstance(x, bool) and x > 0
               for x in (attempt, number)):
        raise EvidenceError("invalid_actions_run_attempt")
    created = run.get("created_at")
    ident = run.get("id")
    if not isinstance(created, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", created):
        raise EvidenceError("invalid_actions_run_timestamp")
    if type(ident) is not int or ident <= 0:
        raise EvidenceError("invalid_actions_run_id")
    return (created, number, attempt, ident)


def assess(sha, runs, jobs_for_run):
    """Pure classifier; exact run and attempt, exact jobs, no promotion authority."""
    sha_check(sha)
    if not isinstance(runs, list):
        raise EvidenceError("invalid_run_list")
    if (len(GATES) != 63 or len(GATE_RECORDS) != len(GATES)
            or len({g[1] for g in GATES}) != len(GATES)):
        raise EvidenceError("invalid_gate_catalog")
    candidates_by_path = {}
    seen_runs = set()
    for run in runs:
        if not isinstance(run, dict):
            raise EvidenceError("invalid_run")
        path = run.get("path")
        if not isinstance(path, str) or not _eligible(run, sha, path):
            continue
        _rank(run)
        if run["id"] in seen_runs:
            raise EvidenceError("duplicate_run_id")
        seen_runs.add(run["id"])
        candidates_by_path.setdefault(path, []).append(run)
    observations = []
    for gate_id, workflow, required_jobs, kind in GATES:
        spec = GATE_RECORDS[gate_id]
        if not required_jobs or len(set(required_jobs)) != len(required_jobs):
            raise EvidenceError("invalid_required_jobs")
        excluded = spec.get("excludedJobNames", [])
        if (not isinstance(excluded, list)
                or any(not isinstance(name, str) or not name for name in excluded)
                or len(set(excluded)) != len(excluded)
                or set(excluded).intersection(required_jobs)
                or bool(excluded) != spec["nonIdeJobsExcluded"]
                or (excluded and workflow != ".github/workflows/lu-repo01-full-source.yml")):
            raise EvidenceError("invalid_excluded_job_catalog")
        excluded = set(excluded)
        candidates = candidates_by_path.get(workflow, [])
        if not candidates:
            observations.append({"gate": gate_id, "kind": kind, "workflow": workflow,
                                 "state": "missing", "runId": None, "runAttempt": None,
                                 "jobsObserved": 0, "requiredJobs": list(required_jobs),
                                 "jobObservations": [{"name": j["name"], "scope": j["scope"], "state": "missing"} for j in spec["jobs"]],
                                 "additionalJobs": []})
            continue
        run = max(candidates, key=_rank)
        state = "pending"
        jobs = jobs_for_run(run["id"], run["run_attempt"])
        if not isinstance(jobs, list):
            raise EvidenceError("invalid_job_list")
        required = set(required_jobs)
        matched = {}
        names_seen, ids_seen = set(), set()
        extras = []
        invalid = None
        for job in jobs:
            if not isinstance(job, dict):
                raise EvidenceError("invalid_job")
            name, ident = job.get("name"), job.get("id")
            if not isinstance(name, str) or not name or type(ident) is not int or ident <= 0:
                invalid = "invalid_job_identity"; break
            if ident in ids_seen or name in names_seen:
                invalid = "invalid_duplicate_job"; break
            names_seen.add(name); ids_seen.add(ident)
            if job.get("head_sha") != sha:
                invalid = "invalid_job_sha"; break
            if job.get("run_id") != run["id"] or type(job.get("run_attempt")) is not int or job.get("run_attempt") != run["run_attempt"]:
                invalid = "invalid_job_attempt"; break
            if name in required:
                matched[name] = job
            else:
                extras.append({"name": name, "status": job.get("status"), "conclusion": job.get("conclusion"),
                               "disposition": "non_ide_job_excluded" if name in excluded else "unreviewed_job"})
        if invalid:
            state = invalid
        elif run.get("status") == "completed":
            if run.get("conclusion") != "success":
                state = "failed_or_cancelled"
            elif set(matched) != required:
                state = "incomplete_jobs"
            elif any(row["disposition"] == "unreviewed_job" for row in extras):
                state = "unreviewed_additional_jobs"
            elif all(j.get("status") == "completed" and j.get("conclusion") == "success" for j in matched.values()):
                state = "observed_success"
            else:
                state = "failed_or_incomplete_jobs"
        elif run.get("status") not in ("queued", "in_progress", "requested", "waiting", "pending"):
            state = "invalid_run_state"
        detail = []
        for spec_job in spec["jobs"]:
            j = matched.get(spec_job["name"])
            job_state = ("missing" if j is None else "invalid_run_evidence" if invalid else
                         "observed_success" if j.get("status") == "completed" and j.get("conclusion") == "success" else
                         "pending" if j.get("status") in ("queued", "in_progress", "waiting", "pending", "requested") else
                         "failed_or_incomplete")
            detail.append({"name": spec_job["name"], "scope": spec_job["scope"], "state": job_state,
                           "jobId": j.get("id") if j else None, "status": j.get("status") if j else None,
                           "conclusion": j.get("conclusion") if j else None})
        observations.append({"gate": gate_id, "kind": kind, "workflow": workflow,
                             "state": state, "runId": run["id"], "runAttempt": run["run_attempt"],
                             "jobsObserved": len(matched), "requiredJobs": list(required_jobs),
                             "jobObservations": detail, "additionalJobs": extras})
    unreviewed = []
    for path, candidates in sorted(candidates_by_path.items()):
        if path in {g[1] for g in GATES}:
            continue
        r = max(candidates, key=_rank)
        unreviewed.append({"workflow": path, "runId": r["id"], "runAttempt": r["run_attempt"],
                           "status": r.get("status"), "conclusion": r.get("conclusion"),
                           "ideCandidate": path.startswith(".github/workflows/ide-")})
    scope_summary = {}
    for row in observations:
        for j in row["jobObservations"]:
            counts = scope_summary.setdefault(j["scope"], {"required": 0, "observedSuccess": 0, "notSuccessful": 0})
            counts["required"] += 1
            # A successful job within an invalid/missing/failed run is recorded individually but cannot promote an entire gate.
            counts["observedSuccess" if j["state"] == "observed_success" else "notSuccessful"] += 1
    return {"schema": SCHEMA, "repository": REPO, "sourceSha": sha,
            "source": "github_actions_read_only_public_api", "catalogReferenceHead": CATALOG["referenceHead"],
            "observations": observations, "unreviewedWorkflows": unreviewed,
            "scopeSummary": scope_summary,
            "allObservedGatesSuccessful": all(row["state"] == "observed_success" for row in observations)
                                          and not any(r["ideCandidate"] for r in unreviewed),
            "releaseApproved": False, "signingIdentityReviewed": False,
            "licenseLegalReviewApproved": False, "protectedExecutionAccepted": False,
            "nativeInstalledReleaseAccepted": False, "gatePromotionAuthority": "none_read_only_snapshot"}


def run_snapshot_identity(runs):
    # Require a stable latest-run/attempt/status snapshot before publishing.
    return sorted((r.get("id"), r.get("run_attempt"), r.get("path"), r.get("status"), r.get("conclusion"))
                  for r in runs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        sha = sha_check(args.sha)
        runs = get_runs(sha)
        report = assess(sha, runs, get_jobs)
        if run_snapshot_identity(runs) != run_snapshot_identity(get_runs(sha)):
            raise EvidenceError("github_api_changed_during_snapshot")
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
