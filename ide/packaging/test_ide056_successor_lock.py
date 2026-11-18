"""IDE-05.6 exact successor-only Cargo lock review; original P06 stays immutable."""
import copy
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
from verify_p06_candidate_lock import check, LockDenied, reviewed_ide05_successor, sha

ROOT=HERE.parent.parent
REVIEW=json.loads((HERE/"p06-reviewed-lock-graphs.json").read_text(encoding="utf-8"))
SUCCESSOR=json.loads((HERE/"ide05-reviewed-lock-successor.json").read_text(encoding="utf-8"))
CARGO=(ROOT/"ide/app/src-tauri/Cargo.lock").read_bytes()

class Ide05SuccessorAcceptance(unittest.TestCase):
    def test_original_review_still_rejects_current_drift(self):
        for platform in ("ubuntu-24.04","windows-2022"):
            with self.assertRaisesRegex(LockDenied,"candidate_cargo_lock_not_reviewed"):
                check(ROOT,platform)
    def test_only_named_successor_admits_exact_bytes_without_release(self):
        # The original IDE05 review remains a positive test on its exact
        # historical bytes, not an approval of the later desktop rand delta.
        historical = subprocess.check_output(
            ["git", "cat-file", "blob", SUCCESSOR["newCargoBlob"]], cwd=ROOT)
        self.assertEqual(sha(historical), SUCCESSOR["newCargoSha256"])
        self.assertNotEqual(CARGO, historical)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for rel, data in (
                ("ide/app/src-tauri/Cargo.lock", historical),
                ("ide/app/package-lock.json", (ROOT/"ide/app/package-lock.json").read_bytes()),
                ("ide/supervisor/Cargo.toml", (ROOT/"ide/supervisor/Cargo.toml").read_bytes()),
            ):
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            for platform in ("ubuntu-24.04","windows-2022"):
                with self.assertRaisesRegex(LockDenied, "ide05_successor_lock_byte_delta_unreviewed"):
                    check(ROOT,platform,ide05_successor=SUCCESSOR)
                r=check(root,platform,review=REVIEW,ide05_successor=SUCCESSOR)
                self.assertTrue(r["ide05ScopedSuccessorAccepted"])
                self.assertEqual(r["cargoLockSha256"],SUCCESSOR["newCargoSha256"])
                self.assertFalse(r["releaseApproved"])
                self.assertFalse(r["independentHumanApproval"])
    def test_changing_successor_json_cannot_self_authorize(self):
        altered=copy.deepcopy(SUCCESSOR)
        altered["newCargoSha256"]=sha(b"malicious Cargo.lock")
        with self.assertRaisesRegex(LockDenied,"ide05_successor_not_independently_pinned"):
            reviewed_ide05_successor(ROOT,CARGO,REVIEW,altered)
    def test_changed_other_package_cannot_be_approved(self):
        edited=CARGO.replace(b'name = "adler2"',b'name = "adl3r2"',1)
        with self.assertRaisesRegex(LockDenied,"ide05_successor_lock_byte_delta_unreviewed"):
            reviewed_ide05_successor(ROOT,edited,REVIEW,SUCCESSOR)
    def test_changed_supervisor_dependency_rejected(self):
        edited=CARGO.replace(b' "base64 0.22.1",',b' "base64 0.22.2",',1)
        with self.assertRaisesRegex(LockDenied,"ide05_successor_lock_byte_delta_unreviewed"):
            reviewed_ide05_successor(ROOT,edited,REVIEW,SUCCESSOR)
    def test_changed_original_review_denied(self):
        tampered=copy.deepcopy(REVIEW)
        tampered["sourceLockBlobSha"]["cargo"]="0"*40
        with self.assertRaisesRegex(LockDenied,"ide05_successor_prior_review_mismatch"):
            reviewed_ide05_successor(ROOT,CARGO,tampered,SUCCESSOR)
    def test_cannot_claim_signature_or_release_approval(self):
        tampered=copy.deepcopy(SUCCESSOR)
        tampered["releaseApproved"]=True
        with self.assertRaisesRegex(LockDenied,"ide05_successor_not_independently_pinned"):
            reviewed_ide05_successor(ROOT,CARGO,REVIEW,tampered)

if __name__=="__main__":
    unittest.main()
