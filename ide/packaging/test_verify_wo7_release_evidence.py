"""WO7-E offline evidence claims are never proof of independently fetched CI."""
import copy
import json
from pathlib import Path
import unittest

from verify_wo7_release_evidence import audit, REQUIRED, WORKFLOWS, Denied

HEAD="a"*40


def full_record():
    rows=[]
    for i,gate in enumerate(sorted(REQUIRED)):
        rows.append({"gate":gate,"sourceSha":HEAD,"conclusion":"success",
                     "os":"ubuntu-24.04" if gate.startswith("native-linux") else "windows-2022",
                     "runId":i+100,"jobId":i+200,"workflow":WORKFLOWS[gate],
                     "witnessSha256":"b"*64,"skipped":False,"containmentVerified":True})
    return {"schema":"lu.ide.wo7.evidence.v1","sourceSha":HEAD,
            "cleanSource":True,"acceptance":rows}


class EvidenceTests(unittest.TestCase):
    def test_checked_in_evidence_denies_release(self):
        path=Path(__file__).with_name("wo7-evidence.inputs.json")
        with self.assertRaises(Denied):
            audit(json.loads(path.read_text(encoding="utf-8")),HEAD)

    def test_positive_metadata_is_not_provenance(self):
        receipt=audit(full_record(),HEAD)
        self.assertFalse(receipt["releaseAccepted"])
        self.assertFalse(receipt["githubAndNativeEvidenceIndependentlyFetched"])
        self.assertEqual(receipt["gateCount"],len(REQUIRED))

    def test_missing_gate(self):
        record=full_record()
        record["acceptance"].pop()
        with self.assertRaisesRegex(Denied,"missing_acceptance_gates"):
            audit(record,HEAD)

    def test_fake_source_job_is_not_native_workflow(self):
        record=full_record()
        record["acceptance"][0]["workflow"]="ide-wo7-signed-release-preflight.yml"
        with self.assertRaisesRegex(Denied,"native_workflow_path_mismatch"):
            audit(record,HEAD)

    def test_skips_or_duplicate_receipts_denied(self):
        record=full_record()
        record["acceptance"][0]["skipped"]=True
        with self.assertRaisesRegex(Denied,"unverified_or_skipped"):
            audit(record,HEAD)
        record=full_record()
        record["acceptance"][0]["runId"]=record["acceptance"][1]["runId"]
        record["acceptance"][0]["jobId"]=record["acceptance"][1]["jobId"]
        with self.assertRaisesRegex(Denied,"missing_or_reused"):
            audit(record,HEAD)

    def test_wrong_os_native_claim_denied(self):
        record=full_record()
        for row in record["acceptance"]:
            if row["gate"]=="native-linux-dap":
                row["os"]="windows-2022"
        with self.assertRaisesRegex(Denied,"linux_witness_not_from_linux"):
            audit(record,HEAD)


if __name__=="__main__":
    unittest.main()
