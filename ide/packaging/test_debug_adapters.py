"""WO6 release manifest test; does not install or execute LLDB-DAP."""
import copy
import json
import unittest
from pathlib import Path

from verify_debug_adapters import errors

ROOT = Path(__file__).with_name("debug-adapters.manifest.json")


class DebuggerManifestTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(ROOT.read_text(encoding="utf-8"))

    def test_unapproved_manifest_is_valid_source_but_denied_as_release(self):
        self.assertEqual(errors(self.data), [])
        reasons = errors(self.data, release=True)
        self.assertIn("release_approval_absent", reasons)
        self.assertIn("independent_review_incomplete", reasons)
        self.assertIn("windows-x86_64_sha_missing", reasons)
        self.assertIn("linux-x86_64_sha_missing", reasons)

    def test_no_arbitrary_debuggee_target(self):
        payload = copy.deepcopy(self.data)
        payload["fixedDebuggee"]["userSelectedTarget"] = True
        self.assertIn("arbitrary_debuggee_disallowed", errors(payload))

    def test_a_boolean_approval_alone_cannot_approve_distribution(self):
        payload = copy.deepcopy(self.data)
        payload["releaseApproved"] = True
        self.assertIn("independent_review_incomplete", errors(payload, release=True))
        self.assertIn("windows_signing_identity_unverified", errors(payload, release=True))

    def test_complete_metadata_requires_both_platforms_and_fixed_names(self):
        payload = copy.deepcopy(self.data)
        payload["releaseApproved"] = True
        payload["review"] = {key: "approved" for key in payload["review"]}
        for platform, item in payload["platforms"].items():
            item["bundleSha256"] = "a" * 64
            item["licenseNotice"] = "LICENSE AND THIRD-PARTY NOTICES"
            item["dependenciesReviewed"] = True
            item["installedAppAccepted"] = True
        payload["platforms"]["windows-x86_64"]["authenticodeSignerThumbprint"] = "a" * 40
        payload["platforms"]["linux-x86_64"]["rootOwnedImmutablePackage"] = True
        self.assertEqual(errors(payload, release=True), [])
        payload["platforms"]["linux-x86_64"]["executable"] = "/tmp/lldb-dap"
        self.assertIn("linux-x86_64_unexpected_executable", errors(payload, release=True))


if __name__ == "__main__":
    unittest.main()
