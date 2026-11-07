"""WO8 installed evidence never treats source or hand-authored JSON as native pass."""
import copy
import json
from pathlib import Path
import unittest
from verify_wo8_installed_evidence import validate, GATES, EvidenceDenied

HEAD="a"*40


def complete():
    observations=[]
    for index,(gate,(workflow,os)) in enumerate(sorted(GATES.items())):
        observations.append({
            "gate":gate, "workflow":workflow, "runnerOs":os,
            "sourceSha":HEAD, "result":"passed","skipped":False,
            "installerSha256":"b"*64,"installedBinarySha256":"c"*64,
            "witnessSha256":"d"*64,"runId":100+index,"jobId":200+index,
            "packageSigned":gate.startswith("signed-"),"releaseApproved":False})
    return {"schema":"lu.ide.wo8.installed.v1",
            "sourceSha":HEAD,"observations":observations}


class InstalledEvidenceContractTests(unittest.TestCase):
    def test_default_manifest_denied(self):
        current=Path(__file__).with_name("wo8-installed-evidence.json")
        with self.assertRaisesRegex(EvidenceDenied,"source_head_drift"):
            validate(json.loads(current.read_text(encoding="utf-8")),HEAD)

    def test_even_complete_metadata_cannot_accept_native_run(self):
        result=validate(complete(),HEAD)
        self.assertFalse(result["realInstalledAcceptanceGranted"])
        self.assertFalse(result["releaseAccepted"])
        self.assertFalse(result["independentRunLogInspectionComplete"])

    def test_old_sha_denied(self):
        data=complete()
        data["sourceSha"]="e"*40
        with self.assertRaisesRegex(EvidenceDenied,"source_head_drift"):
            validate(data,HEAD)

    def test_missing_installed_scenario_denied(self):
        data=complete()
        data["observations"].pop()
        with self.assertRaisesRegex(EvidenceDenied,"missing_installed_gate"):
            validate(data,HEAD)

    def test_fake_windows_from_linux_denied(self):
        data=complete()
        data["observations"][0]["runnerOs"]="ubuntu-24.04" if data["observations"][0]["runnerOs"]!="ubuntu-24.04" else "windows-2022"
        with self.assertRaisesRegex(EvidenceDenied,"wrong_workflow_os_or_commit"):
            validate(data,HEAD)

    def test_skipped_and_invalid_witness_denied(self):
        data=complete()
        data["observations"][0]["skipped"]=True
        with self.assertRaisesRegex(EvidenceDenied,"skipped_or_failed_native_gate"):
            validate(data,HEAD)
        data=complete()
        data["observations"][0]["witnessSha256"]=None
        with self.assertRaisesRegex(EvidenceDenied,"unmeasured_artifact_or_witness"):
            validate(data,HEAD)

    def test_unsigned_protected_gate_denied(self):
        data=complete()
        target=next(r for r in data["observations"] if r["gate"]=="signed-protected-debug")
        target["packageSigned"]=False
        with self.assertRaisesRegex(EvidenceDenied,"unsigned_artifact_used_for_protected_release"):
            validate(data,HEAD)

    def test_unsigned_smoke_cannot_claim_release(self):
        data=complete()
        target=next(r for r in data["observations"] if r["gate"]=="windows-installed-shell")
        target["releaseApproved"]=True
        with self.assertRaisesRegex(EvidenceDenied,"unsigned_smoke_claimed_release"):
            validate(data,HEAD)


if __name__=="__main__":
    unittest.main()
