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
        # Original gitBlob/triggerKinds remain historical evidence. Admit only
        # these three independently reviewed, exact successor workflow blobs.
        # The FIN-04R1 successor adds the reviewed fixed-probe denial gate;
        # retain its exact current Git blob pin, never a wildcard allowance.
        reviewed_successors = {
            '.github/workflows/ide-fin04r1-exact-native.yml': ('6f4bf3ff41acd9eea869d701a0799125aa4e3d65', 'aaca9c87aa8de0ba2dde807c9b43d547fb3ffc37'),
            '.github/workflows/ide-group04d-source.yml': ('08c1e1eeb4a7b71e31707d4377764412d9a1556a', 'c735825a928a1b9b86708f630b5c4893bc87bfd4'),
            '.github/workflows/ide-group06a-native.yml': ('19ba1e3280880a9bc48005c7572a94b8f8d3f1aa', '80edc4e117fe9c5dded1924ad87d186a4892e3c0'),
        }
        self.assertEqual({r['workflow'] for r in rows if 'successorGitBlob' in r},
                         set(reviewed_successors))
        for r in rows:
            self.assertFalse(r['dispatchAuthorized'] or r['retireAuthorized'] or r['parityEstablished'] or r['parentRunObserved'])
            self.assertTrue(set(r['relatedObservedCohortPaths'])<=automatic)
            if r['workflow'] in reviewed_successors:
                historical, successor = reviewed_successors[r['workflow']]
                self.assertEqual(r['gitBlob'], historical)
                self.assertEqual(r.get('successorGitBlob'), successor)
            else:
                self.assertNotIn('successorGitBlob', r)
            blob=subprocess.check_output(['git','hash-object','--',r['workflow']],cwd=ROOT,text=True).strip()
            self.assertEqual(blob,r.get('successorGitBlob',r['gitBlob']))
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
