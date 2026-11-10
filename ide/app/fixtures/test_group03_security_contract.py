#!/usr/bin/env python3
"""Group 3 non-executing negative security contract, NOT a human/OS witness."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SUPERVISOR = ROOT / "supervisor"
PHYSICAL = Path(__file__).with_name("Test-DEV046D-PhysicalHumanConsent.ps1")
TAURI = ROOT / "app" / "src-tauri" / "Cargo.toml"


class AuthorizationBoundaries(unittest.TestCase):
    def test_physical_consent_fails_closed_on_wrong_running_binary(self):
        ps = PHYSICAL.read_text(encoding="utf-8")
        for marker in (
            "requires_physical_interactive_windows_user_not_ci",
            "ExpectedSourceSha", "ExpectedDesktopExecutable",
            "ExpectedDesktopSha256", "git rev-parse HEAD",
            "reviewed_desktop_binary_hash_mismatch",
            "visible_process_does_not_match_reviewed_desktop",
            "running_desktop_image_hash_mismatch",
            "native_human_confirmed", "independent_native_restart_audit_failed",
            "IDE_G03_PHYSICAL_VISIBLE_DESKTOP_SOURCE_AND_BINARY=PINNED_NOT_CONSENT",
            "Read-Host 'After interacting with the real native dialogs",
        ):
            self.assertIn(marker, ps)
        for forbidden in ("SendKeys", "mouse_event", "ci_only_not_installed_consent=PASS"):
            self.assertNotIn(forbidden, ps)

    def test_disposable_acl_grants_are_not_desktop_entitlements(self):
        cm = (SUPERVISOR / "Cargo.toml").read_text(encoding="utf-8")
        desktop = TAURI.read_text(encoding="utf-8")
        self.assertIn("ide056-ci-acl-positive", cm)
        self.assertNotIn("ide056-ci-acl-positive", desktop)
        self.assertNotIn("ide055-ci-wheelhouse", desktop)
        source = (SUPERVISOR / "examples" /
                  "ide056_windows_acl_positive.rs").read_text(encoding="utf-8")
        for marker in ("TokenIntegrityLevel", "WRITE_DAC", "WRITE_OWNER",
                       "TokenIsAppContainer", "IsProcessInJob",
                       "inspect_ide056_readonly_tree"):
            self.assertIn(marker, source)

    def test_simulated_approval_cannot_mint_a_physical_release_witness(self):
        source = (SUPERVISOR / "src" /
                  "debug_action_receipts.rs").read_text(encoding="utf-8")
        for token in ("ci_only_not_installed_consent", "native_human_confirmed",
                      "ResultUncertain", "Consumed"):
            self.assertIn(token, source)


if __name__ == "__main__":
    unittest.main()
