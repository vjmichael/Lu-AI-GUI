import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location(
    "memory_benchmark",Path(__file__).with_name("three_arm_real_model.py"))
m=importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

class BenchContract(unittest.TestCase):
    def test_fixture_facts_are_isolated_from_questions(self):
        corpus,cases,originals=m.fixtures()
        self.assertEqual(len(cases),14)
        self.assertEqual(len(corpus["sessions"]),3)
        self.assertGreater(len(originals),45)
        for c in cases:
            self.assertNotIn(c["expected"].lower(),c["question"].lower())
        self.assertEqual(set(c["bucket"] for c in cases),
            {"old","recent","cross_chat","unknown","stale","general"})
    def test_recent_history_is_a_real_baseline_not_a_strawman(self):
        corpus,cases,_=m.fixtures()
        entries=[e for ch in corpus["sessions"][0]["chapters"] for e in ch["entries"]]
        recent="\n".join(e["text"] for e in entries[-9:])
        self.assertIn("R-18",recent)
        self.assertIn("Vulkan",recent)
        self.assertIn("Linux",recent)
        self.assertIn("NOVA-3",recent)
        self.assertNotIn("Silver Finch",recent)
        self.assertNotIn("Blue Harbor",recent)
    def test_answer_placeholder_diagnostics_are_scored_symmetrically(self):
        for output in ("<value> Linux</value>","<value> Linux",
                       "<short exact value> Linux;","ANSWER: Linux"):
            self.assertEqual(m.norm(output),m.norm("Linux"))
        self.assertEqual(m.norm("<unknown>"),m.norm("UNKNOWN"))
        for output in ("<value>","<short exact value>",
                       "<value> 12 milliseconds; UNKNOWN.",
                       "ANSWER: 7; answer: 31"):
            self.assertNotEqual(m.norm(output),m.norm("7"))
            self.assertNotEqual(m.norm(output),m.norm("12"))
        self.assertNotEqual(m.norm("<value> 31"),m.norm("21"))
    def test_prompt_never_teaches_literal_placeholder(self):
        self.assertNotIn("<value>",m.SYSTEM)
        self.assertNotIn("<short exact value>",m.SYSTEM)
    def test_answer_scoring_no_generous_inference(self):
        self.assertEqual(m.norm("ANSWER: Blue Harbor"),m.norm("Blue Harbor"))
        self.assertNotEqual(m.norm("ANSWER: 9"),m.norm("7"))
        self.assertNotEqual(m.norm("ANSWER: Amber Lantern"),m.norm("UNKNOWN"))
    def test_no_advantage_is_a_negative_result(self):
        def row(arm,bucket,good):
            return {"arm":arm,"bucket":bucket,"correct":good}
        rows=[row("recent_chat","recent",True),
              row("ansf_direct_id","recent",False),
              row("none","recent",False)]
        stats={"recent_chat":{"accuracy":1.0},
               "ansf_direct_id":{"accuracy":0.0}}
        self.assertEqual(m.verdict(rows,stats),"ANSF_REGRESSION_RECENT")
        rows=[row(arm,"old",False) for arm in m.ARMS]
        stats["recent_chat"]["accuracy"]=0
        self.assertEqual(m.verdict(rows,stats),"NO_ANSF_ADVANTAGE")
    def test_source_id_is_an_opaque_locator_not_answer(self):
        value=m.locator("c-example",47)
        self.assertTrue(value.startswith("ansf:v1:"))
        self.assertNotIn("Blue Harbor",value)

if __name__=="__main__":unittest.main()
