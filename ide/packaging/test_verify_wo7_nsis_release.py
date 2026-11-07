"""WO7 fail-closed verifier tests. Mock signatures never imply native acceptance."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_wo7_nsis_release as gate

SOURCE = "a" * 40
SIGNER = "b" * 40


class SignedReleaseGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "installer.exe").write_bytes(b"safe unsigned NSIS fixture")
        (self.root / "service.exe").write_bytes(b"safe unsigned service fixture")
        self.manifest = self.root / "release.json"
        self.data = {
            "schema": gate.__dict__["audit"].__globals__["__name__"] and "lu.ide.wo7.signed-nsis.v1",
            "releaseApproved": True,
            "sourceSha": SOURCE,
            "licensesReviewed": True,
            "acceptance": {k: "approved" for k in gate.GATES},
        }
        for kind, filename in (("installer", "installer.exe"), ("service", "service.exe")):
            self.data[kind] = {
                "path": filename,
                "sha256": hashlib.sha256((self.root / filename).read_bytes()).hexdigest(),
                "signerThumbprint": SIGNER,
            }
        self.save()

    def save(self):
        self.manifest.write_text(json.dumps(self.data), encoding="utf-8")

    def checked(self, signature=None):
        with patch.object(gate, "debugger_errors", return_value=[]):
            return gate.audit(self.manifest, self.root, SOURCE,
                              signature=signature or (lambda p: ("Valid", SIGNER)))

    def denied(self, expected):
        with self.assertRaisesRegex(gate.Denied, expected):
            self.checked()

    def test_repository_manifest_is_unapproved(self):
        original = Path(gate.__file__).with_name("wo7-release.manifest.json")
        current = json.loads(original.read_text(encoding="utf-8"))
        self.assertFalse(current["releaseApproved"])
        self.data = current
        self.save()
        self.denied("release_unapproved")

    def test_preflight_positive_control_is_not_installed_acceptance(self):
        result = self.checked()
        self.assertEqual(result["result"], "binary_preflight_only")
        self.assertFalse(result["installedAcceptancePerformed"])
        self.assertEqual(len(result["artifacts"]), 2)

    def test_metadata_cannot_bypass_actual_debugger_gate(self):
        with self.assertRaisesRegex(gate.Denied, "wo6_debugger_not_release_approved"):
            gate.audit(self.manifest, self.root, SOURCE,
                       signature=lambda p: ("Valid", SIGNER))

    def test_rejects_wrong_signer(self):
        with patch.object(gate, "debugger_errors", return_value=[]):
            with self.assertRaisesRegex(gate.Denied, "installer_signature_invalid"):
                gate.audit(self.manifest, self.root, SOURCE,
                           signature=lambda p: ("Valid", "c" * 40))

    def test_rejects_byte_mutation(self):
        (self.root / "service.exe").write_bytes(b"changed")
        self.denied("service_digest_mismatch")

    def test_rejects_symlink_and_parent_escape(self):
        (self.root / "alias.exe").symlink_to(self.root / "installer.exe")
        self.data["installer"]["path"] = "alias.exe"
        self.save()
        self.denied("symlink_asset_denied")
        self.data["installer"]["path"] = "../installer.exe"
        self.save()
        self.denied("unsafe_asset_path")

    def test_missing_independent_approval_denied(self):
        self.data["acceptance"]["uninstall"] = "pending"
        self.save()
        self.denied("native_and_installed_acceptance_missing")

    def test_source_sha_drift_denied(self):
        self.data["sourceSha"] = "f" * 40
        self.save()
        self.denied("source_sha_mismatch")


if __name__ == "__main__":
    unittest.main()
