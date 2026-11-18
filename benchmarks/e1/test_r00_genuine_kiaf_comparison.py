#!/usr/bin/env python3
"""Negative R00 paired-case identity regressions; no fabricated model output."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from r00_genuine_kiaf_comparison import verified_report, compare

def sample(arm, case, repetition=0):
    raw="real response fixture"
    return {"arm":arm,"caseId":case,"repetition":repetition,
            "classification":"MODEL_OUTPUT_FORMAT","rawResponse":raw,
            "rawSha256":hashlib.sha256(raw.encode()).hexdigest(),
            "checks":{},"allPass":False}

def report(protocol, samples):
    return {"schema":"lu-memory-loop03c-real-model-comparison-v1",
            "responseProtocol":protocol,"modelActuallyLoaded":True,
            "fatalError":None,"installedLu":False,"realOsCustodyProven":False,
            "modelSha256":"a"*64,"nativeFixtureSha256":"b"*64,
            "positiveFixtureSha256":"c"*64,"reviewedFixtureSha256":"d"*64,
            "repetitions":1,"observedModelGenerations":len(samples),
            "samples":samples,"overallPass":False}

class PairedIdentityTests(unittest.TestCase):
    def check(self, samples, valid):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/"raw.json"
            path.write_text(json.dumps(report("QWEN3_EMPTY_THINK_PREFILL_V1",samples)))
            if valid:
                self.assertEqual(verified_report(path,"QWEN3_EMPTY_THINK_PREFILL_V1")[0]["samples"],samples)
            else:
                with self.assertRaisesRegex(ValueError,"case identities differ|Duplicate"):
                    verified_report(path,"QWEN3_EMPTY_THINK_PREFILL_V1")

    def test_equal_counts_but_different_case_ids_rejected(self):
        self.check([sample("source_only","a"),sample("host_classified","b")],False)

    def test_equal_counts_but_different_repetitions_rejected(self):
        self.check([sample("source_only","a",0),sample("host_classified","a",1)],False)

    def test_equal_counts_with_permuted_identity_rejected(self):
        self.check([sample("source_only","a"),sample("source_only","b"),
                    sample("host_classified","a"),sample("host_classified","c")],False)

    def test_identical_pair_accepted(self):
        self.check([sample("source_only","a"),sample("host_classified","a")],True)

    def test_cross_protocol_mismatched_case_matrix_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            raw=Path(tmp)/"raw.json"
            structured=Path(tmp)/"schema.json"
            raw.write_text(json.dumps(report("QWEN3_EMPTY_THINK_PREFILL_V1",
                [sample("source_only","a"),sample("host_classified","a")])))
            structured.write_text(json.dumps(report("LLAMA_COMPLETION_JSON_SCHEMA_V1",
                [sample("source_only","b"),sample("host_classified","b")])))
            with self.assertRaisesRegex(ValueError,"Unpaired case matrix"):
                compare(raw,structured)

if __name__=="__main__":
    unittest.main()
