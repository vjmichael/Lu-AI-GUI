import json
import subprocess
import unittest
from pathlib import Path
from p06_live_native_jobs import TECHNICAL_WORKFLOW, TECHNICAL_GATE, GATES

ROOT=Path(__file__).resolve().parents[2]

class DispositionTests(unittest.TestCase):
    def test_exact_path_membership_and_no_authority_promotion(self):
        rows=json.loads((ROOT/'ide/packaging/ide-05b-r3-outside-cohort.json').read_text())['rows']
        paths={p.relative_to(ROOT).as_posix() for p in (ROOT/'.github/workflows').glob('ide-*.yml')}
        automatic={g[1] for g in GATES}
        self.assertEqual(len(rows),45)
        self.assertEqual({r['workflow'] for r in rows},paths-automatic)
        for r in rows:
            self.assertFalse(r['dispatchAuthorized'] or r['retireAuthorized'] or r['parityEstablished'] or r['parentRunObserved'])
            self.assertTrue(set(r['relatedObservedCohortPaths'])<=automatic)
            blob=subprocess.check_output(['git','hash-object','--',r['workflow']],cwd=ROOT,text=True).strip()
            self.assertEqual(blob,r['gitBlob'])
    def test_technical_lane_existing_pr_or_manual_and_nonexecuting(self):
        source=(ROOT/TECHNICAL_WORKFLOW).read_text()
        trigger=source.split('\non:\n',1)[1].split('\npermissions:',1)[0]
        self.assertIn('workflow_dispatch:',trigger)
        self.assertIn('pull_request:',trigger)
        self.assertIn('github.event.pull_request.number == 236',source)
        self.assertIn('github.event.pull_request.head.ref',source)
        self.assertEqual(trigger.split('  pull_request:',1)[0].strip(),
            "push:\n    branches: [ide/integration-git-6c6d6e6f-to-main-20261113]\n    paths: ['.github/ide-merge/ide-dev02-technical-batch.txt']")
        self.assertIn("github.event_name == 'push' && github.repository == 'vjmichael/Lu-AI-GUI' && github.ref == 'refs/heads/ide/integration-git-6c6d6e6f-to-main-20261113'",source)
        self.assertIn("github.event_name == 'push' && github.sha || github.event.pull_request.head.sha || inputs.expected_source_sha",source)
        self.assertIn('name: technical-installed / ${{ matrix.os }} / ${{ matrix.platform }} / ${{ matrix.bundle }}',source)
        self.assertEqual(len(TECHNICAL_GATE[2]),2)
        self.assertIn('git diff --exit-code HEAD --',source)
        build=next(l for l in source.splitlines() if '@tauri-apps/cli@' in l)
        self.assertNotIn('--features',build)
        self.assertNotIn('upload-artifact',source)
        self.assertIn('verify_language_servers.py --release; then exit 2',source)
        manifest=(ROOT/'ide/app/src-tauri/Cargo.toml').read_text()
        self.assertIn('default = []',manifest)
        config=json.loads((ROOT/'ide/app/src-tauri/tauri.dev02.technical-only.conf.json').read_text())
        self.assertEqual(config['bundle']['resources'],{'test-lsp-resources/':'technical-lsp','verification-notices/':'THIRD_PARTY_LICENSES'})
