"""The P06 scoped installed selector never accepts unreviewed native locks."""
import hashlib
import unittest

from verify_p06_checkout_lock import (
    ORIGINAL_CARGO_BLOB,
    REVIEWED_NPM_BLOB,
    blob_sha1,
    select_scoped_review,
    IDE05_SUCCESSOR,
    IDE_MERGE02_SUCCESSOR,
    LockDenied,
)


class CheckoutScopeTests(unittest.TestCase):
    def test_git_blob_sha_is_raw_bytes_not_normalized(self):
        data = b"fixed\nline\n"
        self.assertEqual(
            blob_sha1(data),
            hashlib.sha1(b"blob 11\0fixed\nline\n").hexdigest(),
        )
        self.assertNotEqual(blob_sha1(data), blob_sha1(data.replace(b"\n", b"\r\n")))

    def test_only_three_immutable_successor_identities(self):
        for digest, expected in (
            (ORIGINAL_CARGO_BLOB, "original"),
            (IDE05_SUCCESSOR["newCargoBlob"], "ide05"),
            (IDE_MERGE02_SUCCESSOR["newCargoBlob"], "ide-merge02"),
        ):
            self.assertEqual(
                select_scoped_review(
                    digest, digest, REVIEWED_NPM_BLOB, REVIEWED_NPM_BLOB
                ),
                expected,
            )

    def test_unknown_cargo_and_npm_are_denied(self):
        for cargo, npm, expected_error in (
            ("f" * 40, REVIEWED_NPM_BLOB, "p06_unreviewed_cargo_blob"),
            (ORIGINAL_CARGO_BLOB, "e" * 40, "p06_unreviewed_npm_blob"),
        ):
            with self.assertRaisesRegex(LockDenied, expected_error):
                select_scoped_review(cargo, cargo, npm, npm)

    def test_uncommitted_or_filtered_lock_data_is_denied(self):
        for head_cargo, worktree_cargo, head_npm, worktree_npm in (
            (ORIGINAL_CARGO_BLOB, "0" * 40, REVIEWED_NPM_BLOB, REVIEWED_NPM_BLOB),
            (ORIGINAL_CARGO_BLOB, ORIGINAL_CARGO_BLOB, REVIEWED_NPM_BLOB, "0" * 40),
            (IDE_MERGE02_SUCCESSOR["newCargoBlob"], ORIGINAL_CARGO_BLOB, REVIEWED_NPM_BLOB, REVIEWED_NPM_BLOB),
        ):
            with self.assertRaisesRegex(
                LockDenied, "p06_checked_out_lock_bytes_differ_from_head"
            ):
                select_scoped_review(
                    head_cargo, worktree_cargo, head_npm, worktree_npm
                )


if __name__ == "__main__":
    unittest.main()
