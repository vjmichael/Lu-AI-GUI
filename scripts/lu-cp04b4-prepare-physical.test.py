"""Offline preparation contracts; synthetic ZIPs are NOT runtime evidence."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location('physical', Path(__file__).with_name('lu-cp04b4-prepare-physical.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def archive(members):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w') as z:
        for k, v in members.items():
            z.writestr(k, v)
    return zipfile.ZipFile(buffer)


class PreparationTests(unittest.TestCase):
    def candidate(self):
        files = {k: f'test only {k}'.encode() for k in ['chrome.exe', 'node.exe', 'lu_browser_worker.mjs',
                 'node_modules/playwright/index.mjs', 'node_modules/playwright-core/index.mjs']}
        files.update({f'fixture/f{i}.txt': b'fixture' for i in range(477)})
        manifest = {'schema': 1, 'runtime': 'lu-browser', 'status': 'candidate', 'platform': 'win32', 'arch': 'x64',
                    'playwrightVersion': '1.64.1', 'files': [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k,v in files.items()]}
        raw = json.dumps(manifest).encode(); manifest_sha = sha(raw)
        files.update({'manifest.json': raw, 'manifest.sha256': manifest_sha.encode(),
                      'candidate-summary.json': json.dumps({'status':'candidate','platform':'win32','arch':'x64',
                      'playwrightVersion':'1.64.1','files':482,'manifestSha256':manifest_sha}).encode()})
        return files, manifest_sha, sha(files['lu_browser_worker.mjs'])

    def test_complete_candidate_stays_unapproved(self):
        files, m, w = self.candidate()
        with patch.multiple(p, MANIFEST=m, WORKER=w), archive(files) as z, tempfile.TemporaryDirectory() as d:
            listing = p.verify_browser(z)
            p.extract(z, listing, Path(d)/'candidate')
            self.assertEqual(len(listing), 485)
            self.assertEqual(json.loads((Path(d)/'candidate/manifest.json').read_bytes())['status'], 'candidate')
            self.assertFalse((Path(d)/'review.json').exists())

    def test_tampering_and_extra_payloads_rejected(self):
        files, m, w = self.candidate()
        for change in ({'chrome.exe': b'modified'}, {'extra.exe': b'extra'}, {'manifest.sha256': b'0'*64}):
            with self.subTest(change=change), patch.multiple(p, MANIFEST=m, WORKER=w), archive(files | change) as z:
                with self.assertRaises(ValueError): p.verify_browser(z)

    def test_wrong_worker_rejected(self):
        files, m, w = self.candidate()
        with patch.multiple(p, MANIFEST=m, WORKER='0'*64), archive(files) as z:
            with self.assertRaisesRegex(ValueError, 'Stale'): p.verify_browser(z)

    def test_path_aliases_rejected(self):
        for members in ({'../evil': b'x'}, {'a.txt': b'a', 'A.txt': b'b'}, {'CON.txt': b'a'},
                        {'a': b'a', 'a/b': b'b'}, {'a.': b'a'}, {'C:/evil': b'a'}):
            with self.subTest(members=members), archive(members) as z:
                with self.assertRaises(ValueError): p.index(z)

    def test_engine_actual_archive(self):
        path = os.environ.get('LU_CP04_PHYSICAL_ENGINE_ARCHIVE')
        if not path: self.skipTest('requires original engine ZIP')
        self.assertEqual(p.file_digest(Path(path)), p.ENGINE_ZIP)
        with zipfile.ZipFile(path) as z:
            self.assertEqual(len(p.verify_engine(z)), 5)


if __name__ == '__main__': unittest.main(verbosity=2)
