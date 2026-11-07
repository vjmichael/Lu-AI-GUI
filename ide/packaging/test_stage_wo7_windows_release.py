"""WO7-B stages only approved pinned files, with all notices, no auto fetch."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from stage_wo7_windows_release import stage, validate, Denied

HEAD = "a" * 40


class StagingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        self.dst = self.root / "installed-payload"
        self.manifest = self.root / "inventory.json"
        helper = self.inputs / "service-install.ps1"
        helper.write_bytes(b"trusted helper fixture")
        self.data = {"schema": "lu.ide.wo7.bundle.v1", "approved": True, "sourceSha": HEAD,
                     "installerScript": {"source": helper.name,
                        "sha256": hashlib.sha256(helper.read_bytes()).hexdigest()}, "files": []}
        assets = [
            ("desktop", "lu-ide-desktop.exe"),
            ("service", "resources/recovery/windows_recovery_service.exe"),
            ("debugger", "resources/debugger/bin/lldb-dap.exe"),
            ("debuggee", "resources/debugger/debuggee/wo6-selftest.exe"),
        ]
        for i, (role, dst) in enumerate(assets):
            content = f"{role} fixture".encode()
            src = self.inputs / f"{i}.exe"
            src.write_bytes(content)
            license_file = self.inputs / f"license-{i}.txt"
            license_file.write_text("Example notice only", encoding="utf-8")
            self.data["files"].append({"role": role, "source": src.name,
                "destination": dst, "sha256": hashlib.sha256(content).hexdigest(),
                "license": "Apache-2.0", "signerThumbprint": "b" * 40 if role == "service" else None,
                "dependenciesReviewed": True,
                "importClosureReviewed": True, "notices": [{"source": license_file.name,
                    "sha256": hashlib.sha256(license_file.read_bytes()).hexdigest()}]})
        self.write_manifest()

    def write_manifest(self):
        self.manifest.write_text(json.dumps(self.data), encoding="utf-8")

    def deny(self, message):
        self.write_manifest()
        with self.assertRaisesRegex(Denied, message):
            stage(self.manifest, self.inputs, self.dst, HEAD)
        self.assertFalse(self.dst.exists())

    def test_positive_control_not_a_release(self):
        report = stage(self.manifest, self.inputs, self.dst, HEAD)
        self.assertFalse(report["releaseApproved"])
        self.assertEqual(len(report["files"]), 4)
        self.assertTrue((self.dst / "resources" / "installer" / "wo7-service-lifecycle.ps1").exists())
        self.assertTrue(json.loads((self.dst / "resources" / "wo7-service-policy.json").read_text())["releaseApproved"])
        self.assertTrue((self.dst / "resources" / "third-party-notices" / "000-000.txt").exists())
        with self.assertRaisesRegex(Denied, "output_must_be_new"):
            stage(self.manifest, self.inputs, self.dst, HEAD)

    def test_current_inventory_denied(self):
        current = Path(__file__).with_name("wo7-bundle.inputs.json")
        self.assertFalse(json.loads(current.read_text())["approved"])

    def test_deny_unreviewed(self):
        self.data["approved"] = False
        self.deny("bundle_not_review_approved")

    def test_deny_digest_mismatch(self):
        self.data["files"][0]["sha256"] = "0" * 64
        self.deny("input_digest_changed")

    def test_deny_missing_license_or_dependency_review(self):
        self.data["files"][2]["notices"] = []
        self.deny("license_notice_missing")
        self.data["files"][2]["notices"] = self.data["files"][0]["notices"]
        self.data["files"][2]["dependenciesReviewed"] = False
        self.deny("dependency_and_import_review_missing")

    def test_deny_path_escape(self):
        self.data["files"][0]["destination"] = "../malware.exe"
        self.deny("unsafe_destination")
        self.data["files"][0]["destination"] = "lu-ide-desktop.exe"
        self.data["files"][0]["source"] = "../outside.exe"
        self.deny("unsafe_asset_path")

    def test_deny_casefold_destination_collision(self):
        self.data["files"].append(dict(self.data["files"][2]))
        self.data["files"][-1]["role"] = "runtime"
        self.data["files"][-1]["destination"] = "RESOURCES/DEBUGGER/BIN/LLDB-DAP.EXE"
        self.deny("duplicate_casefold_destination")

    def test_deny_unlisted_binary_role(self):
        self.data["files"][0]["role"] = "shell-script"
        self.deny("unrecognized_bundle_role")


if __name__ == "__main__":
    unittest.main()
