"""R2 controlled IPC lock successor hostile tests, with no approval inheritance."""
import json
from pathlib import Path
import unittest
from verify_ide_precore_lock_successor import check, Denied, REVIEW_FILE

ROOT = Path(__file__).resolve().parents[2]

class PrecoreR2LockReview(unittest.TestCase):
    def test_exact_original_source_and_successor(self):
        self.assertTrue(check(ROOT))

    def test_one_byte_change_refused(self):
        for path in ('ide/host/Cargo.lock','ide/app/src-tauri/Cargo.lock'):
            original=(ROOT/path).read_bytes()
            for candidate in (original+b' ', original.replace(b'"rustix"',b'"rustiy"',1), original.replace(b'"zeroize"',b'"zer0ize"',1)):
                if candidate == original:
                    continue
                with self.subTest(path=path), self.assertRaises(Denied):
                    check(ROOT, {path:candidate})

    def test_no_review_field_can_be_self_authorized(self):
        review=json.loads((ROOT/REVIEW_FILE).read_text())
        for key,bad in [('releaseApproved',True),('externalPackageChanges',1),('hostSha256','0'*64),('originalMERGE02CUnchanged',False)]:
            changed=dict(review);changed[key]=bad
            with self.subTest(key=key), self.assertRaisesRegex(Denied, 'successor_review_not_independently_pinned'):
                check(ROOT, review_override=changed)

    def test_historical_verifier_still_refuses_successor_as_original(self):
        from verify_ide_merge02c_native_locks import PINS, Denied as OriginalDenied, verify_bytes
        host=(ROOT/'ide/host/Cargo.lock').read_bytes()
        with self.assertRaises(OriginalDenied):
            verify_bytes(host,PINS['ide/host/Cargo.lock'][1],PINS['ide/host/Cargo.lock'][2],PINS['ide/host/Cargo.lock'][3])

if __name__ == '__main__': unittest.main()
