import unittest
from p06_live_native_jobs import assess, TECHNICAL_GATE, TECHNICAL_WORKFLOW, TECHNICAL_BRANCH, GATES
from test_p06_live_native_jobs import run, good_jobs, SHA, OTHER

class TechnicalObservationTests(unittest.TestCase):
    def fixture(self):
        runs=[run(g[1],ident=100+i) for i,g in enumerate(GATES)]
        jobs={r['id']:good_jobs(g,ident=r['id']) for r,g in zip(runs,GATES)}
        return runs,jobs
    def test_green_automatic_does_not_close_manual_technical(self):
        runs,jobs=self.fixture()
        v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertTrue(v['automaticParentCohortSuccessful'])
        self.assertFalse(v['automaticAndTechnicalInstallationSuccessful']); self.assertEqual(v['technicalInstalledObservation']['state'],'missing')
    def test_required_manual_exact_job_matrix(self):
        runs,jobs=self.fixture(); r=run(TECHNICAL_WORKFLOW,ident=999); r['event']='workflow_dispatch'; r['head_branch']=TECHNICAL_BRANCH; runs.append(r)
        jobs[999]=good_jobs(TECHNICAL_GATE,ident=999)
        v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertTrue(v['automaticAndTechnicalInstallationSuccessful']); self.assertFalse(v['protectedExecutionAccepted']); self.assertFalse(v['releaseApproved'])
        jobs[999].pop(); v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertFalse(v['automaticAndTechnicalInstallationSuccessful']); self.assertEqual(v['technicalInstalledObservation']['state'],'incomplete_jobs')
    def test_wrong_event_and_wrong_attempt_never_qualify(self):
        runs,jobs=self.fixture(); r=run(TECHNICAL_WORKFLOW,ident=999); runs.append(r); jobs[999]=good_jobs(TECHNICAL_GATE,ident=999)
        self.assertFalse(assess(SHA,runs,lambda i,a:jobs[i])['allObservedGatesSuccessful'])
        r['event']='workflow_dispatch'; r['head_branch']=TECHNICAL_BRANCH; r['run_attempt']=2
        self.assertEqual(assess(SHA,runs,lambda i,a:jobs[i])['technicalInstalledObservation']['state'],'invalid_job_attempt')
    def test_newest_manual_failure_not_replaced_by_older_green(self):
        runs,jobs=self.fixture(); r=run(TECHNICAL_WORKFLOW,ident=999); r['event']='workflow_dispatch'; r['head_branch']=TECHNICAL_BRANCH; runs.append(r); jobs[999]=good_jobs(TECHNICAL_GATE,ident=999)
        latest=run(TECHNICAL_WORKFLOW,ident=1000,conclusion='failure'); latest['event']='workflow_dispatch'; latest['head_branch']=TECHNICAL_BRANCH; runs.append(latest); jobs[1000]=good_jobs(TECHNICAL_GATE,ident=1000)
        self.assertEqual(assess(SHA,runs,lambda i,a:jobs[i])['technicalInstalledObservation']['state'],'failed_or_cancelled')
    def test_parent_replay_not_eligible_for_new_sha(self):
        r=run(TECHNICAL_WORKFLOW,ident=999);r['event']='workflow_dispatch'; r['head_branch']=TECHNICAL_BRANCH
        self.assertEqual(assess(OTHER,[r],lambda *_:self.fail('foreign jobs read'))['technicalInstalledObservation']['state'],'missing')

    def test_only_exact_batch_branch_push_eligible(self):
        runs,jobs=self.fixture(); r=run(TECHNICAL_WORKFLOW,ident=999); r['event']='push'; r['head_branch']=TECHNICAL_BRANCH; runs.append(r); jobs[999]=good_jobs(TECHNICAL_GATE,ident=999)
        v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertTrue(v['automaticAndTechnicalInstallationSuccessful']); self.assertFalse(v['protectedExecutionAccepted']); self.assertFalse(v['releaseApproved'])
        for key,value in [('head_branch','other-branch'),('head_repository',{'full_name':'foreign/repo'}),('head_sha',OTHER)]:
            old=r[key]; r[key]=value
            v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertEqual(v['technicalInstalledObservation']['state'],'missing')
            r[key]=old

    def test_only_exact_existing_pr_eligible(self):
        runs,jobs=self.fixture(); r=run(TECHNICAL_WORKFLOW,ident=999); r['event']='pull_request'; r['head_branch']=TECHNICAL_BRANCH; r['pull_requests']=[{'number':236}]; runs.append(r); jobs[999]=good_jobs(TECHNICAL_GATE,ident=999)
        self.assertTrue(assess(SHA,runs,lambda i,a:jobs[i])['automaticAndTechnicalInstallationSuccessful'])
        for key,value in [('pull_requests',[{'number':237}]),('head_branch','other-branch')]:
            old=r[key]; r[key]=value
            v=assess(SHA,runs,lambda i,a:jobs[i]); self.assertFalse(v['automaticAndTechnicalInstallationSuccessful']); self.assertEqual(v['technicalInstalledObservation']['state'],'missing')
            r[key]=old
