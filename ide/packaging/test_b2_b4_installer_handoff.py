"""Offline transport and admission regressions; no native installer approval."""
import io
import hashlib
import shutil
import urllib.error
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import b2_b4_installer_handoff as h
from test_p05_upstream_bundle_integration import UpstreamBundlingTests
from verify_p04_license_bundle import audit, sha

HEAD='a'*40
PLATFORM='ubuntu-24.04'
CTX={'sourceSha':HEAD,'repositoryId':123,'pullRequest':236,'imageOS':'ubuntu24','imageVersion':'20261118.1',
     'platform':PLATFORM,'runnerArch':'X64','releaseApproved':False,'installedAcceptance':False}


def run_fixture():
    return {'path':h.producer_path(PLATFORM),'event':'pull_request','head_sha':HEAD,'run_attempt':1,'id':100,
            'repository':{'id':123},'head_repository':{'id':123},
            'pull_requests':[{'number':236,'head':{'sha':HEAD,'repo':{'id':123}},'base':{'repo':{'id':123}}}]}


def job_fixture():
    plat=h.PLATFORMS[PLATFORM]
    return {'name':plat[4],'head_sha':HEAD,'run_id':100,'run_attempt':1,'conclusion':'failure','labels':[PLATFORM],
            'steps':[{'name':n,'number':i,'conclusion':'success'} for i,n in enumerate(
                ['Verify exact source checkout for native installed package','Validate frontend before package admission','Fetch resolved Cargo sources before collecting license notices',plat[5],plat[6],'Record exact-source B2/B4 package handoff (not installed acceptance)',plat[7]],1)]
                     +[{'name':'Installed B2 acceptance','number':8,'conclusion':'failure'}]}


def art_fixture():
    return {'id':200,'name':h.artifact_name(PLATFORM),'expired':False,'size_in_bytes':1024,'digest':'sha256:'+'a'*64,
            'workflow_run':{'id':100,'head_sha':HEAD,'repository_id':123,'head_repository_id':123}}


class FakeApi:
    def __init__(self, runs=None,jobs=None,artifacts=None):
        self.runs=[run_fixture()] if runs is None else runs
        self.jobs=[job_fixture()] if jobs is None else jobs
        self.artifacts=[art_fixture()] if artifacts is None else artifacts
    def json(self,suffix):
        if suffix.startswith('workflows/'):
            assert 'head_sha='+HEAD in suffix
            return {'total_count':len(self.runs),'workflow_runs':self.runs}
        if suffix=='runs/100/attempts/1/jobs?per_page=100':
            return {'total_count':len(self.jobs),'jobs':self.jobs}
        if suffix=='runs/100/artifacts?per_page=100':
            return {'total_count':len(self.artifacts),'artifacts':self.artifacts}
        raise AssertionError(suffix)


