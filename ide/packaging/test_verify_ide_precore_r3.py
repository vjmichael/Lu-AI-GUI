"""Hostile, strictly non-authorizing R3 cross-source/package/custody tests."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from verify_ide_precore_r3 import BASE, Denied, clean_relative, reconcile, source_identity


class R3Tests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.patch = self.root / "r2.patch"
        self.patch.write_bytes(b"exact R2 patch fixture\n")
        self.digest = hashlib.sha256(self.patch.read_bytes()).hexdigest()
        self.r2 = {
            "task": "IDE-PRECORE-CLOSEOUT-R2", "original_pr_sha": BASE,
            "gh_source_pushed": False, "cumulative_applies_clean_exact_original": True,
            "cumulative_sha256": self.digest,
            "host_lock_successor": {"sha256": "a" * 64},
            "desktop_lock_successor": {"sha256": "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"},
        }
        self.record = {
            "schema": "lu.ide.precore.r3.admission-register.v1",
            "repository": "vjmichael/Lu-AI-GUI", "originalPr": 236,
            "originalHead": BASE, "r2CumulativeSha256": self.digest,
            "r2HostLockSha256": "a" * 64, "r2DesktopLockSha256": "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255",
            "candidateSha": None,
            "custodyReceipts": {key: None for key in
                 ("scmTerminal", "jobEmpty", "originalCancel", "revocation", "restartRecovery")},
            "physicalWindowsAccepted": False, "releaseApproved": False,
        }
        self.wo7 = {
            "schema": "lu.ide.wo7.signed-nsis.v1", "sourceSha": None,
            "releaseApproved": False, "licensesReviewed": False,
            **{role: {"path": None, "sha256": None, "signerThumbprint": None}
               for role in ("installer", "service", "desktop")},
        }
        self.bundle = {"schema": "lu.ide.wo7.bundle.v1", "sourceSha": None, "approved": False}
        self.wo7_ev = {"schema": "lu.ide.wo7.evidence.v1", "sourceSha": None,
                       "cleanSource": False, "acceptance": []}
        self.wo9 = {"schema": "lu.ide.wo9.native-release.v1", "sourceSha": None, "observations": []}
        self.record["signingTool"] = {"path": None, "sha256": None}
        self.record["publisherThumbprint"] = None
        self.wo8 = {"schema": "lu.ide.wo8.installed.v1", "sourceSha": None,
                    "observations": []}

    def run_audit(self, **changes):
        kwargs = {"record": self.record, "r2": self.r2, "patch": self.patch,
                  "wo7": self.wo7, "bundle": self.bundle,
                  "wo7_evidence": self.wo7_ev, "wo8": self.wo8, "wo9": self.wo9}
        kwargs.update(changes)
        return reconcile(**kwargs)

    def test_realistic_empty_inputs_remain_blocked(self):
        output = self.run_audit()
        self.assertTrue(output["staticMetadataConsistent"])
        self.assertFalse(output["releaseApproved"])
        self.assertFalse(output["physicalAdmissionApproved"])
        self.assertFalse(output["protectedExecutionAccepted"])
        self.assertIn("signed_installer_not_provisioned", output["blockers"])
        self.assertIn("protected_worker_jobEmpty_receipt_missing", output["blockers"])

    def test_r2_patch_changed_rejected(self):
        self.patch.write_bytes(b"tampered")
        with self.assertRaisesRegex(Denied, "r2_cumulative_patch_changed"):
            self.run_audit()

    def test_original_pr_drift_rejected(self):
        self.r2["original_pr_sha"] = "c" * 40
        with self.assertRaisesRegex(Denied, "r2_original_provenance_mismatch"):
            self.run_audit()

    def test_r2_missing_reviewed_lock_pin_rejected(self):
        self.record["r2DesktopLockSha256"] = "c" * 64
        with self.assertRaisesRegex(Denied, "r2_lock_pin_mismatch"):
            self.run_audit()

    def test_historical_truncated_receipt_cannot_replace_full_source_pin(self):
        self.r2["desktop_lock_successor"]["sha256"] = "77e317747fdf2743f55363c9809f866a770255"
        self.record["r2DesktopLockSha256"] = "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"
        result = self.run_audit()
        self.assertIn("r2_truncated_receipt_requires_independent_source_lock_verification", result["blockers"])
        self.assertFalse(result["physicalAdmissionApproved"])

    def test_r2_reviewed_full_digest_refuses_truncated_register_pin(self):
        self.r2["desktop_lock_successor"]["sha256"] = "77e317747fdf2743f55363c9809f866a770255"
        self.record["r2DesktopLockSha256"] = "77e317747fdf2743f55363c9809f866a770255"
        with self.assertRaisesRegex(Denied, "r2_lock_pin_mismatch"):
            self.run_audit()

    def test_source_manifest_cross_commit_rejected(self):
        self.record["candidateSha"] = "c" * 40
        self.bundle["sourceSha"] = "d" * 40
        with self.assertRaisesRegex(Denied, "cross_manifest_source_drift"):
            self.run_audit()

    def test_base_commit_cannot_be_declared_successor(self):
        self.record["candidateSha"] = BASE
        with self.assertRaisesRegex(Denied, "invalid_successor_candidate_commit"):
            self.run_audit()

    def test_release_approval_cannot_be_self_asserted(self):
        self.wo7["releaseApproved"] = True
        with self.assertRaisesRegex(Denied, "unearned_release_approval_claim"):
            self.run_audit()

    def test_physical_approval_cannot_be_self_asserted(self):
        self.record["physicalWindowsAccepted"] = True
        with self.assertRaisesRegex(Denied, "physical_acceptance_flag_not_explicitly_false"):
            self.run_audit()

    def test_unsigned_reviewed_signtool_not_invented(self):
        result = self.run_audit()
        self.assertIn("reviewed_native_signtool_pin_missing", result["blockers"])
        self.assertIn("reviewed_publisher_certificate_not_provisioned", result["blockers"])

    def test_invalid_signtool_digest_denied(self):
        self.record["signingTool"] = {"path": "signtool.exe", "sha256": "not-a-sha"}
        with self.assertRaisesRegex(Denied, "incomplete_signing_tool_pin"):
            self.run_audit()

    def test_invalid_publisher_identity_denied(self):
        self.record["publisherThumbprint"] = "test-publisher"
        with self.assertRaisesRegex(Denied, "invalid_publisher_thumbprint"):
            self.run_audit()

    def test_wrong_wo9_source_denied(self):
        self.record["candidateSha"] = "c" * 40
        self.wo9["sourceSha"] = "d" * 40
        with self.assertRaisesRegex(Denied, "cross_manifest_source_drift"):
            self.run_audit()

    def test_wrong_manifest_sha_type_denied(self):
        self.wo8["sourceSha"] = 123
        with self.assertRaisesRegex(Denied, "cross_manifest_source_drift"):
            self.run_audit()

    def test_cross_role_replay_receipt_digest_denied(self):
        self.record["candidateSha"] = "c" * 40
        for name in self.record["custodyReceipts"]:
            self.record["custodyReceipts"][name] = {
                "sha256": "a" * 64, "origin": "independent-installed-windows",
                "sourceSha": "c" * 40, "originalOperationId": "op-" + name}
        with self.assertRaisesRegex(Denied, "reused_custody_receipt_digest"):
            self.run_audit()

    def test_partial_signer_pin_rejected(self):
        self.wo7["installer"]["path"] = "lu-ide-signed-setup.exe"
        self.wo7["installer"]["sha256"] = "a" * 64
        with self.assertRaisesRegex(Denied, "incomplete_signed_role_pin"):
            self.run_audit()

    def test_custody_partial_metadata_rejected(self):
        self.record["custodyReceipts"]["jobEmpty"] = {"sha256": "a" * 64, "origin": "test-only"}
        with self.assertRaisesRegex(Denied, "invalid_custody_receipt_metadata"):
            self.run_audit()

    def test_filled_custody_metadata_still_no_native_authority(self):
        self.record["candidateSha"] = "c" * 40
        for n, name in enumerate(self.record["custodyReceipts"]):
            self.record["custodyReceipts"][name] = {
                "sha256": (format(n, "x") + "d" * 63), "origin": "independent-installed-windows", "sourceSha": "c" * 40, "originalOperationId": "op-" + name}
        result = self.run_audit()
        self.assertFalse(result["physicalAdmissionApproved"])
        self.assertIn("independent_native_custody_receipt_authentication_pending", result["blockers"])

    def test_unsafe_asset_paths_rejected(self):
        root = self.root / "candidate"
        root.mkdir()
        (root / "ok.exe").write_bytes(b"binary")
        for name in ("../escape.exe", "C:/unsafe.exe", "a\\unsafe.exe", "a/../ok.exe"):
            with self.subTest(name=name), self.assertRaises(Denied):
                clean_relative(root, name)

    def test_symlink_asset_rejected(self):
        root = self.root / "candidate"
        root.mkdir()
        (root / "ok.exe").write_bytes(b"binary")
        try:
            (root / "link.exe").symlink_to(root / "ok.exe")
        except OSError:
            self.skipTest("symlinks not available on this host")
        with self.assertRaisesRegex(Denied, "linked_package_asset"):
            clean_relative(root, "link.exe")

    def test_license_review_claim_needs_inventory_digest(self):
        self.wo7["licensesReviewed"] = True
        with self.assertRaisesRegex(Denied, "reviewed_license_inventory_digest_missing"):
            self.run_audit()

    def test_signed_role_must_match_reviewer_publisher(self):
        self.record["publisherThumbprint"] = "f" * 40
        self.wo7["installer"] = {"path": "lu-ide-signed-setup.exe", "sha256": "c" * 64,
                                 "signerThumbprint": "a" * 40}
        with self.assertRaisesRegex(Denied, "unreviewed_publisher_for_role"):
            self.run_audit()

    def test_signed_role_path_substitution_rejected(self):
        self.wo7["service"] = {"path": "evil.exe", "sha256": "c" * 64,
                               "signerThumbprint": "a" * 40}
        with self.assertRaisesRegex(Denied, "unreviewed_candidate_binary_path"):
            self.run_audit()

    def test_signed_bytes_digest_mismatch_denied(self):
        self.record["candidateSha"] = "c" * 40
        candidate = self.root / "candidate"
        candidate.mkdir()
        for role, filename in (("installer", "lu-ide-signed-setup.exe"),
                               ("service", "windows_recovery_service.exe"),
                               ("desktop", "lu-ide-desktop.exe")):
            (candidate / filename).write_bytes(role.encode())
            self.wo7[role] = {"path": filename, "sha256": "e" * 64,
                              "signerThumbprint": "a" * 40}
        with self.assertRaisesRegex(Denied, "signed_role_digest_mismatch"):
            self.run_audit(candidate_root=candidate)

    def test_native_source_identity_must_match_app_and_default_off(self):
        checkout = self.root / "checkout"
        (checkout / "ide/host").mkdir(parents=True)
        (checkout / "ide/app/src-tauri").mkdir(parents=True)
        (checkout / "ide/host/Cargo.lock").write_bytes(b"h")
        (checkout / "ide/app/src-tauri/Cargo.lock").write_bytes(b"d")
        self.r2["host_lock_successor"]["sha256"] = hashlib.sha256(b"h").hexdigest()
        self.r2["desktop_lock_successor"]["sha256"] = "77e317747fdf2743f55363c9809f866a770255"
        (checkout / "ide/packaging").mkdir(parents=True)
        (checkout / "ide/packaging/ide-precore-r2-ipc-lock-successor.json").write_text(json.dumps({
            "hostSha256": self.r2["host_lock_successor"]["sha256"],
            "desktopSha256": "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"}))
        config = {"productName": "Lu IDE", "identifier": "org.lu.ide",
                  "version": "0.1.0-alpha.1", "bundle": {"active": False}}
        (checkout / "ide/app/src-tauri/tauri.conf.json").write_text(json.dumps(config))
        cargo = '[package]\nname="lu-ide-desktop"\nversion="0.1.0-alpha.1"\n[features]\ndefault=[]\n'
        (checkout / "ide/app/src-tauri/Cargo.toml").write_text(cargo)
        with patch("verify_ide_precore_r3.git_output", side_effect=["c" * 40, ""]), \
             patch("verify_ide_precore_r3.subprocess.run") as run, \
             patch("verify_ide_precore_r3.sha256", side_effect=[hashlib.sha256(b"h").hexdigest(), "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"]):
            run.return_value.returncode = 0
            result = source_identity(checkout, "c" * 40, self.r2)
        self.assertFalse(result["defaultProtectedFeatures"])
        config["bundle"]["active"] = True
        (checkout / "ide/app/src-tauri/tauri.conf.json").write_text(json.dumps(config))
        with patch("verify_ide_precore_r3.git_output", side_effect=["c" * 40, ""]), \
             patch("verify_ide_precore_r3.subprocess.run") as run, \
             patch("verify_ide_precore_r3.sha256", side_effect=[hashlib.sha256(b"h").hexdigest(), "77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255"]):
            run.return_value.returncode = 0
            with self.assertRaisesRegex(Denied, "default_bundle_release_enabled"):
                source_identity(checkout, "c" * 40, self.r2)


if __name__ == "__main__":
    unittest.main()
