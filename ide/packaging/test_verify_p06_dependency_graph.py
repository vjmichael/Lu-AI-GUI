"""Hostile P06 resolved graph checks, no release inference."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from verify_p06_dependency_graph import inspect,Denied
SHA="a"*40
class GraphTests(unittest.TestCase):
    def setUp(self):
        t=tempfile.TemporaryDirectory()
        self.addCleanup(t.cleanup)
        self.root=Path(t.name)
        tauri=self.root/"ide/app/src-tauri"
        tauri.mkdir(parents=True)
        self.cargo=tauri/"Cargo.lock"
        self.npm=self.root/"ide/app/package-lock.json"
        self.cargo.write_text(self.cargo_data(),encoding="utf-8")
        (tauri/"Cargo.toml").write_text('[dependencies]\ntauri={version="=2.12.5"}\n',encoding="utf-8")
        (self.root/"ide/app/package.json").write_text('{"dependencies":{"@tauri-apps/api":"2.12.4"}}',encoding="utf-8")
        self.data={"lockfileVersion":3,"packages":{
            "":{"dependencies":{"@tauri-apps/api":"2.12.4"}},
            "node_modules/@tauri-apps/api":{"version":"2.12.4","resolved":"https://registry.npmjs.org/api","integrity":"sha512-test"}}}
        self.write_npm()
    def cargo_data(self,minor="2.12.1"):
        return ('version=4\n[[package]]\nname="tauri"\nversion="2.12.5"\n'
                '[[package]]\nname="tauri-runtime"\nversion="'+minor+'"\n'
                '[[package]]\nname="tauri-runtime-wry"\nversion="2.12.2"\n')
    def write_npm(self):
        self.npm.write_text(json.dumps(self.data)+" "*100,encoding="utf-8")
    def check(self):
        with patch("verify_p06_dependency_graph.audit_licenses",return_value={
            "actualBytesVerified":True,"upstreamRetrievalVerified":True,"unresolved":2}):
            return inspect(self.root,SHA,self.root/"license")
    def test_resolved_lock_hash_and_explicit_denial(self):
        result=self.check()
        self.assertEqual(len(result["cargoLockSha256"]),64)
        self.assertFalse(result["releaseApproved"])
        self.assertEqual(result["unresolvedLicenseEntries"],2)
    def test_wrong_runtime_minor_denied(self):
        self.cargo.write_text(self.cargo_data("2.13.0"),encoding="utf-8")
        with self.assertRaisesRegex(Denied,"resolved_tauri_runtime_minor_drift"):
            self.check()
    def test_absent_cargo_lock_denied(self):
        self.cargo.unlink()
        with self.assertRaisesRegex(Denied,"missing_or_unsafe_lockfile"):
            self.check()
    def test_js_resolution_drift_denied(self):
        self.data["packages"]["node_modules/@tauri-apps/api"]["version"]="2.11.1"
        self.write_npm()
        with self.assertRaisesRegex(Denied,"resolved_js_api_version_drift"):
            self.check()
    def test_missing_npm_integrity_denied(self):
        del self.data["packages"]["node_modules/@tauri-apps/api"]["integrity"]
        self.write_npm()
        with self.assertRaisesRegex(Denied,"npm_resolution_integrity_missing"):
            self.check()
    def test_unknown_cargo_git_source_denied(self):
        self.cargo.write_text(self.cargo_data()+'[[package]]\nname="evil"\nversion="1.0.0"\nsource="git+https://example.test"\n',encoding="utf-8")
        with self.assertRaisesRegex(Denied,"unreviewed_nonregistry_cargo_dependency"):
            self.check()
if __name__=="__main__": unittest.main()
