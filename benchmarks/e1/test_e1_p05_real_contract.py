"""E1-P05 benchmark contract tests. These fixtures are NOT runtime results."""
import pathlib
import tempfile
import unittest
from e1_p05_real_three_arm import (
    SCHEMA,ARMS,KINDS,EXPECTED,MODEL_SHA,aggregate,check_binary,digest,
)

class NativeBenchmarkContract(unittest.TestCase):
    def test_missing_model_and_native_arms_report_missing_not_success(self):
        result=aggregate([],False)
        self.assertEqual(set(result),set(ARMS))
        self.assertTrue(all(v["state"]=="NOT_EXECUTED" for v in result.values()))
        self.assertTrue(all(v["taskSuccessRate"] is None for v in result.values()))

    def test_task_fixture_both_formats_have_independent_expected_content(self):
        self.assertEqual(set(KINDS),set(EXPECTED))
        self.assertNotEqual(EXPECTED["docx"]["required"],EXPECTED["xlsx"]["required"])
        self.assertEqual(len(MODEL_SHA),64)
        self.assertEqual(SCHEMA,"lu-e1-p05-real-three-arm-v2")

    def test_metrics_do_not_confuse_knowledge_and_artifact_creation(self):
        rows=[
            {"mode":"direct_model","kind":"xlsx","latencyMs":100.0,
             "taskSuccess":False,"knowledgeSpecAccurate":True,
             "unauthorizedAttempts":0,"unauthorizedAttemptsBlocked":0},
            {"mode":"native_utility","kind":"xlsx","latencyMs":8.0,
             "taskSuccess":True,"unauthorizedAttempts":1,
             "unauthorizedAttemptsBlocked":1},
            {"mode":"reviewed_app","kind":"xlsx","latencyMs":25.0,
             "taskSuccess":True,"unauthorizedAttempts":1,
             "unauthorizedAttemptsBlocked":1},
        ]
        stats=aggregate(rows,True)
        self.assertEqual(stats["direct_model"]["modelSpecAccuracy"],1)
        self.assertEqual(stats["direct_model"]["taskSuccessRate"],0)
        self.assertEqual(stats["native_utility"]["taskSuccessRate"],1)
        self.assertEqual(stats["reviewed_app"]["unauthorizedAttemptsBlocked"],1)

    def test_both_real_native_creators_must_have_format_parity_in_scoring(self):
        rows=[
            {"mode":mode,"kind":kind,"latencyMs":10.0,"taskSuccess":True,
             "unauthorizedAttempts":1,"unauthorizedAttemptsBlocked":1}
            for mode in ("native_utility","reviewed_app") for kind in KINDS
        ]
        summary=aggregate(rows,False)
        for mode in ("native_utility","reviewed_app"):
            self.assertEqual(summary[mode]["trials"],2)
            self.assertEqual(summary[mode]["taskSuccessRate"],1.0)
            self.assertTrue(all(summary[mode]["byFormat"][kind]["total"]==1
                                for kind in KINDS))

    def test_nearest_rank_p95_does_not_understate_two_and_four_trials(self):
        def rows(latencies):
            return [{
                "mode":"native_utility", "kind":"docx", "latencyMs":value,
                "taskSuccess":True, "unauthorizedAttempts":1,
                "unauthorizedAttemptsBlocked":1,
            } for value in latencies]
        two=aggregate(rows([10.0,90.0]), False)["native_utility"]
        self.assertEqual(two["p50Ms"],50.0)
        self.assertEqual(two["p95Ms"],90.0)
        self.assertEqual(two["p95SampleCount"],2)
        self.assertEqual(two["p95Estimator"],"nearest_rank_empirical")
        four=aggregate(rows([10.0,20.0,30.0,100.0]),False)["native_utility"]
        self.assertEqual(four["p95Ms"],100.0)
        self.assertEqual(four["p95SampleCount"],4)

    def test_missing_or_empty_real_binary_refused(self):
        with tempfile.TemporaryDirectory() as root:
            path=pathlib.Path(root)/"missing-worker"
            with self.assertRaises(RuntimeError):
                check_binary(path,"test")
            path.write_bytes(b"trusted bytes only for hash helper unit test")
            self.assertEqual(len(digest(path)),64)
if __name__=="__main__":unittest.main()
