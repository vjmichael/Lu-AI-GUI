"""Hostile pure-data P05 owner successor tests; no native acceptance claim."""
import copy
import json
from pathlib import Path
import unittest
import subprocess
from verify_p05_owner_successor import (
    EXPECTED, SCOPED_SOURCE_ONLY, Denied, compare_38_unchanged_and_replacements,
    compare_38_with_separate_scoped_source_dispositions,
    verify_source_only_patch, project_reviewed_wo6_current_source,
    WO6_PATH, WO6_PRIOR_OWNER_BLOB, WO6_CURRENT_SOURCE_BLOB,
    WO6_REVIEWED_TRANSFER_BLOB, WO6_ROOT_CHECK_PRIOR, WO6_ROOT_CHECK_CURRENT, original_blob_sha,
    WO6_ROOT_REVIEWED_BLOB, WO6_INTERRUPTED_BUILD_PRIOR, WO6_INTERRUPTED_BUILD_CURRENT,
    WO6_INTERRUPTED_REVIEWED_BLOB, WO6_TIMEOUT_PRIOR, WO6_TIMEOUT_CURRENT,
    WO6_INTERRUPTED_MODE_PRIOR, WO6_INTERRUPTED_MODE_CURRENT,
    CLANGD_PATH, CLANGD_PRIOR_BLOB, CLANGD_CURRENT_BLOB, project_checked_out_clangd_source,
    E2_SMOKE_PATH, E2_SMOKE_PRIOR_BLOB, E2_SMOKE_CURRENT_BLOB, project_checked_out_e2_smoke_source,
    P05_04J_DELTAS, project_reviewed_04j_workflows, project_checked_out_04j_source,
    P03_R12_PATH, P03_R12_PRIOR, P03_R12_CURRENT, P03_R12_ADDED_STEP,
    P03_R12_BUILD_CURRENT, P03_R12_BUILD_PRIOR,
    project_reviewed_p03_source, project_checked_out_p03_source,
)