class MetadataTests(unittest.TestCase):
    def test_select_exact_uploaded_package_even_if_b2_installed_failed(self):
        self.assertEqual(h.find_artifact(FakeApi(),CTX,PLATFORM)[1]['id'],200)
    def test_stale_or_wrong_branch_or_attempt_or_pr_or_repository(self):
        for key,value in [('head_sha','b'*40),('event','workflow_dispatch'),('run_attempt',2),('path',h.producer_path('windows-2022')),
                          ('head_repository',{'id':999}),('pull_requests',[]),('repository',{'id':999})]:
            with self.subTest(key=key):
                run=run_fixture(); run[key]=value
                self.assertIsNone(h.find_artifact(FakeApi(runs=[run]),CTX,PLATFORM))
    def test_ambiguous_producer_falls_back(self):
        self.assertIsNone(h.find_artifact(FakeApi(runs=[run_fixture(),run_fixture()]),CTX,PLATFORM))
        self.assertIsNone(h.find_artifact(FakeApi(artifacts=[art_fixture(),art_fixture()]),CTX,PLATFORM))
    def test_upload_and_prior_build_gates_required(self):
        for i in range(7):
            job=job_fixture(); job['steps'][i]['conclusion']='failure'
            self.assertIsNone(h.find_artifact(FakeApi(jobs=[job]),CTX,PLATFORM))
        job=job_fixture(); job['steps'][0]['number']=99
        self.assertIsNone(h.find_artifact(FakeApi(jobs=[job]),CTX,PLATFORM))
    def test_missing_or_expired_digest_falls_back(self):
        for key,val in [('expired',True),('digest',None)]:
            a=art_fixture(); a[key]=val
            self.assertIsNone(h.find_artifact(FakeApi(artifacts=[a]),CTX,PLATFORM))
    def test_cross_head_artifact_identity_denied(self):
        a=art_fixture(); a['workflow_run']['head_sha']='b'*40
        with self.assertRaisesRegex(h.Denied,'artifact_run_binding_mismatch'):
            h.find_artifact(FakeApi(artifacts=[a]),CTX,PLATFORM)
    def test_other_lane_accepted_and_reused_package_cannot_be_producer(self):
        run=run_fixture();run['path']=h.producer_path(PLATFORM,'b4')
        self.assertEqual(h.find_artifact(FakeApi(runs=[run]),CTX,PLATFORM,'b4')[1]['id'],200)
        job=job_fixture();job['steps'][3]['conclusion']='skipped'
        self.assertIsNone(h.find_artifact(FakeApi(runs=[run],jobs=[job]),CTX,PLATFORM,'b4'))
    def test_wait_dependency_only_toward_earlier_running_job(self):
        run=run_fixture();run['status']='in_progress'
        job=job_fixture();job['status']='in_progress';job['started_at']='2026-11-18T19:00:00Z'
        job['steps'][3]['conclusion']=None
        own=job_fixture();own['run_id']=101;own['status']='in_progress';own['started_at']='2026-11-18T19:01:00Z'
        class WaitApi:
            def json(self,suffix):
                assert suffix=='runs/101/attempts/1/jobs?per_page=100'
                return {'total_count':1,'jobs':[own]}
        self.assertTrue(h.should_wait(WaitApi(),CTX,PLATFORM,run,job,101))
        own['started_at']='2026-11-18T18:59:00Z'
        self.assertFalse(h.should_wait(WaitApi(),CTX,PLATFORM,run,job,101))
        own['started_at']=job['started_at']
        self.assertTrue(h.should_wait(WaitApi(),CTX,PLATFORM,run,job,101))
        run['status']='queued'
        self.assertFalse(h.should_wait(WaitApi(),CTX,PLATFORM,run,job,101))
        run['status']='in_progress';job['steps'][3]['conclusion']='skipped'
        self.assertFalse(h.should_wait(WaitApi(),CTX,PLATFORM,run,job,101))
    def test_archive_without_producer_returns_no_evidence(self):
        self.assertIsNone(h.find_artifact(FakeApi(artifacts=[]),CTX,PLATFORM))


