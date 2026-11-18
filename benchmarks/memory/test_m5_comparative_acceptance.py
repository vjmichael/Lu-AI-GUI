"""M5-05 evidence scorer schema tests; fixtures are NOT accepted model results."""
import unittest
from m5_comparative_acceptance import (
    analyze_model,analyze_physical,build,InvalidEvidence,SCHEMA,
)
class M5PilotContract(unittest.TestCase):
    def test_no_inputs_never_mint_release_result_or_metrics(self):
        report=build({})
        self.assertEqual(report["schema"],SCHEMA)
        self.assertFalse(report["release_accepted"])
        self.assertEqual(len(report["missing"]),6)
        self.assertNotIn("paired_real_GGUF",report["measured"])
        self.assertIsNone(report["not_measured"]["rediscovery_tool_calls"])
    def test_both_arms_must_be_paired_and_real_text_present(self):
        model={"schema":"lu-m5-repeated-task-real-gguf-v1",
               "installed_windows_acceptance":False,
               "actual_authorized_tool_effects_tested":False,
               "model_sha256":"a"*64,"engine_sha256":"b"*64,
               "rows":[]}
        with self.assertRaises(InvalidEvidence):analyze_model(model)
        model["rows"]=[
          {"case":"m5","trial":0,"arm":"no_memory","correct":True,"known":True,
           "model_text":"ANSWER: TRACE","latency_ms":3,"retrieval_ms":0,
           "source_id":None}]
        with self.assertRaises(InvalidEvidence):analyze_model(model)
    def test_hostile_reuse_of_permission_or_prior_task_is_not_a_model_proof(self):
        model={"schema":"lu-m5-repeated-task-real-gguf-v1",
               "installed_windows_acceptance":True,
               "actual_authorized_tool_effects_tested":False,
               "model_sha256":"a"*64,"engine_sha256":"b"*64,
               "rows":[{}]}
        with self.assertRaises(InvalidEvidence):analyze_model(model)
    def test_installed_witness_requires_independent_uia_pid(self):
        physical={"schema":"lu-m5-installed-local-model-acceptance-v1",
                  "hashes":{"installedExe":"a"*64,"engine":"b"*64,
                            "firstModel":"c"*64,"replacementModel":"d"*64},
                  "modelSessions":[
                    {"phase":phase,"windowObservation":{"pid":11,"uiAProcessId":12}}
                    for phase in ("primary-before-restart",
                    "same-primary-after-restart",
                    "replacement-after-model-swap")
                  ]}
        with self.assertRaises(InvalidEvidence):analyze_physical(physical)
    def test_invalid_or_missing_input_fails_closed(self):
        report=build({"model":"/definitely/not/a/real/model-result.json"})
        self.assertEqual(report["gates"]["m5-02-real-local-model"]["state"],
                         "INVALID_OR_INCOMPLETE")
        self.assertFalse(report["release_accepted"])
        self.assertEqual(len(report["errors"]),1)
if __name__=="__main__":unittest.main()
