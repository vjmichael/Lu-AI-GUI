import importlib.util
import pathlib
import unittest

path = pathlib.Path(__file__).resolve().parents[1] / 'scripts' / 'ci_prune_superseded.py'
spec = importlib.util.spec_from_file_location('ci_prune', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
now = 1794117600
old = 'a' * 40
head = 'b' * 40

def make(id, sha, status='queued', workflow=4, branch='lu/test', when='2026-11-08T03:00:00Z', pr=110):
    return dict(id=id, head_sha=sha, status=status, event='pull_request',
                workflow_id=workflow, head_branch=branch, created_at=when,
                conclusion=None, pull_requests=[dict(number=pr)])

class QueueSafety(unittest.TestCase):
    def test_only_old_superseded(self):
        newer = make(9, head, when='2026-11-08T04:00:00Z')
        self.assertEqual([x['old'] for x in module.candidates([make(7,old),newer],{'lu/test':head},now)],[7])
    def test_never_active_or_current_or_unique(self):
        newer = make(9,head,when='2026-11-08T04:00:00Z')
        group = [make(1,head), make(2,old,'in_progress'),make(3,old,'completed'),make(4,old,workflow=5),newer]
        self.assertEqual(module.candidates(group,{'lu/test':head},now),[])
    def test_needs_head_and_original_pr(self):
        newer = make(9,head,when='2026-11-08T04:00:00Z')
        self.assertEqual(module.candidates([make(7,old),newer],{},now),[])
        self.assertEqual(module.candidates([make(7,old),dict(newer,pull_requests=[dict(number=111)])],{'lu/test':head},now),[])
    def test_never_after_successor_cancel(self):
        newer = dict(make(9,head,when='2026-11-08T04:00:00Z'),status='completed',conclusion='cancelled')
        self.assertEqual(module.candidates([make(7,old),newer],{'lu/test':head},now),[])

if __name__ == '__main__': unittest.main()
