"""M5 benchmark contract tests. These do not impersonate GGUF responses."""
import unittest
from m5_repeated_task_real_model import (
    ARMS,CASES,SOURCES,validate_cases,corpus,prompt_for,norm,metrics,verdict,
)
class M5RepeatTaskContract(unittest.TestCase):
    def test_distinct_provenance_and_unknown_safety_cases(self):
        self.assertEqual(validate_cases(),len(CASES))
        self.assertEqual(ARMS,("no_memory","original_ansf"))
        self.assertEqual(len({c[4] for c in CASES}),len(CASES)-1)
        self.assertTrue(any(c[1]=="correction" for c in CASES))
        self.assertTrue(any(c[1]=="version" for c in CASES))
        self.assertTrue(any(c[1]=="authority" for c in CASES))
        self.assertTrue(any(c[1]=="cross_chat" for c in CASES))
    def test_fixture_is_real_native_schema_with_originals(self):
        dataset=corpus()
        self.assertEqual(len(dataset["sessions"]),len(SOURCES))
        for item in dataset["sessions"]:
            self.assertTrue(item["id"].startswith("c-"))
            self.assertEqual(len(item["chapters"]),1)
            self.assertTrue(all(row["role"] in ("user","assistant")
              for row in item["chapters"][0]["entries"]))
    def test_no_memory_arm_never_receives_source_data(self):
        case=CASES[0]
        src={"status":"found","source_id":"ansf:v1:Yy1tNS1idWlsZA:1",
             "source_role":"user","content":"Recovery handoff codename is Harbor Finch."}
        no=prompt_for(case,"no_memory")
        yes=prompt_for(case,"original_ansf",src)
        self.assertIn(case[2],no)
        self.assertIn(case[2],yes)
        self.assertNotIn("Harbor Finch",no)
        self.assertNotIn("ansf:v1:",no)
        self.assertIn("Harbor Finch",yes)
        self.assertIn("NO TOOL GRANTS",yes)
    def test_strict_single_line_model_scoring(self):
        self.assertEqual(norm("ANSWER: Harbor Finch"),"harbor finch")
        self.assertEqual(norm("ANSWER: UNKNOWN"),"unknown")
        self.assertNotEqual(norm("ANSWER: wrong\nANSWER: right"),"right")
        self.assertNotEqual(norm("This output has no answer marker"),"unknown")
        self.assertNotEqual(norm("ANSWER: <invented>"),"invented")
    def test_verdict_demands_real_positive_recall_without_harming_unknown(self):
        # These are numeric CONTRACT examples, never evidence of model runs.
        empty={"known_answer_accuracy":0.0,"unknown_safety_accuracy":1.0}
        loss={"known_answer_accuracy":0.2,"unknown_safety_accuracy":0.5}
        gain={"known_answer_accuracy":0.75,"unknown_safety_accuracy":1.0}
        self.assertEqual(verdict([],{"no_memory":empty,
          "original_ansf":loss}),"UNSAFE_MEMORY_FALSE_CONFIDENCE")
        self.assertEqual(verdict([],{"no_memory":empty,
          "original_ansf":empty}),"NO_DEMONSTRATED_MEMORY_BENEFIT")
        self.assertTrue(verdict([],{"no_memory":empty,
          "original_ansf":gain}).startswith("M5_HEADLESS_MEMORY_BENEFIT"))
if __name__=="__main__":unittest.main()
