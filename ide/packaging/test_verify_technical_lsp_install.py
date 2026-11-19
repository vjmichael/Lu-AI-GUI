import hashlib
import json
from pathlib import Path
import os
import shutil
import tempfile
import unittest

import test_stage_complete_native_lsp_candidate as original_tests
from stage_complete_native_lsp_candidate import candidate
from verify_technical_lsp_install import verify_resources, verify_from_archives, RECEIPT, MARKER, WARNING, strict_json


class TechnicalInstalledResourceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.platform = 'windows-x86_64'
        self.m, self.n, archives, notices = original_tests.OriginalFullNativeBundleTests().fixture(self.root, self.platform)
        self.archives, self.original_notices = archives, notices
        self.ref = self.root/'reference'
        self.installed = self.root/'installed'
        candidate(self.m, self.n, archives, notices, self.ref, self.platform)
        (self.ref/MARKER).write_bytes(WARNING)
        shutil.copytree(self.ref,self.installed)
    def verify(self):
        return verify_resources(self.ref,self.installed,self.m,self.n,self.platform)
    def test_exact_complete_resources_pass(self):
        self.assertGreater(self.verify(), 10)
    def test_tampered_binary_denied(self):
        (self.installed/'clangd/bin/clangd.exe').write_bytes(b'fake')
        with self.assertRaises(ValueError): self.verify()
    def test_changed_notice_same_file_count_denied(self):
        next((self.installed/'clangd/NOTICES').iterdir()).write_bytes(b'fake')
        with self.assertRaises(ValueError): self.verify()
    def test_missing_header_denied(self):
        (self.installed/'clangd/lib/clang/23/include/stddef.h').unlink()
        with self.assertRaises((OSError,ValueError)): self.verify()
    def test_extra_file_denied(self):
        (self.installed/'clangd/bin/unexpected.dll').write_bytes(b'fake')
        with self.assertRaises(ValueError): self.verify()
    def test_forged_installed_receipt_denied(self):
        r = json.loads((self.installed/RECEIPT).read_bytes())
        r['releaseApproved'] = True
        (self.installed/RECEIPT).write_text(json.dumps(r))
        with self.assertRaises(ValueError): self.verify()
    def test_forged_matching_original_receipt_authority_denied(self):
        r = json.loads((self.ref/RECEIPT).read_bytes())
        r['protectedLuProtocolAccepted'] = True
        for path in (self.ref,self.installed): (path/RECEIPT).write_text(json.dumps(r))
        with self.assertRaises(ValueError): self.verify()
    def test_forged_matching_notice_hash_denied(self):
        rel = next((self.ref/'clangd/NOTICES').iterdir()).relative_to(self.ref/'clangd').as_posix()
        r = json.loads((self.ref/RECEIPT).read_bytes())
        raw=b'fake-notice'; r['providers']['clangd']['files'][rel]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
        for path in (self.ref,self.installed):
            (path/'clangd'/rel).write_bytes(raw)
            (path/RECEIPT).write_text(json.dumps(r))
        with self.assertRaises(ValueError): self.verify()
    def test_marker_required_and_exact(self):
        (self.installed/MARKER).write_text('approved')
        with self.assertRaises(ValueError): self.verify()
    def test_duplicate_receipt_field_denied(self):
        (self.ref/RECEIPT).write_text('{"schema":1,"schema":2}')
        with self.assertRaises(ValueError): self.verify()
    def test_linked_directory_denied(self):
        target=self.installed/'clangd/bin'; shutil.rmtree(target)
        try: target.symlink_to(self.ref/'clangd/bin',target_is_directory=True)
        except OSError: self.skipTest('native symlink privilege unavailable')
        with self.assertRaises(ValueError): self.verify()
    def test_independent_archive_reconstruction_denies_forged_binary_in_both_trees(self):
        v=lambda: verify_from_archives(self.ref,self.installed,self.m,self.n,self.archives,self.original_notices,self.platform)
        self.assertGreater(v(),10)
        rel='bin/clangd.exe'; raw=b'forged executable'
        r=json.loads((self.ref/RECEIPT).read_bytes())
        h=hashlib.sha256(raw).hexdigest()
        r['providers']['clangd']['files'][rel]={'sha256':h,'bytes':len(raw)}
        r['providers']['clangd']['originalExecutableSha256']=h
        for path in (self.ref,self.installed):
            (path/'clangd'/rel).write_bytes(raw)
            (path/RECEIPT).write_text(json.dumps(r))
        # A copied self-consistent receipt would pass naive tree checks.
        self.assertGreater(self.verify(),10)
        with self.assertRaises(ValueError): v()
    def test_forged_archive_pin_denied(self):
        r=json.loads((self.ref/RECEIPT).read_bytes())
        r['providers']['clangd']['archiveSha256']='0'*64
        for path in (self.ref,self.installed): (path/RECEIPT).write_text(json.dumps(r))
        with self.assertRaises(ValueError): self.verify()

if __name__=='__main__': unittest.main()
