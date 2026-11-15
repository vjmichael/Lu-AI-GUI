"""WO8 Windows must verify exact scoped P06 lock on both installer passes.

The immutable P06 predecessor checker still denies the new local desktop
rand delta; only the separately pinned, unsigned IDE-MERGE-02 successor
can verify the *actual* installed-workflow graph. No release promotion.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/ide-wo8-installed-windows.yml"
CALL = "python ide/packaging/verify_p06_candidate_lock.py"


def check(text: str):
    calls = [line.strip() for line in text.splitlines()
             if line.strip().startswith(CALL)]
    if len(calls) != 2:
        raise ValueError("wo8_p06_expected_both_pre_and_post_lock_checks")
    for line in calls:
        if (line != CALL + " --repo-root . --platform windows-2022 --ide-merge02-successor"):
            raise ValueError("wo8_p06_unreviewed_original_or_loosened_successor")
    if text.count("cargo fetch --locked --manifest-path ide/app/src-tauri/Cargo.toml") != 2:
        raise ValueError("wo8_p06_missing_existing_security_or_installation_gate:two_cargo_fetch")
    for required in (
        "cargo fetch --locked --manifest-path ide/app/src-tauri/Cargo.toml",
        "npm ci --prefix ide/app --ignore-scripts --no-audit --no-fund",
        "verify_p04_license_bundle.py",
        "--require-upstream",
        "source checkout is not exact pull-request head",
        "Test-Wo8NativeDesktopUI.ps1",
        "WO8_NATIVE_PROTECTED_FEATURES=NOT_ACCEPTED",
    ):
        if required not in text:
            raise ValueError("wo8_p06_missing_existing_security_or_installation_gate:" + required)
    return True


class WO8ScopedP06WiringTests(unittest.TestCase):
    def test_real_native_wo8_uses_both_pinned_scoped_lock_checks(self):
        self.assertTrue(check(WORKFLOW.read_text(encoding="utf-8")))

    def test_unscoped_original_reviewer_is_still_denied_for_new_lock(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unreviewed_original"):
            check(text.replace(" --ide-merge02-successor", "", 1))

    def test_missing_or_skipped_second_lock_check_is_denied(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "expected_both"):
            check(text.replace(CALL, "# missing " + CALL, 1).replace(
                "# missing " + CALL + " --repo-root . --platform windows-2022 --ide-merge02-successor",
                "# absent first P06 check", 1))

    def test_cannot_swap_to_unlocked_cargo_or_drop_license_gate(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        for original in ("cargo fetch --locked", "--require-upstream"):
            with self.subTest(original=original):
                with self.assertRaisesRegex(ValueError, "missing_existing"):
                    check(text.replace(original, original.replace("--", ""), 1))


if __name__ == "__main__":
    unittest.main()
