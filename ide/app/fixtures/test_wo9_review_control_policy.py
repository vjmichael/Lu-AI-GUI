"""Policy-only fake AT-SPI tests; not native installed acceptance."""
import ast, types, unittest
from pathlib import Path
source=Path(__file__).with_name("wo8_linux_editor_atspi.py").read_text(encoding="utf-8")
module=ast.parse(source)
funcs=[n for n in module.body if isinstance(n,ast.FunctionDef) and n.name in ('native_workflow_review_control','activate_native_workflow_review')]
assert len(funcs)==2
flags=('VISIBLE','SHOWING','ENABLED','SENSITIVE','FOCUSABLE','FOCUSED')
api=types.SimpleNamespace(**{'STATE_'+x:x for x in flags},SCROLL_ANYWHERE='anywhere')
visible=set(flags[:4]); label='Verify frozen Git and artifact evidence again'
class Clock:
    def __init__(self): self.now=0
    def monotonic(self): self.now+=.01;return self.now
    def sleep(self,t): self.now+=t
class Node:
    def __init__(self,name,role,states=(),children=(),scroll=False,result=True,actions=('click',)):
        self.name=name; self.role=role;self.states=set(states);self.children=list(children);self.scroll=scroll;self.result=result;self.actions=actions;self.clicks=0;self.scrolls=0;self.focuses=0
    @property
    def childCount(self): return len(self.children)
    def getChildAtIndex(self,i): return self.children[i]
    def getRoleName(self): return self.role
    def getState(self): return types.SimpleNamespace(contains=lambda f:f in self.states)
    def queryComponent(self): return self
    def scrollTo(self,_):
        self.scrolls+=1
        if self.scroll:self.states.update(visible)
        return self.scroll
    def grabFocus(self):
        self.focuses+=1
        if 'FOCUSABLE' in self.states and visible<=self.states:self.states.add('FOCUSED');return True
        return False
    def queryAction(self):return self
    @property
    def nActions(self):return len(self.actions)
    def getName(self,i):return self.actions[i]
    def doAction(self,i):self.clicks+=1;return self.result

def deny(s): raise RuntimeError(s)
class Policy(unittest.TestCase):
    def run_case(self,children,roots=None):
        self.keys=[]; root=Node('Multi-project native workflow evidence','region',visible,children)
        env=dict(time=Clock(),pyatspi=api,elements=lambda:roots if roots is not None else [root],_native_dialog_ancestor=lambda n:False,deny=deny,key=lambda k:self.keys.append(k))
        exec(compile(ast.Module(body=funcs,type_ignores=[]),'wo9','exec'),env)
        return env['activate_native_workflow_review'],env['native_workflow_review_control']
    def test_exact_action(self):
        n=Node(label,'push button',visible);act,_=self.run_case([n]);act(label,.3);self.assertEqual(n.clicks,1)
    def test_scroll_then_action(self):
        n=Node(label,'button',{'ENABLED','SENSITIVE'},scroll=True);act,_=self.run_case([n]);act(label,.3);self.assertEqual((n.scrolls,n.clicks),(1,1))
    def test_hidden_no_input(self):
        n=Node(label,'button',{'ENABLED','SENSITIVE'});act,_=self.run_case([n]);self.assertRaises(RuntimeError,act,label,.3);self.assertEqual(n.clicks,0);self.assertEqual(self.keys,[])
    def test_duplicate(self):
        a=Node(label,'push button',visible);b=Node(label,'button',[]);act,_=self.run_case([a,b]);self.assertRaises(RuntimeError,act,label,.3);self.assertEqual(a.clicks+b.clicks,0)
    def test_role_mismatch(self):
        n=Node(label,'text',visible);act,_=self.run_case([n]);self.assertRaises(RuntimeError,act,label,.3);self.assertEqual(n.clicks,0)
    def test_blocked(self):
        n=Node(label,'button',visible);b=Node('Blocked by native evidence','status',[]);act,_=self.run_case([n,b]);self.assertRaisesRegex(RuntimeError,'blocked',act,label,.3);self.assertEqual(n.clicks,0)
    def test_disabled(self):
        n=Node(label,'button',{'VISIBLE','SHOWING'});act,_=self.run_case([n]);self.assertRaises(RuntimeError,act,label,.3);self.assertEqual(n.clicks,0)
    def test_uncertain_no_retry(self):
        n=Node(label,'button',visible,result=False);act,_=self.run_case([n]);self.assertRaisesRegex(RuntimeError,'uncertain',act,label,.3);self.assertEqual(n.clicks,1);self.assertEqual(self.keys,[])
    def test_keyboard_only_once(self):
        n=Node(label,'button',visible|{'FOCUSABLE'});act,_=self.run_case([n]);act(label,.3);self.assertEqual(n.clicks,0);self.assertEqual(self.keys,['Return'])
    def test_ambiguous_action_denied(self):
        n=Node(label,'button',visible,actions=('click','press'));act,_=self.run_case([n]);self.assertRaises(RuntimeError,act,label,.3);self.assertEqual(n.clicks,0)
    def test_status_exact(self):
        n=Node('Exact evidence matched','status',{'VISIBLE'});_,find=self.run_case([n]);self.assertIs(find(n.name,.3),n)
if __name__=='__main__':unittest.main(verbosity=2)
