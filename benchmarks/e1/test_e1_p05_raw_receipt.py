"""Synthetic contract fixtures only; NOT native E1-P05 measurements."""
import unittest
from e1_p05_real_three_arm import aggregate
from verify_e1_p05_raw import verify

SHA="1"*40
DIGEST="a"*64
def result(mode, kind, trial, latency):
    row = {
        "schema":"lu-e1-p05-native-real-arm-v2",
        "mode":mode,"kind":kind,"trial":trial,
        "latencyMs":latency,"taskSuccess":mode!="direct_model",
        "modelUsed":mode=="direct_model",
        "source":"actual-local-qwen-gguf-chat" if mode=="direct_model"
            else "real-compiled-lu-core-native",
        "artifactCreated":mode!="direct_model",
    }
    if mode=="direct_model":
        row["modelRawText"]="fixture response (not model output)"
    else:
        row.update({"journalBacked":True,"supported":True,
                    "semanticAccuracy":True,"independentPythonZipWitness":True,
                    "independentPythonSemanticWitness":True,
                    "artifactSha256":DIGEST,"artifactBytes":1024,
                    "unauthorizedAttempts":1,"unauthorizedAttemptsBlocked":1})
    return row

def fixture(model=False):
    modes = ("direct_model","native_utility","reviewed_app") if model else (
        "native_utility","reviewed_app"
    )
    rows=[result(mode,kind,trial,10.0+10*trial)
          for mode in modes for kind in ("docx","xlsx") for trial in (0,1)]
    return {
        "schema":"lu-e1-p05-real-three-arm-v2",
        "sourceCommit":SHA,"nativeSha256":DIGEST,"workerSha256":DIGEST,
        "modelSha256":DIGEST if model else None,
        "engineSha256":DIGEST if model else None,
        "trialCount":len(rows),"results":rows,
        "summary":aggregate(rows,model),
        "status":"GENUINE_THREE_ARM_HEADLESS" if model
            else "NATIVE_ARMS_ONLY_MODEL_NOT_EXECUTED",
        "limitations":{"modelInferenceExecuted":model},
    }

class EvidenceContract(unittest.TestCase):
    def test_synthetic_native_contract_structure(self):
        self.assertTrue(verify(fixture(),SHA,False,2))
    def test_synthetic_three_arm_contract_structure(self):
        self.assertTrue(verify(fixture(True),SHA,True,2))
    def test_wrong_head_unsafe_to_accept(self):
        with self.assertRaisesRegex(ValueError,"source SHA"):
            verify(fixture(),"2"*40,False,2)
    def test_missing_model_must_not_look_like_three_arm(self):
        with self.assertRaises(ValueError):
            verify(fixture(),SHA,True,2)
    def test_duplicate_row_or_missing_trial_fail(self):
        r=fixture();r["results"][1]=r["results"][0]
        with self.assertRaisesRegex(ValueError,"duplicate"):
            verify(r,SHA,False,2)
    def test_summary_p95_needs_nearest_rank(self):
        r=fixture()
        r["summary"]["native_utility"]["p95Ms"]=10.0
        with self.assertRaisesRegex(ValueError,"p50/p95"):
            verify(r,SHA,False,2)
    def test_missing_independent_semantics_is_not_evidence(self):
        r=fixture()
        r["results"][0]["independentPythonSemanticWitness"]=False
        with self.assertRaisesRegex(ValueError,"Office publication"):
            verify(r,SHA,False,2)

if __name__=="__main__":unittest.main()
