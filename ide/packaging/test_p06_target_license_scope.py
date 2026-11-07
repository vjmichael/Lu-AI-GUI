"""P06 target graph categorization never grants missing notice exceptions."""
import copy
import unittest
from p06_target_license_scope import scope, ScopeDenied

SHA="a"*40
WIN="x86_64-pc-windows-msvc"
class TargetScopeTests(unittest.TestCase):
    def setUp(self):
        self.inventory={"schema":"lu.ide.p04.verification-licenses.v1",
            "sourceSha":SHA,"releaseApproved":False,"licenseLegalReviewApproved":False,
            "upstreamRetrieval":{"allSourcesRetrieved":True},
            "resolvedPackages":[
                {"manager":"cargo","name":"winapi","version":"1.0"},
                {"manager":"cargo","name":"objc2","version":"2.0"},
                {"manager":"npm","name":"@esbuild/win32-x64","version":"0.25.9"}],
            "unresolved":[
                "unresolved_notice_or_id:cargo:winapi:1.0",
                "unresolved_notice_or_id:cargo:objc2:2.0",
                "unresolved_notice_or_id:npm:@esbuild/win32-x64:0.25.9"]}
        self.metadata={"packages":[
            {"id":"win","name":"winapi","version":"1.0"},
            {"id":"apple","name":"objc2","version":"2.0"}],
            "resolve":{"nodes":[{"id":"win"}]}}
    def inspect(self):
        return scope(self.inventory,self.metadata,b"inventory",b"metadata",
            SHA,"windows-2022",WIN)
    def test_target_graph_scope_is_never_license_waiver(self):
        out=self.inspect()
        self.assertEqual(out["unresolvedCount"],3)
        self.assertEqual(out["targetGraphRelations"]["selected_in_target_resolve_graph"],1)
        self.assertEqual(out["targetGraphRelations"]["not_selected_in_target_resolve_graph"],1)
        self.assertEqual(out["targetGraphRelations"]["not_cargo_package"],1)
        self.assertTrue(out["allUnresolvedRemainOpen"])
        self.assertTrue(all(x["disposition"]=="open" and x["copyrightCleared"] is False
                            for x in out["tasks"]))
        self.assertFalse(out["releaseApproved"])
        self.assertFalse(out["binaryShipmentProven"])
    def test_target_triple_mismatch_denied(self):
        with self.assertRaisesRegex(ScopeDenied,"unreviewed_cargo_target"):
            scope(self.inventory,self.metadata,b"x",b"y",SHA,"windows-2022",
                  "x86_64-unknown-linux-gnu")
    def test_unknown_package_does_not_become_false_exemption(self):
        self.metadata["packages"].pop()
        out=self.inspect()
        self.assertEqual(out["targetGraphRelations"]["not_in_filtered_metadata_package_inventory"],1)
        self.assertTrue(all(x["disposition"]=="open" for x in out["tasks"]))
    def test_unknown_resolve_node_denied(self):
        self.metadata["resolve"]["nodes"].append({"id":"missing"})
        with self.assertRaisesRegex(ScopeDenied,"duplicate_or_unknown_cargo_resolve_node"):
            self.inspect()
    def test_missing_resolved_notice_package_denied(self):
        self.inventory["resolvedPackages"].pop()
        with self.assertRaisesRegex(ScopeDenied,"unresolved_notice_without_resolved_package"):
            self.inspect()
    def test_forged_release_denied(self):
        self.inventory["releaseApproved"]=True
        with self.assertRaisesRegex(ScopeDenied,"untrusted_unresolved_inventory"):
            self.inspect()
if __name__=="__main__":
    unittest.main()
