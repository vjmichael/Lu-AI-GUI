"""Positive and hostile tests for canonical upstream license retrieval."""
from pathlib import Path
import tempfile
import unittest
from urllib.error import HTTPError, URLError
from upstream_notice_receipt import SOURCES, verify_sources, NoticeDenied, download

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
    def test_bounded_transport_retry_recovers_exact_origin(self):
        url = next(iter(self.data))
        counts = []
        sleeps = []
        content = self.data[url]
        class FakeResponse:
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def geturl(self):
                return url
            def read(self, _):
                return content
        def opener(request, timeout):
            self.assertEqual(request.full_url, url)
            self.assertEqual(timeout, 20)
            counts.append(1)
            if len(counts) < 3:
                raise URLError("temporary network unavailable")
            return FakeResponse()
        self.assertEqual(download(url, opener=opener, sleeper=sleeps.append),
                         content)
        self.assertEqual(len(counts), 3)
        self.assertEqual(sleeps, [1, 2])

    def test_redirect_and_http_404_not_retried(self):
        url = next(iter(self.data))
        calls = []
        def permanently_missing(request, timeout):
            calls.append(1)
            raise HTTPError(url, 404, "not found", {}, None)
        with self.assertRaises(HTTPError):
            download(url, opener=permanently_missing,
                     sleeper=lambda _: self.fail("unexpected_retry"))
        self.assertEqual(len(calls), 1)
        class RedirectResponse:
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def geturl(self):
                return url + "/other"
            def read(self, _):
                return b"A" * 200
        with self.assertRaisesRegex(NoticeDenied, "upstream_unexpected_redirect"):
            download(url, opener=lambda *_a, **_k: RedirectResponse(),
                     sleeper=lambda _: self.fail("unexpected_retry"))

    def test_transport_failure_never_produces_license_receipt(self):
        url = next(iter(self.data))
        calls = []
        sleeps = []
        def missing(request, timeout):
            calls.append(1)
            raise URLError("unavailable")
        with self.assertRaises(URLError):
            download(url, opener=missing, sleeper=sleeps.append)
        self.assertEqual(len(calls), 3)
        self.assertEqual(sleeps, [1, 2])

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
    def test_windows_crlf_conversion_is_not_tampering(self):
        first = next(iter(self.data))
        self.data[first] = self.data[first].replace(b"\n", b"\r\n")
        self.assertTrue(verify_sources(self.root, "a"*40, self.fetch)["allSourcesRetrieved"])
if __name__ == "__main__":
    unittest.main()
