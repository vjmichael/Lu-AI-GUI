"""P05 regression: fail-closed detection of Rust/JS/CLI Tauri drift."""
import json
from pathlib import Path
import tempfile
import unittest
from verify_tauri_minor import check_alignment, VersionDenied, WORKFLOWS

class TauriAlignmentTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.write("ide/app/src-tauri/Cargo.toml",
                   '[dependencies]\ntauri = { version = "=2.12.5", features = [] }\n')
        self.package = {"dependencies": {"@tauri-apps/api": "2.12.4"}}
        self.flush_package()
        for path in WORKFLOWS:
            self.write(path, "npx --yes @tauri-apps/cli@2.12.4 build --bundles nsis\n"
                             "& ide/app/fixtures/Test-InstalledAlphaNsis.ps1\n")
    def write(self, name, data):
        p = self.root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(data, encoding="utf-8")
    def flush_package(self):
        self.write("ide/app/package.json", json.dumps(self.package))
    def test_exact_patch_and_matching_minor_accepted(self):
        result = check_alignment(self.root)
        self.assertEqual(result["minor"], "2.12")
        self.assertTrue(result["nativeInstallerGatesPreserved"])
    def test_cargo_caret_drift_denied(self):
        self.write("ide/app/src-tauri/Cargo.toml",
                   '[dependencies]\ntauri = { version = "2.12.5", features = [] }\n')
        with self.assertRaisesRegex(VersionDenied, "rust_tauri_requires_exact_patch_pin"):
            check_alignment(self.root)
    def test_cross_minor_resolved_cargo_denied(self):
        self.write("ide/app/src-tauri/Cargo.toml",
                   '[dependencies]\ntauri = { version = "=2.13.0", features = [] }\n')
        with self.assertRaisesRegex(VersionDenied, "rust_js_major_minor_mismatch"):
            check_alignment(self.root)
    def test_js_npm_range_denied(self):
        self.package["dependencies"]["@tauri-apps/api"] = "^2.12.4"
        self.flush_package()
        with self.assertRaisesRegex(VersionDenied, "javascript_api_requires_exact_patch_pin"):
            check_alignment(self.root)
    def test_cli_minor_mismatch_denied(self):
        self.write(WORKFLOWS[0],
                   "npx --yes @tauri-apps/cli@2.11.5 build --bundles nsis\n"
                   "Test-InstalledAlphaNsis.ps1\n")
        with self.assertRaisesRegex(VersionDenied, "installed_workflow_cli_major_minor_mismatch"):
            check_alignment(self.root)
    def test_installed_native_smoke_step_cannot_disappear(self):
        self.write(WORKFLOWS[1],
                   "npx --yes @tauri-apps/cli@2.12.4 build --bundles nsis\n")
        with self.assertRaisesRegex(VersionDenied, "installed_native_smoke_gate_removed"):
            check_alignment(self.root)
    def test_installed_api_mismatch_denied(self):
        self.write("ide/app/node_modules/@tauri-apps/api/package.json",
                   json.dumps({"version": "2.11.1"}))
        with self.assertRaisesRegex(VersionDenied, "installed_api_version_differs_from_source_pin"):
            check_alignment(self.root)
    def test_lockfile_runtime_minor_mismatch_denied(self):
        self.write("ide/app/src-tauri/Cargo.lock",
                   'version = 3\n[[package]]\nname = "tauri"\nversion = "2.12.5"\n'
                   '[[package]]\nname = "tauri-runtime"\nversion = "2.13.0"\n'
                   '[[package]]\nname = "tauri-runtime-wry"\nversion = "2.12.3"\n')
        with self.assertRaisesRegex(VersionDenied, "resolved_tauri_runtime_major_minor_mismatch"):
            check_alignment(self.root)
    def test_resolved_matching_lockfile_accepted(self):
        self.write("ide/app/src-tauri/Cargo.lock",
                   'version = 3\n[[package]]\nname = "tauri"\nversion = "2.12.5"\n'
                   '[[package]]\nname = "tauri-runtime"\nversion = "2.12.2"\n'
                   '[[package]]\nname = "tauri-runtime-wry"\nversion = "2.12.3"\n')
        self.assertEqual(check_alignment(self.root)["minor"], "2.12")
    def test_missing_workflow_denied(self):
        (self.root / WORKFLOWS[0]).unlink()
        with self.assertRaisesRegex(VersionDenied, "required_installed_workflow_missing"):
            check_alignment(self.root)

if __name__ == "__main__":
    unittest.main()
