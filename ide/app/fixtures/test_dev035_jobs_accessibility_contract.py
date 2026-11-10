#!/usr/bin/env python3
"""Source contract for the *real installed* Jobs AT-SPI empty-journal witness.

No fake worker result, no DOM injection and no relaxation of default-deny.
The native GUI run must independently establish the actual state.
"""
from pathlib import Path
import unittest

HERE = Path(__file__).parent
WEB = HERE.parent / "web" / "main.jsx"
FIXTURE = HERE / "dev035_jobs_installed_atspi.py"


class NativeJobsA11yContract(unittest.TestCase):
    def test_empty_lifecycle_state_is_semantically_named_only_when_empty(self):
        web = WEB.read_text(encoding="utf-8")
        self.assertIn('aria-label="DEV-03.5 verified protected job lifecycle"', web)
        self.assertIn('projectJobLifecycle(buildStatus,activeProject?.projectId,', web)
        self.assertIn(').jobs.length===0', web)
        self.assertIn('role="status"\n                      aria-label="No matching protected job journal entries observed"', web)
        self.assertIn('No matching protected job journal entries observed.</p>', web)
        self.assertIn('disabled={!activeProject || !buildStatus?.ready', web)

    def test_real_atspi_checks_exact_named_status_under_real_jobs_region(self):
        fixture = FIXTURE.read_text(encoding="utf-8")
        self.assertIn('def native_empty_job_journal(seconds=16):', fixture)
        self.assertIn('current.name or "").strip() == "DEV-03.5 verified protected job lifecycle"', fixture)
        self.assertIn('name == "No matching protected job journal entries observed"', fixture)
        self.assertIn('state.contains(pyatspi.STATE_VISIBLE)', fixture)
        self.assertIn('visited < 3200', fixture)
        self.assertIn('depth < 24', fixture)
        self.assertIn('if last_region == 1 and exact_status == 1 and lists == 0:', fixture)
        self.assertIn('native_empty_job_journal(seconds=16)', fixture)
        self.assertNotIn('accessible("No matching protected job journal entries observed",seconds=16)', fixture)
        self.assertIn('run.getState().contains(pyatspi.STATE_ENABLED)', fixture)
        self.assertIn('if digest()!=old:', fixture)
        self.assertIn('DEV035_LINUX_INSTALLED_PROTECTED_WORKER=NOT_ACCEPTED', fixture)


if __name__ == "__main__":
    unittest.main()
