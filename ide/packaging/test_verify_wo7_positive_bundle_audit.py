"""Hostile source-only fixtures. No fake signature is accepted in production CLI."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import verify_wo7_positive_bundle_audit as gate

HEAD = "a" * 40


def jwrite(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, sort_keys=True), encoding="utf-8")
    return path


class PositiveBundleEvidenceTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name)
        self.candidate = self.base / "candidate"
        self.review = self.base / "review"
        self.stage = self.review / "staged-payload"
        self.candidate.mkdir()
        self.stage.mkdir(parents=True)
        self.legal = self.candidate / "THIRD_PARTY_LICENSES"
        self.legal.mkdir()
        self.legal_manifest = jwrite(self.legal / "inventory.json", {"sourceSha": HEAD})
        self.binary = {}
        roles = ("installer", "desktop", "service")
        manifest = {"schema": "lu.ide.wo7.signed-nsis.v1", "releaseApproved": False,
                    "sourceSha": HEAD, "licensesReviewed": True}
        for role in roles:
            filename = role + ".exe"
            f = self.candidate / filename
            f.write_bytes((role + ": test-only unsigned fixture").encode())
            manifest[role] = {"path": filename, "sha256": gate.digest(f),
                              "signerThumbprint": "b" * 40}
            self.binary[role] = gate.digest(f)
        jwrite(self.candidate / "wo7-release.manifest.json", manifest)
        rows = []
        for role in ("desktop", "service", "debugger", "debuggee"):
            f = self.stage / (role + ".exe")
            f.write_bytes((role + ": test-only unsigned fixture").encode())
            rows.append({"role": role, "destination": f.name, "sha256": gate.digest(f)})
        self.rows = rows
        jwrite(self.stage / "resources/wo7-service-policy.json", {"schema": "test-only"})
        helper = self.stage / "resources/installer/wo7-service-lifecycle.ps1"
        helper.parent.mkdir(parents=True, exist_ok=True)
        helper.write_text("# test-only lifecycle fixture", encoding="utf-8")
        self.stage_path = jwrite(self.stage / "resources/wo7-staged-tree.json",
                                 {"schema": "lu.ide.wo7.staged-tree.v1", "sourceSha": HEAD,
                                  "files": rows})
        self.static = {"staticImportClosure": True, "fullReviewedPrivateDllChain": True,
                       "dynamicImportAuditStillRequired": True, "inspected": {}}
        self.dynamic = {"schema": gate.EXPECTED_DYNAMIC, "sourceSha": HEAD,
                        "closureReviewed": True, "usesRealNativeBinaryInspection": True,
                        "reviewMethod": "Independent examination of real native imports",
                        "inspectedSha256ByDestination": {x["destination"]: x["sha256"] for x in rows}}
        self.dynamic_path = jwrite(self.review / "dynamic-import-review.json", self.dynamic)
        self.review_body = {"schema": gate.EXPECTED_REVIEW, "sourceSha": HEAD,
                            "candidateRunId": 111, "reviewRunId": 222,
                            "legalReviewApproved": True, "dynamicImportReviewApproved": True,
                            "reviewerIdentity": "independent-reviewer",
                            "licenseInventorySha256": gate.digest(self.legal_manifest),
                            "stageAttestationSha256": gate.digest(self.stage_path),
                            "dynamicReviewSha256": gate.digest(self.dynamic_path),
                            "staticImportReportSha256": hashlib.sha256(json.dumps(
                                self.static, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
                            "installerSha256": self.binary["installer"],
                            "desktopSha256": self.binary["desktop"],
                            "serviceSha256": self.binary["service"]}
        self.review_path = jwrite(self.review / "independent-review.json", self.review_body)
        self.pinned = gate.digest(self.review_path)
        self.mock_signature = lambda path: ("Valid", "b" * 40)
        self.mocks = [mock.patch.object(gate, "audit_candidate", return_value={"verifiedSignatures": [1, 2, 3]}),
                      mock.patch.object(gate, "audit_licenses", return_value={
                          "actualBytesVerified": True, "unresolved": 0,
                          "upstreamRetrievalVerified": True, "signedOrLegallyApproved": False}),
                      mock.patch.object(gate, "audit_staged_pe", return_value=self.static)]
        for obj in self.mocks:
            obj.start()
            self.addCleanup(obj.stop)

    def check(self, review_sha=None, candidate_id=111, review_id=222):
        return gate.audit(self.candidate, self.review, HEAD, review_sha or self.pinned,
                          candidate_id, review_id, "independent-reviewer",
                          signature=self.mock_signature)

    def resave_review(self):
        jwrite(self.review_path, self.review_body)
        self.pinned = gate.digest(self.review_path)

    def test_matching_synthetic_source_metadata_never_grants_release(self):
        result = self.check()
        self.assertEqual(result["actualSignedBinaryCount"], 3)
        self.assertFalse(result["releaseAccepted"])
        self.assertFalse(result["installedSCMJobAcceptance"])
        self.assertTrue(result["independentGitHubAndPhysicalWitnessReviewStillRequired"])

    def test_protected_digest_cannot_be_replaced_with_unpinned_review(self):
        self.review_body["reviewerIdentity"] = "intruder"
        jwrite(self.review_path, self.review_body)
        with self.assertRaisesRegex(gate.Denied, "independent_review_not_pinned"):
            self.check()

    def test_missing_protected_pin_fails(self):
        with self.assertRaisesRegex(gate.Denied, "reviewed_digest_missing"):
            self.check(review_sha="not-an-approved-hash")

    def test_reused_or_fake_run_ids_fail(self):
        for a, b in ((111, 111), (0, 222), (111, 0), (True, 222)):
            with self.subTest(candidate=a, review=b), self.assertRaises(gate.Denied):
                self.check(candidate_id=a, review_id=b)

    def test_wrong_source_rejected_before_loading_files(self):
        with self.assertRaisesRegex(gate.Denied, "invalid_source_sha"):
            gate.audit(self.candidate, self.review, "wrong", self.pinned, 111, 222,
                       "independent-reviewer")

    def test_independent_review_wrong_actual_run_rejected(self):
        self.review_body["reviewRunId"] = 999
        self.resave_review()
        with self.assertRaisesRegex(gate.Denied, "independent_legal_or_import_review_missing"):
            self.check()

    def test_forged_reviewer_login_rejected(self):
        self.review_body["reviewerIdentity"] = "someone-else"
        self.resave_review()
        with self.assertRaisesRegex(gate.Denied, "independent_legal_or_import_review_missing"):
            self.check()

    def test_candidate_cannot_claim_release_before_installed_acceptance(self):
        manifest = self.candidate / "wo7-release.manifest.json"
        obj = json.loads(manifest.read_text())
        obj["releaseApproved"] = True
        jwrite(manifest, obj)
        with self.assertRaisesRegex(gate.Denied, "signed_candidate_incorrectly_claimed_release"):
            self.check()

    def test_independent_legal_review_missing_rejected(self):
        self.review_body["legalReviewApproved"] = False
        self.resave_review()
        with self.assertRaisesRegex(gate.Denied, "independent_legal_or_import_review_missing"):
            self.check()

    def test_dynamic_review_must_bind_all_exact_executable_hashes(self):
        self.dynamic["inspectedSha256ByDestination"].pop("debugger.exe")
        jwrite(self.dynamic_path, self.dynamic)
        with self.assertRaisesRegex(gate.Denied, "dynamic_review_does_not_cover_exact_binary_tree"):
            self.check()

    def test_dynamic_review_cannot_claim_static_only(self):
        self.dynamic["usesRealNativeBinaryInspection"] = False
        jwrite(self.dynamic_path, self.dynamic)
        with self.assertRaisesRegex(gate.Denied, "independent_dynamic_import_review_incomplete"):
            self.check()

    def test_unlisted_private_dll_is_denied(self):
        (self.stage / "unreviewed-native.dll").write_bytes(b"unlisted")
        with self.assertRaisesRegex(gate.Denied, "staged_payload_has_unlisted_or_missing_bytes"):
            self.check()

    def test_staged_service_must_equal_signed_candidate(self):
        self.rows[1]["sha256"] = "f" * 64
        jwrite(self.stage_path, {"schema": "lu.ide.wo7.staged-tree.v1",
                                 "sourceSha": HEAD, "files": self.rows})
        with self.assertRaisesRegex(gate.Denied, "signed_candidate_stage_binary_mismatch"):
            self.check()

    def test_incomplete_upstream_legal_bytes_denied(self):
        gate.audit_licenses.return_value["unresolved"] = 4
        with self.assertRaisesRegex(gate.Denied, "technical_license_provenance_incomplete"):
            self.check()

    def test_missing_static_pe_closure_denied(self):
        self.static["staticImportClosure"] = False
        with self.assertRaisesRegex(gate.Denied, "real_static_import_closure_incomplete"):
            self.check()

    def test_external_review_sha_bound_to_actual_candidate(self):
        self.review_body["installerSha256"] = "0" * 64
        self.resave_review()
        with self.assertRaisesRegex(gate.Denied, "independent_review_digest_mismatch:installerSha256"):
            self.check()

    def test_untrusted_candidate_signer_denied(self):
        gate.audit_candidate.side_effect = gate.Denied("candidate_signer_or_chain_invalid")
        with self.assertRaisesRegex(gate.Denied, "candidate_signer_or_chain_invalid"):
            self.check()

    def test_workflow_requires_protected_pins_and_separate_run_origins(self):
        workflow = Path(__file__).resolve().parents[2] / ".github/workflows/ide-wo7-release-bundle-audit.yml"
        text = workflow.read_text(encoding="utf-8")
        self.assertIn("environment: lu-ide-release-audit", text)
        self.assertIn("vars.IDE_WO7_INDEPENDENT_REVIEW_SHA256", text)
        self.assertIn("vars.IDE_WO7_INDEPENDENT_REVIEWER_LOGIN", text)
        self.assertIn("$review.actor.login -ceq $candidate.actor.login", text)
        self.assertIn("name: lu-ide-wo7-candidate", text)
        self.assertIn("name: lu-ide-wo7-independent-review", text)
        self.assertIn("RELEASE_ACCEPTANCE=NOT_GRANTED", text)
        self.assertNotIn("releaseApproved: true", text)


if __name__ == "__main__":
    unittest.main()
