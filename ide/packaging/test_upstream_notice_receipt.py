"""Positive and hostile tests for canonical upstream license retrieval."""
from pathlib import Path
import tempfile
import unittest
from upstream_notice_receipt import SOURCES, verify_sources, NoticeDenied

class UpstreamNoticeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.folder = self.root / "ide/packaging/upstream-licenses"
        self.folder.mkdir(parents=True)
        self.data = {}
        for name, (_, url) in SOURCES.items():
            content = (name + " canonical upstream full license notice\n" + "A"*200).encode()
            (self.folder / name).write_bytes(content)
            self.data[url] = content
    def fetch(self, url):
        return self.data[url]
    def test_exact_upstream_notice_bytes(self):
        result = verify_sources(self.root, "a"*40, self.fetch)
        self.assertTrue(result["allSourcesRetrieved"])
        self.assertEqual(len(result["sources"]), 6)
        self.assertFalse(result["releaseApproved"])
    def test_remote_notice_tampering_refused(self):
        first = next(iter(self.data))
        self.data[first] = b"unreviewed upstream rewrite" + b"Z"*220
        with self.assertRaisesRegex(NoticeDenied, "source_notice_differs_from_upstream"):
            verify_sources(self.root, "a"*40, self.fetch)
    def test_missing_local_license_refused(self):
        (self.folder / next(iter(SOURCES))).unlink()
        with self.assertRaisesRegex(NoticeDenied, "upstream_notice_missing"):
            verify_sources(self.root, "a"*40, self.fetch)
    def test_large_response_refused(self):
        first = next(iter(self.data))
        self.data[first] = b"A"*2_000_001
        with self.assertRaisesRegex(NoticeDenied, "invalid_upstream_notice_bytes"):
            verify_sources(self.root, "a"*40, self.fetch)
    def test_invalid_sha_refused(self):
        with self.assertRaisesRegex(NoticeDenied, "missing_exact_source_sha"):
            verify_sources(self.root, "main", self.fetch)
    def test_windows_crlf_conversion_is_not_accepted_as_byte_exact(self):
        first = next(iter(self.data))
        self.data[first] = self.data[first].replace(b"\n", b"\r\n")
        with self.assertRaisesRegex(NoticeDenied, "source_notice_differs_from_upstream"):
            verify_sources(self.root, "a"*40, self.fetch)
    def test_checkout_crlf_mutation_is_denied_even_if_meaning_same(self):
        first = next(iter(SOURCES))
        local = self.folder / first
        local.write_bytes(local.read_bytes().replace(b"\n", b"\r\n"))
        with self.assertRaisesRegex(NoticeDenied, "source_notice_differs_from_upstream"):
            verify_sources(self.root, "a"*40, self.fetch)
if __name__ == "__main__":
    unittest.main()