class PayloadTests(unittest.TestCase):
    def setUp(self):
        # Reuse the existing upstream-retrieval fixture with its unmodified legal verifier.
        self.fixture=UpstreamBundlingTests('test_actual_upstream_retrieval_and_every_notice_byte_bound')
        self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        self.fixture.collect(); self.base=self.fixture.root/'staged';self.base.mkdir()
        self.fixture.out.rename(self.base/h.NOTICES)
        self.installer=self.base/'target/release/bundle/deb/example.deb'
        self.installer.parent.mkdir(parents=True); self.installer.write_bytes(b'fixture-not-an-installer')
        self.ctx={**CTX,'cargoLockSha256':sha(self.fixture.root/'ide/app/src-tauri/Cargo.lock'),
                  'npmLockSha256':sha(self.fixture.root/'ide/app/package-lock.json')}
        receipt=audit(self.base/h.NOTICES,HEAD,require_upstream=True)
        receipt.update(installerSha256=sha(self.installer),installerName=self.installer.name)
        (self.base/h.RECEIPT).write_text(json.dumps(receipt),encoding='utf-8')
        self.proof={**self.ctx,**h.verify_payload(self.base,self.installer,self.ctx),'schema':h.SCHEMA,
                    'producerRunId':100,'producerAttempt':1,'producerWorkflow':h.producer_path(PLATFORM)}
        self.write_proof(self.proof)
    def write_proof(self,proof):
        (self.base/h.MANIFEST).write_text(json.dumps(proof),encoding='utf-8')
    def accept(self):
        h.accept_payload(self.base,self.installer,self.ctx,run_fixture(),PLATFORM)
    def test_exact_transport_and_legal_bytes(self):
        self.accept()
        self.assertFalse(self.proof['installedAcceptance']);self.assertFalse(self.proof['releaseApproved'])
    def test_tampered_installer(self):
        self.installer.write_bytes(b'modified')
        with self.assertRaisesRegex(h.Denied,'original_package_receipt_mismatch'):self.accept()
    def test_tampered_notice(self):
        (self.base/h.NOTICES/'upstream-fetch.json').write_bytes(b'evil')
        with self.assertRaisesRegex(ValueError,'upstream_fetch_receipt_digest_drift'):self.accept()
    def test_rebound_manifest_or_approval_denied(self):
        for key,val in [('producerRunId',999),('producerAttempt',2),('platform','windows-2022'),('releaseApproved',True),
                        ('installedAcceptance',True),('sourceSha','b'*40),('sourceTree','bad'),('recipeSha256','bad')]:
            with self.subTest(key=key):
                self.write_proof({**self.proof,key:val})
                with self.assertRaisesRegex(h.Denied,'producer_manifest_mismatch'):self.accept()
    def test_mismatched_runner_image_falls_back(self):
        self.write_proof({**self.proof,'imageVersion':'another'})
        with self.assertRaises(h.Unavailable):self.accept()
    def test_archive_roundtrip_uses_original_upload_layout(self):
        archive=self.fixture.root/'payload.zip'; dest=self.fixture.root/'unpack';dest.mkdir()
        with zipfile.ZipFile(archive,'w') as z:
            for p in self.base.rglob('*'):
                if p.is_file():z.write(p,p.relative_to(self.base).as_posix())
        installer=h.extract(archive,dest,PLATFORM)
        h.accept_payload(dest,installer,self.ctx,run_fixture(),PLATFORM)
        self.assertEqual(sha(installer),sha(self.installer))
    def test_producer_record_is_fresh_and_bound_to_current_run(self):
        (self.base/h.MANIFEST).unlink()
        repo=self.fixture.root/'producer';(repo/h.ROOT).parent.mkdir(parents=True)
        shutil.copytree(self.base,repo/h.ROOT)
        env={'GITHUB_RUN_ID':'100','GITHUB_RUN_ATTEMPT':'1'}
        with patch.object(h,'context',return_value=self.ctx),patch.object(h,'verify_checkout'):
            h.record(repo,PLATFORM,env,'b2')
            proof=json.loads((repo/h.ROOT/h.MANIFEST).read_text())
            self.assertEqual(proof,self.proof)
            with self.assertRaises(FileExistsError):h.record(repo,PLATFORM,env,'b2')
    def test_windows_archive_recipe_is_separate_and_wrong_platform_refused(self):
        win='windows-2022'
        window=self.base/'target/release/bundle/nsis/lu-ide_0.1.0_x64-setup.exe'
        window.parent.mkdir(parents=True);self.installer.rename(window)
        ctx={**self.ctx,'platform':win}
        receipt=audit(self.base/h.NOTICES,HEAD,require_upstream=True)
        receipt.update(installerSha256=sha(window),installerName=window.name)
        (self.base/h.RECEIPT).write_text(json.dumps(receipt),encoding='utf-8')
        self.write_proof({**ctx,**h.verify_payload(self.base,window,ctx),'schema':h.SCHEMA,
                          'producerRunId':100,'producerAttempt':1,'producerWorkflow':h.producer_path(win,'b4')})
        archive=self.fixture.root/'windows.zip';dest=self.fixture.root/'unpack-win';dest.mkdir()
        with zipfile.ZipFile(archive,'w') as z:
            for p in self.base.rglob('*'):
                if p.is_file():z.write(p,p.relative_to(self.base).as_posix())
        installer=h.extract(archive,dest,win)
        h.accept_payload(dest,installer,ctx,run_fixture(),win,'b4')
        with self.assertRaises(h.Denied):h.extract(archive,self.fixture.root/'bad-platform',PLATFORM)
    def test_full_consumer_copy_never_publishes_producer_as_fresh_build(self):
        archive=self.fixture.root/'peer.zip'
        with zipfile.ZipFile(archive,'w') as z:
            for p in self.base.rglob('*'):
                if p.is_file():z.write(p,p.relative_to(self.base).as_posix())
        consumer=self.fixture.root/'consumer';base=consumer/h.ROOT;base.mkdir(parents=True)
        runner=self.fixture.root/'runner';runner.mkdir()
        env={'GITHUB_RUN_ATTEMPT':'1','GH_TOKEN':'test','GITHUB_REPOSITORY':'owner/repo','RUNNER_TEMP':str(runner),'GITHUB_RUN_ID':'101'}
        api=FakeApi();api.download=lambda a,d:shutil.copyfile(archive,d)
        with patch.object(h,'context',return_value=self.ctx),patch.object(h,'verify_checkout'),patch.object(h,'Api',return_value=api):
            self.assertTrue(h.consume(consumer,PLATFORM,env,0,'b4'))
        target=h.package(base,PLATFORM)
        self.assertEqual(sha(target),sha(self.installer))
        self.assertTrue((base/h.NOTICES/'inventory.json').exists())
        self.assertFalse((base/h.RECEIPT).exists())
        self.assertFalse((base/h.MANIFEST).exists())
        proof=json.loads((runner/'ide-b2-b4-producer.json').read_text())
        self.assertEqual(proof['artifactId'],200);self.assertFalse(proof['installedAcceptance'])
    def test_invalid_path_extra_file_symlink_and_case_collision(self):
        cases=[('../escape',None),('verification-notices/../x',None),('verification-notices/x:ads',None),
               ('verification-notices/CON.txt',None),('evil.exe',None),('verification-notices/link',stat.S_IFLNK),
               (h.MANIFEST.upper(),None)]
        for i,(name,mode) in enumerate(cases):
            with self.subTest(name=name):
                archive=self.fixture.root/f'bad{i}.zip';dest=self.fixture.root/f'unpack{i}';dest.mkdir()
                with zipfile.ZipFile(archive,'w') as z:
                    z.write(self.base/h.MANIFEST,h.MANIFEST)
                    info=zipfile.ZipInfo(name)
                    if mode: info.create_system=3;info.external_attr=mode<<16
                    z.writestr(info,b'bad')
                with self.assertRaises(h.Denied):h.extract(archive,dest,PLATFORM)
    def test_mismatch_lock(self):
        self.ctx['cargoLockSha256']='b'*64
        with self.assertRaisesRegex(h.Denied,'notice_lock_mismatch'):self.accept()


