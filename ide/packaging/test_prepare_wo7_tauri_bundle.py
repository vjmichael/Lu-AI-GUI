"""WO7-B trusted staged Tauri handoff checks and tamper denial."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from prepare_wo7_tauri_bundle import prepare, Denied

HEAD="a"*40
def digest(blob):
    return hashlib.sha256(blob).hexdigest()


class TauriHandoffTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.payload=self.root/"payload"
        self.payload.mkdir()
        self.tauri=self.root/"src-tauri"
        self.tauri.mkdir()
        self.desktop=self.root/"desktop.exe"
        self.desktop.write_bytes(b"reviewed desktop")
        required=[("desktop","lu-ide-desktop.exe"),
                  ("service","resources/recovery/windows_recovery_service.exe"),
                  ("debugger","resources/debugger/bin/lldb-dap.exe"),
                  ("debuggee","resources/debugger/debuggee/wo6-selftest.exe")]
        rows=[]
        for role,target in required:
            blob=b"reviewed desktop" if role=="desktop" else role.encode()
            path=self.payload/target
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes(blob)
            rows.append({"role":role,"destination":target,"sha256":digest(blob),"notices":[]})
        policy=self.payload/"resources/wo7-service-policy.json"
        policy.write_text('{"releaseApproved":true}',encoding="utf-8")
        helper=self.payload/"resources/installer/wo7-service-lifecycle.ps1"
        helper.parent.mkdir(parents=True,exist_ok=True)
        helper.write_bytes(b"reviewed helper")
        self.attest=self.payload/"resources/wo7-staged-tree.json"
        self.doc={"schema":"lu.ide.wo7.staged-tree.v1","sourceSha":HEAD,
                  "releaseApproved":False,"files":rows,
                  "servicePolicySha256":digest(policy.read_bytes()),
                  "lifecycleScriptSha256":digest(helper.read_bytes())}
        self.save()

    def save(self):
        self.attest.write_text(json.dumps(self.doc),encoding="utf-8")

    def test_handoff_copies_exact_reviewed_resource_bytes(self):
        result=prepare(self.payload,self.desktop,self.tauri,HEAD)
        self.assertFalse(result["signedOrInstalled"])
        self.assertTrue((self.tauri/"resources/installer/wo7-service-lifecycle.ps1").exists())
        with self.assertRaisesRegex(Denied,"tauri_resources_must_be_new"):
            prepare(self.payload,self.desktop,self.tauri,HEAD)

    def test_changed_binary_denied(self):
        (self.payload/"resources/debugger/bin/lldb-dap.exe").write_bytes(b"tampered")
        with self.assertRaisesRegex(Denied,"changed_staged_binary"):
            prepare(self.payload,self.desktop,self.tauri,HEAD)

    def test_unlisted_extra_file_denied(self):
        (self.payload/"resources/unlisted.dll").write_bytes(b"bad")
        with self.assertRaisesRegex(Denied,"unlisted_or_missing_stage_bytes"):
            prepare(self.payload,self.desktop,self.tauri,HEAD)

    def test_changed_compiled_desktop_denied(self):
        self.desktop.write_bytes(b"different desktop")
        with self.assertRaisesRegex(Denied,"compiled_desktop_differs"):
            prepare(self.payload,self.desktop,self.tauri,HEAD)

    def test_wrong_source_denied(self):
        with self.assertRaisesRegex(Denied,"source_and_stage_attestation_mismatch"):
            prepare(self.payload,self.desktop,self.tauri,"f"*40)


if __name__=="__main__":
    unittest.main()
