import importlib.util, json, pathlib, sqlite3, subprocess, sys, tempfile, unittest
from contextlib import closing
PATH = pathlib.Path(__file__).parent / 'cp05e5-installed-journal.py'
spec=importlib.util.spec_from_file_location('oracle',PATH); o=importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
class Diagnostic(unittest.TestCase):
    def setUp(self):
        self.t=tempfile.TemporaryDirectory(); self.root=pathlib.Path(self.t.name)
        b=self.root/'ansf/books'; b.mkdir(parents=True)
        with closing(sqlite3.connect(b/'test.sqlite')) as c, c:
            c.execute('CREATE TABLE ansf_live_chat_events(seq,event_id,session_id,user_turn_id,generation_id,text,kind)')
            c.executemany('INSERT INTO ansf_live_chat_events VALUES(?,?,?,?,?,?,?)', [(1,'e','chat','task','task',o.APPROVE,'user'),(2,'e2','chat','task','task','PRIVATE_TEST_VALUE','assistant_failed')])
        for db in ['lu-capability-negotiation-audit.sqlite','lu-core-utility-model-audit.sqlite']:
            with closing(sqlite3.connect(self.root/db)) as c, c:
                c.execute('CREATE TABLE lu_model_boundary_audit(sequence,conversation_id,task_id,kind,payload)')
        self.add('selection','USER_REQUEST',o.APPROVE); self.add('selection','MODEL_OUTPUT',json.dumps({'selection':'lu_validate_schema'})); self.add('selection','HOST_ROUTE_SELECTED',json.dumps({'intentApproved':True,'executed':False,'candidate':'lu_validate_schema'}))
    def tearDown(self): self.t.cleanup()
    def add(self,db,kind,payload,task='task'):
        name='lu-capability-negotiation-audit.sqlite' if db=='selection' else 'lu-core-utility-model-audit.sqlite'
        with closing(sqlite3.connect(self.root/name)) as c, c:
            seq=c.execute('SELECT count(*) FROM lu_model_boundary_audit').fetchone()[0]+1
            c.execute('INSERT INTO lu_model_boundary_audit VALUES(?,?,?,?,?)',(seq,'chat',task,kind,payload))
    def run_fail(self):
        r=subprocess.run([sys.executable,str(PATH),str(self.root),'approve'],capture_output=True,text=True)
        self.assertNotEqual(r.returncode,0); self.assertIn('CP05_FAILURE_DIAGNOSTIC:',r.stderr)
        self.assertNotIn('PRIVATE_TEST_VALUE',r.stderr); return r.stderr
    def test_zero_feedback(self):
        self.add('utility','MODEL_OUTPUT','PRIVATE_TEST_VALUE'); self.add('utility','GENERATION_FAILURE','TokenLoop')
        s=self.run_fail(); self.assertIn('observed 0',s); self.assertIn('TokenLoop',s); self.assertIn('NOT_JSON',s)
    def test_multiple_feedback_still_fail(self):
        self.add('utility','TOOL_FEEDBACK',json.dumps({'name':'lu_boundary','response':{'ok':False}}))
        self.add('utility','TOOL_FEEDBACK',json.dumps({'name':'lu_validate_schema','response':{'ok':True}}))
        s=self.run_fail(); self.assertIn('observed 2',s); self.assertIn('SYNTHETIC_BOUNDARY_FEEDBACK',s); self.assertIn('NATIVE_SCHEMA_OK',s)
    def test_unrelated_task_excluded(self):
        self.add('utility','MODEL_OUTPUT','PRIVATE_TEST_VALUE','unrelated'); self.add('utility','GENERATION_FAILURE','InferenceFailed','unrelated')
        x=o.failure_diagnostic(self.root,'approve'); self.assertEqual(x['utility']['kindCounts'],{})
    def test_bounded(self):
        for _ in range(70): self.add('utility','MODEL_OUTPUT','PRIVATE_TEST_VALUE')
        x=o.failure_diagnostic(self.root,'approve'); self.assertTrue(x['utility']['truncated']); self.assertEqual(len(x['utility']['orderedCategories']),64)
    def test_unavailable_original(self):
        self.assertEqual(o.failure_diagnostic(self.root,'deny')['originalTurn'],'UNAVAILABLE_OR_NOT_UNIQUE')
    def test_failure_category_redacts(self):
        self.assertEqual(o.failure_category('ModelReply(Unauthorized(PRIVATE_TEST_VALUE))'),'Unauthorized')
        self.assertEqual(o.failure_category('PRIVATE_TEST_VALUE'),'UNCLASSIFIED')
    def test_backend_failure_detail_is_exact_and_redacted(self):
        self.add('utility','GENERATION_FAILURE','InferenceFailedAt("backend_no_visible_tokens")')
        s=self.run_fail(); self.assertIn('backend_no_visible_tokens',s)
        self.assertEqual(o.failure_category('InferenceFailedAt("backend_timeout")'), 'backend_timeout')
        self.assertEqual(o.failure_category('InferenceFailedAt("backend_timeout PRIVATE_TEST_VALUE")'), 'UNCLASSIFIED')
        self.assertEqual(o.failure_category('InferenceFailedAt("backend_timeout") PRIVATE_TEST_VALUE'), 'UNCLASSIFIED')
    def test_success_gate_unchanged(self):
        self.add('utility','MODEL_OUTPUT',json.dumps({'kind':'tool','name':'lu_validate_schema','arguments':{'source':'current_request'}}))
        self.add('utility','TOOL_FEEDBACK',json.dumps({'name':'lu_validate_schema','response':{'ok':True,'result':{'valid':True,'draft':'2020-12'}}}))
        self.add('utility','MODEL_OUTPUT',json.dumps({'kind':'final','text':'exact answer'})); self.add('utility','FINAL_ANSWER','exact answer')
        with closing(sqlite3.connect(self.root/'ansf/books/test.sqlite')) as c, c: c.execute("UPDATE ansf_live_chat_events SET kind='assistant_completed' WHERE seq=2")
        r=subprocess.run([sys.executable,str(PATH),str(self.root),'approve'],capture_output=True,text=True)
        self.assertEqual(r.returncode,0,r.stderr); self.assertTrue(json.loads(r.stdout)[0]['valid']); self.assertNotIn('CP05_FAILURE_DIAGNOSTIC',r.stderr)
if __name__=='__main__': unittest.main(verbosity=2)