class TransportTests(unittest.TestCase):
    def test_download_strips_token_and_checks_github_digest(self):
        payload=b'known zip bytes';art=art_fixture();art['digest']='sha256:'+hashlib.sha256(payload).hexdigest()
        api=h.Api('owner/repo','secret-test-token')
        seen=[]
        def opener(req,timeout):
            seen.append(req)
            if len(seen)==1:
                raise urllib.error.HTTPError(req.full_url,302,'redirect',{'Location':'https://test.blob.core.windows.net/a?signed=1'},io.BytesIO())
            return io.BytesIO(payload)
        with tempfile.TemporaryDirectory() as folder:
            with patch.object(api.opener,'open',side_effect=opener):
                api.download(art,Path(folder)/'artifact.zip')
            self.assertEqual((Path(folder)/'artifact.zip').read_bytes(),payload)
        self.assertEqual(seen[0].get_header('Authorization'),'Bearer secret-test-token')
        self.assertIsNone(seen[1].get_header('Authorization'))
    def test_transport_rejects_wrong_digest_or_untrusted_redirect(self):
        for url,reason in [('https://evil.example/zip','untrusted_artifact_download_host'),('https://test.blob.core.windows.net/a','transport_digest_mismatch')]:
            api=h.Api('owner/repo','secret-test-token');seen=[]
            def opener(req,timeout):
                seen.append(req)
                if len(seen)==1:raise urllib.error.HTTPError(req.full_url,302,'redirect',{'Location':url},io.BytesIO())
                return io.BytesIO(b'tampered')
            with tempfile.TemporaryDirectory() as folder,patch.object(api.opener,'open',side_effect=opener):
                with self.assertRaisesRegex(h.Denied,reason):api.download(art_fixture(),Path(folder)/'artifact.zip')


class WorkflowTests(unittest.TestCase):
    def test_original_independent_witnesses_and_fallback_remain(self):
        root=Path(__file__).resolve().parents[2]/'.github/workflows'
        for platform,osname,fixtures in [('windows-2022','windows',('Test-B2InstalledDraftRecovery.ps1','Test-Wo8NativeDesktopUI.ps1')),('ubuntu-24.04','linux',('Test-B2InstalledDeb.sh','Test-B4InstalledDeb.sh'))]:
            for lane,fixture in zip(('b2','b4'),fixtures):
                text=(root/f'ide-{lane}-installed-editor-{osname}.yml').read_text()
                self.assertIn('pull_request:',text)
                self.assertIn('record --platform '+platform+' --lane '+lane,text)
                self.assertIn('consume --platform '+platform+' --lane '+lane,text)
                self.assertIn('npm run check',text);self.assertIn('actions: read',text)
                self.assertNotIn('actions: write',text)
                self.assertEqual(text.count("if: steps.reuse.outputs.reused == 'false'"),3)
                self.assertIn('verify_p04_license_bundle.py',text);self.assertIn('--require-upstream',text)
                self.assertIn(fixture,text)
                self.assertIn('NOT_APPROVED' if osname=='linux' or lane=='b2' else 'NOT_ACCEPTED',text)
                self.assertNotIn('continue-on-error:',text)

if __name__=='__main__':unittest.main()
