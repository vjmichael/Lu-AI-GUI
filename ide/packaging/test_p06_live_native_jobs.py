"""P06 live Actions evidence classifier; hostile/no-network source regressions."""
import copy
import unittest
from pathlib import Path
from unittest.mock import patch

from p06_live_native_jobs import (
    GATES, REPO, SCHEMA, EvidenceError, _all_pages, _eligible,
    assess, sha_check,
)

SHA = "a" * 40
OTHER = "b" * 40


def run(workflow, *, ident=100, head=SHA, status="completed",
        conclusion="success", created="2026-11-08T00:00:00Z",
        repo=REPO, head_repo=REPO):
    return {
        "id": ident, "head_sha": head, "path": workflow,
        "repository": {"full_name": repo},
        "head_repository": {"full_name": head_repo},
        "event": "push", "created_at": created,
        "run_number": ident, "run_attempt": 1,
        "status": status, "conclusion": conclusion,
    }


def good_jobs(gate, sha=SHA):
    return [{
        "id": 10000 + index, "name": name, "head_sha": sha,
        "status": "completed", "conclusion": "success",
    } for index, name in enumerate(gate[2])]


class P06NativeEvidenceTests(unittest.TestCase):
    def test_missing_is_not_inferred_from_green_source_data(self):
        report = assess(SHA, [], lambda _: self.fail("network for absent run"))
        self.assertEqual(report["schema"], SCHEMA)
        self.assertFalse(report["allObservedGatesSuccessful"])
        self.assertEqual(len(report["observations"]), len(GATES))
        self.assertTrue(all(x["state"] == "missing" for x in report["observations"]))
        self.assertFalse(report["releaseApproved"])
        self.assertFalse(report["signingIdentityReviewed"])
        self.assertFalse(report["licenseLegalReviewApproved"])
        self.assertFalse(report["protectedExecutionAccepted"])
        self.assertFalse(report["nativeInstalledReleaseAccepted"])
        self.assertEqual(report["gatePromotionAuthority"], "none_read_only_snapshot")

    def test_complete_source_and_native_jobs_still_cannot_issue_release(self):
        runs = [run(g[1], ident=100+i) for i, g in enumerate(GATES)]
        jobs = {r["id"]: good_jobs(g) for r, g in zip(runs, GATES)}
        report = assess(SHA, runs, lambda ident: jobs[ident])
        self.assertTrue(report["allObservedGatesSuccessful"])
        self.assertTrue(all(row["state"] == "observed_success"
                            for row in report["observations"]))
        self.assertFalse(report["releaseApproved"])
        self.assertFalse(report["protectedExecutionAccepted"])
        self.assertFalse(report["nativeInstalledReleaseAccepted"])

    def test_older_green_cannot_rescue_newer_failed_run(self):
        gate = GATES[1]
        old = run(gate[1], ident=23, created="2026-11-07T10:00:00Z")
        new = run(gate[1], ident=24, conclusion="failure",
                  created="2026-11-08T10:00:00Z")
        report = assess(SHA, [old, new], lambda ident: good_jobs(gate))
        phase = next(x for x in report["observations"] if x["gate"] == gate[0])
        self.assertEqual(phase["runId"], 24)
        self.assertEqual(phase["state"], "failed_or_cancelled")
        self.assertFalse(report["allObservedGatesSuccessful"])

    def test_cancelled_or_incomplete_does_not_promote(self):
        gate = GATES[1]
        for status, conclusion, expected in [
            ("queued", None, "pending"),
            ("in_progress", None, "pending"),
            ("completed", "cancelled", "failed_or_cancelled"),
            ("completed", "success", "incomplete_jobs"),
        ]:
            with self.subTest(status=status, conclusion=conclusion,
                              expected=expected):
                jobs = good_jobs(gate)
                if expected == "incomplete_jobs":
                    jobs.pop()
                result = assess(SHA, [run(gate[1], status=status,
                                           conclusion=conclusion)],
                                lambda _: jobs)
                self.assertEqual(result["observations"][1]["state"], expected)

    def test_green_job_but_missing_os_matrix_member_is_incomplete(self):
        gate = GATES[0]
        report = assess(SHA, [run(gate[1])],
                        lambda _: good_jobs(gate)[:1])
        self.assertEqual(report["observations"][0]["state"], "incomplete_jobs")

    def test_duplicate_job_name_and_wrong_job_sha_are_denied(self):
        gate = GATES[1]
        jobs = good_jobs(gate)
        with_duplicate = jobs + [copy.deepcopy(jobs[0])]
        self.assertEqual(assess(SHA, [run(gate[1])],
                                lambda _: with_duplicate)["observations"][1]["state"],
                         "invalid_duplicate_job")
        jobs[0]["head_sha"] = OTHER
        self.assertEqual(assess(SHA, [run(gate[1])],
                                lambda _: jobs)["observations"][1]["state"],
                         "invalid_job_sha")

    def test_foreign_or_stale_run_is_not_mistaken_for_owned_exact_head(self):
        gate = GATES[1]
        candidates = [
            run(gate[1], head=OTHER),
            run(gate[1], repo="attacker/fork"),
            run(gate[1], head_repo="attacker/fork"),
            run("attacker.yml"),
        ]
        result = assess(SHA, candidates,
                        lambda _: self.fail("foreign or stale job trusted"))
        self.assertEqual(result["observations"][1]["state"], "missing")

    def test_empty_or_tampered_commit_rejected(self):
        for sha in ("", "A" * 40, "a" * 39, "x" * 40, None, 1):
            with self.subTest(sha=sha), self.assertRaises(EvidenceError):
                sha_check(sha)

    def test_malformed_run_attempt_denies_untrusted_github_payload(self):
        gate = GATES[1]
        item = run(gate[1])
        item["run_attempt"] = True
        with self.assertRaisesRegex(EvidenceError, "invalid_actions_run_attempt"):
            assess(SHA, [item], lambda _: good_jobs(gate))

    def test_live_api_pagination_rejects_stale_or_forged_counts(self):
        resource = "/repos/" + REPO + "/actions/runs?head_sha=" + SHA
        with patch("p06_live_native_jobs._public_github_json",
                   return_value={"total_count": 2, "workflow_runs": [
                       {"id": 1}, {"id": 2},
                   ]}):
            self.assertEqual(len(_all_pages(resource, "workflow_runs")), 2)
        with patch("p06_live_native_jobs._public_github_json",
                   return_value={"total_count": 2, "workflow_runs": [
                       {"id": 1}, {"id": 1},
                   ]}):
            with self.assertRaisesRegex(EvidenceError, "duplicate"):
                _all_pages(resource, "workflow_runs")
        with patch("p06_live_native_jobs._public_github_json",
                   return_value={"total_count": 2, "workflow_runs": []}):
            with self.assertRaisesRegex(EvidenceError, "truncated"):
                _all_pages(resource, "workflow_runs")

    def test_catalog_matches_real_required_windows_linux_workflow_sources(self):
        root = Path(__file__).resolve().parents[2]
        self.assertEqual(len(GATES), 5)
        for gate_id, workflow, jobs, kind in GATES:
            with self.subTest(gate=gate_id):
                self.assertTrue(workflow.startswith(".github/workflows/ide-"))
                source = (root / workflow).read_text(encoding="utf-8")
                self.assertTrue(source.startswith("name: IDE "))
                self.assertIn("\njobs:\n", source)
                for job in jobs:
                    # GitHub appends matrix OS in parentheses. The workflow
                    # source must still declare the exact underlying job key.
                    base_job = job.split(" (", 1)[0]
                    self.assertIn("\n  " + base_job + ":\n", source)
                    if " (ubuntu-24.04)" in job:
                        self.assertIn("ubuntu-24.04", source)
                    if " (windows-2022)" in job:
                        self.assertIn("windows-2022", source)
                self.assertIn(kind, (
                    "native_source", "protected_native", "unsigned_installed",
                    "source_dependency",
                ))

    def test_runner_origin_is_pinned_to_exact_repo(self):
        gate = GATES[2]
        sample = run(gate[1])
        self.assertTrue(_eligible(sample, SHA, gate[1]))
        for field in ("repository", "head_repository"):
            tampered = copy.deepcopy(sample)
            tampered[field] = {"full_name": "other/owner"}
            self.assertFalse(_eligible(tampered, SHA, gate[1]))


if __name__ == "__main__":
    unittest.main()
