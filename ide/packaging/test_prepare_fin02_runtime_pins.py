import hashlib
import tempfile
import unittest
from pathlib import Path
from prepare_fin02_runtime_pins import Denied, sha_file, tree_sha, verify

class Fin02RuntimePinTests(unittest.TestCase):
    def test_runtime_tree_digest_is_strictly_content_based(self):
        with tempfile.TemporaryDirectory() as d:
            r=Path(d)
            (r/"bin").mkdir()
            (r/"bin/node.exe").write_bytes(b"offline-reviewed-node")
            (r/"LICENSES").mkdir()
            (r/"LICENSES/node.txt").write_text("licensed upstream original")
            first=tree_sha(r,"runtime")
            (r/"bin/node.exe").write_bytes(b"tampered")
            self.assertNotEqual(first,tree_sha(r,"runtime"))

    def test_links_and_missing_payload_refused(self):
        with tempfile.TemporaryDirectory() as d:
            r=Path(d)
            (r/"target").write_bytes(b"original")
            (r/"link").symlink_to(r/"target")
            with self.assertRaises(Denied):
                tree_sha(r,"runtime")
            with self.assertRaises((Denied,OSError)):
                verify(r,"windows",{})

    def test_windows_tree_depth_first_does_not_equal_flat_sort(self):
        with tempfile.TemporaryDirectory() as d:
            r=Path(d)
            (r/"a").mkdir()
            (r/"a"/"b").write_bytes(b"b")
            (r/"a.txt").write_bytes(b"a")
            self.assertEqual(len(tree_sha(r,"win_tree")),64)
            self.assertNotEqual(tree_sha(r,"win_tree"),tree_sha(r,"runtime"))

if __name__=="__main__":
    unittest.main()
