"""P06 live Actions evidence classifier; hostile/no-network source regressions."""
import copy
import unittest
from pathlib import Path
from unittest.mock import patch

from p06_live_native_jobs import (
    GATES, GATE_RECORDS, REPO, SCHEMA, EvidenceError, _all_pages, _eligible, get_jobs, run_snapshot_identity,
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


def good_jobs(gate, sha=SHA, ident=100, attempt=1):
    return [{
        "id": 10000 + index, "name": name, "head_sha": sha,
        "run_id": ident, "run_attempt": attempt,
        "status": "completed", "conclusion": "success",
    } for index, name in enumerate(gate[2])]


class P06NativeEvidenceTests(unittest.TestCase):
    def test_missing_is_not_inferred_from_green_source_data(self):
        report = assess(SHA, [], lambda *_: self.fail("network for absent run"))
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
        jobs = {r["id"]: good_jobs(g, ident=r["id"]) for r, g in zip(runs, GATES)}
        report = assess(SHA, runs, lambda ident, attempt: jobs[ident])
        self.assertTrue(report["allObservedGatesSuccessful"])
        self.assertTrue(all(row["state"] == "observed_success"
                            for row in report["observations"]))
        self.assertFalse(report["releaseApproved"])
        self.assertFalse(report["protectedExecutionAccepted"])
        self.assertFalse(report["nativeInstalledReleaseAccepted"])

    def test_older_green_cannot_rescue_newer_failed_run(self):
        gate = GATES[0]
        old = run(gate[1], ident=23, created="2026-11-07T10:00:00Z")
        new = run(gate[1], ident=24, conclusion="failure",
                  created="2026-11-08T10:00:00Z")
        report = assess(SHA, [old, new], lambda ident, attempt: good_jobs(gate, ident=ident, attempt=attempt))
        phase = next(x for x in report["observations"] if x["gate"] == gate[0])
        self.assertEqual(phase["runId"], 24)
        self.assertEqual(phase["state"], "failed_or_cancelled")
        self.assertFalse(report["allObservedGatesSuccessful"])

    def test_cancelled_or_incomplete_does_not_promote(self):
        gate = GATES[0]
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
                                lambda *_: jobs)
                self.assertEqual(result["observations"][0]["state"], expected)

    def test_green_job_but_missing_os_matrix_member_is_incomplete(self):
        gate = GATES[0]
        report = assess(SHA, [run(gate[1])],
                        lambda *_: good_jobs(gate)[:1])
        self.assertEqual(report["observations"][0]["state"], "incomplete_jobs")

    def test_duplicate_job_name_and_wrong_job_sha_are_denied(self):
        gate = GATES[0]
        jobs = good_jobs(gate)
        with_duplicate = jobs + [copy.deepcopy(jobs[0])]
        self.assertEqual(assess(SHA, [run(gate[1])],
                                lambda *_: with_duplicate)["observations"][0]["state"],
                         "invalid_duplicate_job")
        jobs[0]["head_sha"] = OTHER
        self.assertEqual(assess(SHA, [run(gate[1])],
                                lambda *_: jobs)["observations"][0]["state"],
                         "invalid_job_sha")

    def test_foreign_or_stale_run_is_not_mistaken_for_owned_exact_head(self):
        gate = GATES[0]
        candidates = [
            run(gate[1], head=OTHER),
            run(gate[1], repo="attacker/fork"),
            run(gate[1], head_repo="attacker/fork"),
            run("attacker.yml"),
        ]
        result = assess(SHA, candidates,
                        lambda *_: self.fail("foreign or stale job trusted"))
        self.assertEqual(result["observations"][0]["state"], "missing")

    def test_empty_or_tampered_commit_rejected(self):
        for sha in ("", "A" * 40, "a" * 39, "x" * 40, None, 1):
            with self.subTest(sha=sha), self.assertRaises(EvidenceError):
                sha_check(sha)

    def test_malformed_run_attempt_denies_untrusted_github_payload(self):
        gate = GATES[0]
        item = run(gate[1])
        item["run_attempt"] = True
        with self.assertRaisesRegex(EvidenceError, "invalid_actions_run_attempt"):
            assess(SHA, [item], lambda *_: good_jobs(gate))

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

    def test_catalog_matches_real_consolidated_workflow_source(self):
        root = Path(__file__).resolve().parents[2]
        self.assertEqual(len(GATES), 63)
        self.assertEqual(sum(len(g[2]) for g in GATES), 139)
        gate_id, workflow, jobs, kind = GATES[0]
        self.assertEqual(gate_id, "IDE_merged_native")
        self.assertEqual(kind, "mixed_hosted")
        self.assertEqual(len(jobs), 6)
        self.assertEqual(sum(j["scope"] == "hosted_installed" for j in GATE_RECORDS[gate_id]["jobs"]), 2)
        scopes = {j["scope"] for r in GATE_RECORDS.values() for j in r["jobs"]}
        self.assertTrue({"source_identity", "hosted_installed", "protected_native_checks", "dependency_provenance"} <= scopes)
        for gid, path, jobs, _ in GATES:
            source = (root / path).read_text(encoding="utf-8")
            self.assertIn("\njobs:\n", source)
            self.assertEqual(len(set(jobs)), len(jobs))
            for job in GATE_RECORDS[gid]["jobs"]:
                self.assertIn("\n  " + job["workflowJobKey"] + ":\n", source)
        for name in ("ide-b4-installed-editor-windows", "ide-b4-split-editor-native", "ide-b5-large-project-native", "ide-wo9-installed-native"):
            self.assertIn(name, GATE_RECORDS)

    def test_green_compilers_do_not_mask_missing_installed_jobs(self):
        gate = GATES[0]
        report = assess(SHA, [run(gate[1])], lambda *_: good_jobs(gate)[:4])
        self.assertEqual(report["observations"][0]["state"], "incomplete_jobs")
        self.assertFalse(report["allObservedGatesSuccessful"])
        self.assertTrue(any(j["state"] == "missing" for j in report["observations"][0]["jobObservations"] if j["scope"] == "hosted_installed"))

    def test_attempt_specific_jobs_and_missing_sha(self):
        gate = GATES[0]
        current = run(gate[1]); current["run_attempt"] = 2
        calls = []
        def read(ident, attempt):
            calls.append((ident, attempt))
            return good_jobs(gate, attempt=attempt)
        self.assertEqual(assess(SHA, [current], read)["observations"][0]["state"], "observed_success")
        self.assertEqual(calls, [(100, 2)])
        self.assertEqual(assess(SHA, [current], lambda *_: good_jobs(gate))["observations"][0]["state"], "invalid_job_attempt")
        jobs = good_jobs(gate, attempt=2); jobs[0].pop("head_sha")
        self.assertEqual(assess(SHA, [current], lambda *_: jobs)["observations"][0]["state"], "invalid_job_sha")
        with patch("p06_live_native_jobs._all_pages", return_value=[]) as fetch:
            get_jobs(100, 2)
            fetch.assert_called_once_with(f"/repos/{REPO}/actions/runs/100/attempts/2/jobs", "jobs")

    def test_unreviewed_ide_workflow_cannot_promote_complete_cohort(self):
        runs = [run(g[1], ident=100+i) for i,g in enumerate(GATES)]
        jobs = {r["id"]: good_jobs(g, ident=r["id"]) for r,g in zip(runs,GATES)}
        runs.append(run(".github/workflows/ide-new.yml",ident=999))
        report = assess(SHA,runs,lambda ident, attempt:jobs[ident])
        self.assertFalse(report["allObservedGatesSuccessful"])
        self.assertTrue(report["unreviewedWorkflows"][0]["ideCandidate"])

    def test_new_attempt_changes_snapshot(self):
        a = run(GATES[0][1]); b = copy.deepcopy(a); b["run_attempt"] = 2
        self.assertNotEqual(run_snapshot_identity([a]), run_snapshot_identity([b]))

    def test_unreviewed_job_and_duplicate_id_fail_closed(self):
        gate = GATES[0]; jobs = good_jobs(gate)
        job = dict(jobs[0],name="unreviewed",id=99999)
        self.assertEqual(assess(SHA,[run(gate[1])],lambda *_:jobs+[job])["observations"][0]["state"],"unreviewed_additional_jobs")
        job["id"] = jobs[0]["id"]
        self.assertEqual(assess(SHA,[run(gate[1])],lambda *_:jobs+[job])["observations"][0]["state"],"invalid_duplicate_job")

    def test_runner_origin_is_pinned_to_exact_repo(self):
        gate = GATES[0]
        sample = run(gate[1])
        self.assertTrue(_eligible(sample, SHA, gate[1]))
        for field in ("repository", "head_repository"):
            tampered = copy.deepcopy(sample)
            tampered[field] = {"full_name": "other/owner"}
            self.assertFalse(_eligible(tampered, SHA, gate[1]))


if __name__ == "__main__":
    unittest.main()
