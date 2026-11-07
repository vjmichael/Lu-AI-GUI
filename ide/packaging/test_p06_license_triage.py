"""P06 category hints never waive any missing upstream license evidence."""
import copy
import unittest
from p06_license_triage import triage,TriageDenied,enforce_known_risk_set
class TriageTests(unittest.TestCase):
    def setUp(self):
        self.data={"schema":"lu.ide.p04.verification-licenses.v1",
            "sourceSha":"a"*40,"releaseApproved":False,"licenseLegalReviewApproved":False,
            "upstreamRetrieval":{"allSourcesRetrieved":True},
            "resolvedPackages":[
                {"manager":"cargo","name":"lu-ide-desktop","version":"0.1","declaredLicense":"Apache-2.0","notices":[]},
                {"manager":"npm","name":"@esbuild/linux-x64","version":"0.25.9","declaredLicense":"MIT","notices":[]}],
            "unresolved":["unresolved_notice_or_id:cargo:lu-ide-desktop:0.1",
                          "unresolved_notice_or_id:npm:@esbuild/linux-x64:0.25.9"]}
    def test_explicitly_open_even_if_known_permissive_license(self):
        record=triage(self.data,b"fixture")
        self.assertEqual(record["unresolvedCount"],2)
        self.assertTrue(all(x["disposition"]=="open" and x["reviewedBy"] is None for x in record["tasks"]))
        self.assertFalse(record["releaseApproved"])
    def test_mpl_file_level_review_stays_open(self):
        self.data["resolvedPackages"].append({"manager":"cargo","name":"selectors",
            "version":"0.38.0","declaredLicense":"MPL-2.0","notices":[]})
        result=triage(self.data,b"fixture")
        flags=result["specialLicenseReviews"]
        self.assertEqual(len(flags),1)
        self.assertEqual(flags[0]["reviewRoute"],"mpl_2_file_level_conditions")
        self.assertIsNone(flags[0]["selectedLicenseOption"])
        self.assertEqual(flags[0]["disposition"],"open")
    def test_dual_licensed_lgpl_alternative_is_not_forced_lgpl(self):
        self.data["resolvedPackages"].append({"manager":"cargo","name":"r-efi",
            "version":"6.0.0","declaredLicense":"MIT OR Apache-2.0 OR LGPL-2.1-or-later","notices":[]})
        report=triage(self.data,b"fixture")
        self.assertEqual(report["specialLicenseReviews"][0]["reviewRoute"],"multi_license_choice_review")
        self.assertIsNone(report["specialLicenseReviews"][0]["selectedLicenseOption"])
    def test_new_or_disappeared_special_obligation_fails_policy(self):
        report=triage(self.data,b"fixture")
        policy={"schema":"lu.ide.p06.known-license-risk.v1","releaseApproved":False,
                "allRequireIndependentReview":True,"expectedRiskPackages":[{"manager":"cargo",
                    "name":"unreviewed","version":"1.0","declaredLicense":"GPL-3.0"}]}
        with self.assertRaisesRegex(TriageDenied,"new_or_missing_copyleft_or_multi_license_review"):
            enforce_known_risk_set(report,policy)
    def test_missing_package_denied(self):
        self.data["resolvedPackages"].pop()
        with self.assertRaisesRegex(TriageDenied,"unresolved_without_resolved_package"):
            triage(self.data,b"fixture")
    def test_untrusted_release_claim_denied(self):
        self.data["releaseApproved"]=True
        with self.assertRaisesRegex(TriageDenied,"untrusted_complete_license_inventory"):
            triage(self.data,b"fixture")
    def test_missing_receipt_denied(self):
        self.data["upstreamRetrieval"]=None
        with self.assertRaisesRegex(TriageDenied,"untrusted_complete_license_inventory"):
            triage(self.data,b"fixture")
    def test_all_platform_hints_never_claim_exclusion(self):
        self.data["unresolved"]=self.data["unresolved"][:1]
        result=triage(self.data,b"fixture")
        self.assertEqual(result["tasks"][0]["targetRelevance"],"requires_independent_native_binary_check")
if __name__=="__main__":unittest.main()
