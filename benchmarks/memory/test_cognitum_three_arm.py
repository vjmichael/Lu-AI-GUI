import importlib.util, pathlib, sys, unittest
D=pathlib.Path(__file__).resolve().parent
sys.path.insert(0,str(D))
spec=importlib.util.spec_from_file_location("cognitum",D/"cognitum_three_arm_real_model.py")
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class TestCognitumThreeArm(unittest.TestCase):
    def test_no_oracle_ids(self):
        self.assertEqual(m.ARMS,("none","recent_chat","ansf_hybrid"))
        self.assertGreaterEqual(len(m.CASES),12)
        self.assertEqual(len(m.CASES),len(set(c[0] for c in m.CASES)))
        for case,question,expected,source,evidence in m.CASES:
            self.assertNotIn("ansf:v1:",question)
            self.assertNotIn(expected.lower(),question.lower())
            if source: self.assertIn(source,{s[0] for s in m.SOURCES})
    def test_source_chunks_bounded(self):
        parts=m.chunks("pinned original\n\n"+"abcde "*350)
        self.assertTrue(all(0<len(p)<=1400 for p in parts))
    def test_real_embedding_shape_rejected(self):
        good={"data":[{"index":0,"embedding":[0.1,0.2,0.3]}]}
        self.assertEqual(m.strict_embedding(good,3),[0.1,0.2,0.3])
        with self.assertRaises(RuntimeError):m.strict_embedding(good,384)
        with self.assertRaises(RuntimeError):
            m.strict_embedding({"data":[{"index":0,"embedding":[0,0,0]}]})
    def test_no_mock_results(self):
        s=(D/"cognitum_three_arm_real_model.py").read_text()
        self.assertIn('run_native(native,"discover"',s)
        self.assertIn("INVALID_COGNITUM_REAL_MODEL_EVIDENCE",s)
        self.assertIn("source_info.get(",s)
if __name__=="__main__":unittest.main()