class OriginalAndOwnerSuccessor(unittest.TestCase):
    def setUp(self):
        self.base={p:values[0] for p,values in EXPECTED.items()}
        self.base.update({f".github/workflows/ide-original-{n:02d}.yml":f"{n:040x}" for n in range(30)})
        self.now=dict(self.base)
        self.now.update({p:values[1] for p,values in EXPECTED.items()})
    def test_owner_successor_not_native_evidence(self):
        result=compare_38_unchanged_and_replacements(self.base,self.now)
        self.assertEqual(result["historicalExactWorkflows"],38)
        self.assertEqual(result["ownerAuthorizedCurrentReplacements"],8)
        self.assertFalse(result["nativeAccepted"])
        self.assertFalse(result["releaseApproved"])
    def test_unknown_ninth_workflow_source_change_rejected(self):
        self.now[".github/workflows/ide-original-01.yml"]="f"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_missing_workflow_denied(self):
        self.now.pop(".github/workflows/ide-original-03.yml")
        with self.assertRaisesRegex(Denied,"p05_expected_exact_38"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_owner_authorized_source_identity_change_denied(self):
        p=next(iter(EXPECTED))
        self.now[p]="0"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)
    def test_historical_pin_mutation_denied(self):
        p=next(iter(EXPECTED))
        self.base[p]="f"*40
        with self.assertRaisesRegex(Denied,"p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base,self.now)


class FiveAdditionalSourceOnlyTests(unittest.TestCase):
    def setUp(self):
        root = Path(__file__).resolve().parents[2]
        inventory = json.loads(
            (root / "ide/packaging/p05-workflow-coverage.json").read_text(encoding="utf-8")
        )
        self.base = {row["path"]: row["blobSha"] for row in inventory["requiredWorkflows"]}
        self.now = dict(self.base)
        self.now.update({path: values[1] for path, values in EXPECTED.items()})
        self.now.update({path: values[1] for path, values in SCOPED_SOURCE_ONLY.items()})
        self.record = json.loads(
            (root / "ide/audits/p05_additional_current_source_dispositions.json").read_text(encoding="utf-8")
        )

    def test_exact_five_source_dispositions_are_not_historical_approvals(self):
        result = compare_38_with_separate_scoped_source_dispositions(
            self.base, self.now, self.record
        )
        self.assertEqual(result["historicalExactWorkflows"], 38)
        self.assertEqual(result["unchangedCurrentWorkflows"], 25)
        self.assertEqual(result["ownerAuthorizedHistoricalReplacements"], 8)
        self.assertEqual(result["separatelyReconciledSourceOnlyDifferences"], 5)
        self.assertFalse(result["historicalApprovalRewritten"])
        self.assertFalse(result["nativeAccepted"])
        self.assertFalse(result["releaseApproved"])

    def test_original_historical_eight_only_verifier_stays_strict(self):
        with self.assertRaisesRegex(Denied, "p05_unreviewed_or_unretained"):
            compare_38_unchanged_and_replacements(self.base, self.now)

    def test_unreviewed_sixth_source_delta_denied(self):
        path = next(p for p in self.base if p not in EXPECTED and p not in SCOPED_SOURCE_ONLY)
        self.now[path] = "f" * 40
        with self.assertRaisesRegex(Denied, "p05_unreviewed_or_unretained"):
            compare_38_with_separate_scoped_source_dispositions(self.base, self.now, self.record)

    def test_changed_source_only_blob_denied(self):
        path = next(iter(SCOPED_SOURCE_ONLY))
        self.now[path] = "0" * 40
        with self.assertRaisesRegex(Denied, "p05_unreviewed_or_unretained"):
            compare_38_with_separate_scoped_source_dispositions(self.base, self.now, self.record)

    def test_historical_source_pin_rewrite_denied(self):
        path = next(iter(SCOPED_SOURCE_ONLY))
        self.base[path] = SCOPED_SOURCE_ONLY[path][1]
        with self.assertRaisesRegex(Denied, "p05_unreviewed_or_unretained"):
            compare_38_with_separate_scoped_source_dispositions(self.base, self.now, self.record)

    def test_missing_source_row_and_duplicate_denied(self):
        for modify in ("remove", "duplicate"):
            review = copy.deepcopy(self.record)
            if modify == "remove":
                review["workflows"].pop()
            else:
                review["workflows"][-1] = review["workflows"][0]
            with self.subTest(modify=modify):
                with self.assertRaisesRegex(Denied, "p05_source_disposition"):
                    compare_38_with_separate_scoped_source_dispositions(self.base, self.now, review)

    def test_no_false_owner_approval_or_native_release_claim(self):
        for key, value in (
            ("priorOwnerDecisionModified", True),
            ("nativeWindowsAccepted", True),
            ("releaseApproved", True),
            ("originalNativeNegativeTestsRetained", False),
        ):
            review = copy.deepcopy(self.record)
            review[key] = value
            with self.subTest(field=key):
                with self.assertRaisesRegex(Denied, "p05_source_disposition"):
                    compare_38_with_separate_scoped_source_dispositions(self.base, self.now, review)
        review = copy.deepcopy(self.record)
        review["workflows"][0]["historicalOwnerAuthorization"] = True
        with self.assertRaisesRegex(Denied, "p05_source_disposition"):
            compare_38_with_separate_scoped_source_dispositions(self.base, self.now, review)

    def test_original_trigger_or_job_removal_fails_exact_source_normalization(self):
        path = ".github/workflows/ide-wo8-installed-linux.yml"
        original = (
            "on:\n  pull_request:\n    paths: ['ide/app/**']\njobs:\n"
            "  linux:\n    steps:\n      - name: Original installed test\n"
            "        run: |\n"
            "          python ../packaging/verify_p06_candidate_lock.py --repo-root ../.. --platform ubuntu-24.04\n"
            "          python ../packaging/verify_p06_candidate_lock.py --repo-root ../.. --platform ubuntu-24.04\n"
        )
        current = original.replace(
            " --platform ubuntu-24.04\n",
            " --platform ubuntu-24.04 --ide-merge02-successor\n",
        )
        self.assertTrue(verify_source_only_patch(path, original, current))
        bad_trigger = current.replace("  pull_request:", "  push:")
        with self.assertRaisesRegex(Denied, "p05_source_only_patch_expands"):
            verify_source_only_patch(path, original, bad_trigger)
        bad_test = current.replace("      - name: Original installed test\n", "")
        with self.assertRaisesRegex(Denied, "p05_source_only_original_named_step_removed"):
            verify_source_only_patch(path, original, bad_test)

class ReviewedWo6CurrentSourceTests(unittest.TestCase):
    def test_exact_successor_delta_not_an_owner_grant(self):
        root = Path(__file__).resolve().parents[2]
        def get_blob(sha):
            return subprocess.run(
                ["git", "-C", str(root), "cat-file", "blob", sha],
                check=True, capture_output=True, timeout=20,
            ).stdout
        prior = get_blob(WO6_PRIOR_OWNER_BLOB)
        current = get_blob(WO6_CURRENT_SOURCE_BLOB)
        observed = {WO6_PATH: WO6_CURRENT_SOURCE_BLOB}
        projected = project_reviewed_wo6_current_source(observed, prior, current)
        self.assertEqual(projected[WO6_PATH], WO6_PRIOR_OWNER_BLOB)
        self.assertEqual(observed[WO6_PATH], WO6_CURRENT_SOURCE_BLOB)
        # The prior transfer-only version remains immutable but is no longer
        # the admitted current workflow. Removing the ownership check or adding
        # any unrelated tool/permission must fail closed.
        self.assertEqual(current.count(WO6_ROOT_CHECK_CURRENT), 1)
        interrupted_reviewed = current.replace(WO6_TIMEOUT_CURRENT, WO6_TIMEOUT_PRIOR, 1)
        self.assertEqual(original_blob_sha(interrupted_reviewed), WO6_INTERRUPTED_REVIEWED_BLOB)
        self.assertEqual(interrupted_reviewed, get_blob(WO6_INTERRUPTED_REVIEWED_BLOB))
        root_reviewed = interrupted_reviewed.replace(WO6_INTERRUPTED_BUILD_CURRENT, WO6_INTERRUPTED_BUILD_PRIOR, 1).replace(
            WO6_INTERRUPTED_MODE_CURRENT, WO6_INTERRUPTED_MODE_PRIOR, 1)
        self.assertEqual(original_blob_sha(root_reviewed), WO6_ROOT_REVIEWED_BLOB)
        self.assertEqual(root_reviewed, get_blob(WO6_ROOT_REVIEWED_BLOB))
        old = root_reviewed.replace(WO6_ROOT_CHECK_CURRENT, WO6_ROOT_CHECK_PRIOR, 1)
        self.assertEqual(original_blob_sha(old), WO6_REVIEWED_TRANSFER_BLOB)
        self.assertEqual(old, get_blob(WO6_REVIEWED_TRANSFER_BLOB))
        for wrong in (current + b"\n", current.replace(b"actions: read", b"actions: write"),
                      old, root_reviewed, interrupted_reviewed,
                      current.replace(WO6_TIMEOUT_CURRENT, WO6_TIMEOUT_PRIOR),
                      current.replace(WO6_INTERRUPTED_BUILD_CURRENT, WO6_INTERRUPTED_BUILD_PRIOR),
                      current.replace(WO6_INTERRUPTED_MODE_CURRENT, WO6_INTERRUPTED_MODE_PRIOR),
                      current.replace(b"/usr/bin/rustdoc; do", b"/usr/bin/rustdoc /usr/bin/python3; do"),
                      current.replace(b'test "$(stat -c %u "$file")" = 0', b'true')):
            with self.assertRaisesRegex(Denied, "p05_wo6_current_source_blob_not_exact"):
                project_reviewed_wo6_current_source(observed, prior, wrong)
        with self.assertRaisesRegex(Denied, "p05_wo6_current_source_blob_not_exact"):
            project_reviewed_wo6_current_source(
                {WO6_PATH: "f" * 40}, prior, current
            )


class ClangdCurrentSourceTests(unittest.TestCase):
    def test_exact_successor_projection_without_native_approval(self):
        root = Path(__file__).resolve().parents[2]
        observed = {CLANGD_PATH: CLANGD_CURRENT_BLOB}
        projected, previous = project_checked_out_clangd_source(root, observed)
        self.assertEqual(projected[CLANGD_PATH], CLANGD_PRIOR_BLOB)
        self.assertEqual(observed[CLANGD_PATH], CLANGD_CURRENT_BLOB)
        self.assertIn("Get-Command clangd.exe", previous)
        with self.assertRaisesRegex(Denied, "p05_clangd_current_source_blob_not_exact"):
            project_checked_out_clangd_source(root, {CLANGD_PATH: "f" * 40})


class E2SmokeCurrentSourceTests(unittest.TestCase):
    def test_exact_lock_successor_projection(self):
        root = Path(__file__).resolve().parents[2]
        observed = {E2_SMOKE_PATH: E2_SMOKE_CURRENT_BLOB}
        projected, previous = project_checked_out_e2_smoke_source(root, observed)
        self.assertEqual(projected[E2_SMOKE_PATH], E2_SMOKE_PRIOR_BLOB)
        self.assertEqual(observed[E2_SMOKE_PATH], E2_SMOKE_CURRENT_BLOB)
        self.assertEqual(previous.count("--ide-merge02-successor"), 0)
        with self.assertRaisesRegex(Denied, "p05_e2_smoke_current_source_not_exact"):
            project_checked_out_e2_smoke_source(root, {E2_SMOKE_PATH: "f" * 40})


class Reviewed04JWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.observed = {path: delta[1] for path, delta in P05_04J_DELTAS.items()}
        self.expected = {path: delta[0] for path, delta in P05_04J_DELTAS.items()}

    def read_blob(self, sha):
        return subprocess.run(["git", "-C", str(self.root), "cat-file", "blob", sha],
                              check=True, timeout=20, capture_output=True).stdout

    def test_exact_two_prior_pins_and_original_test_commands(self):
        projected, prior = project_checked_out_04j_source(self.root, self.observed)
        self.assertEqual(projected, self.expected)
        self.assertEqual(set(prior), set(self.expected))
        self.assertEqual(self.observed, {p:d[1] for p,d in P05_04J_DELTAS.items()})
        self.assertIn("python3 ide/app/fixtures/test_atspi_signature_contract.py", prior[next(iter(prior))])
        self.assertIn("test \"$(find \"$exportdir\" -name '*.zip' -type f | wc -l)\" -eq 1", prior[list(prior)[1]])

    def test_arbitrary_blob_and_extra_workflow_delta_are_denied(self):
        for path in self.observed:
            bad = dict(self.observed); bad[path] = "f" * 40
            with self.assertRaisesRegex(Denied, "p05_04j_current_workflow_not_exact"):
                project_reviewed_04j_workflows(bad, self.read_blob)
            with self.assertRaisesRegex(Denied, "p05_04j_workflow_delta_not_exact"):
                project_reviewed_04j_workflows(self.observed, lambda sha: self.read_blob(sha) + b"\n" if sha == self.observed[path] else self.read_blob(sha))
        missing = dict(self.observed); missing.pop(next(iter(missing)))
        with self.assertRaisesRegex(Denied, "p05_04j_current_workflow_not_exact"):
            project_reviewed_04j_workflows(missing, self.read_blob)


class P03R12ProjectionTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.blobs = {P03_R12_PATH: P03_R12_CURRENT}
        def read(sha):
            return subprocess.run(["git", "-C", str(self.root), "cat-file", "blob", sha],
                                  check=True, timeout=20, capture_output=True).stdout
        self.prior, self.current = read(P03_R12_PRIOR), read(P03_R12_CURRENT)

    def test_exact_additive_source_restoration(self):
        result = project_checked_out_p03_source(self.root, self.blobs)
        self.assertEqual(result, {P03_R12_PATH: P03_R12_PRIOR})
        self.assertEqual(self.blobs, {P03_R12_PATH: P03_R12_CURRENT})
        self.assertEqual(self.current.replace(P03_R12_ADDED_STEP, b"", 1).replace(
            P03_R12_BUILD_CURRENT, P03_R12_BUILD_PRIOR, 1), self.prior)

    def test_mutated_trigger_permission_and_tests_denied(self):
        for bad in (self.current+b"\n", self.current.replace(b"pull_request:", b"push:", 1),
                    self.current.replace(P03_R12_ADDED_STEP, b"", 1),
                    self.current.replace(P03_R12_BUILD_CURRENT, P03_R12_BUILD_PRIOR, 1)):
            with self.subTest(blob=original_blob_sha(bad)), self.assertRaises(Denied):
                project_reviewed_p03_source(self.blobs, self.prior, bad)
        with self.assertRaises(Denied):
            project_reviewed_p03_source({P03_R12_PATH: P03_R12_PRIOR}, self.prior, self.current)
        with self.assertRaises(Denied):
            project_reviewed_p03_source(self.blobs, self.prior+b"\n", self.current)


if __name__=="__main__":
    unittest.main()
