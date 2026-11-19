import hashlib
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import stat
from pathlib import Path
from prepare_fin02_runtime_pins import Denied, sha_file, tree_sha, verify, linux_cargo

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

    def test_fixed_linux_rustdoc_pin_and_cargo_only_size_bound(self):
        paths=("/usr/bin/bwrap", "/usr/bin/cargo", "/usr/bin/rustc",
               "/usr/bin/rustdoc", "/usr/bin/x86_64-linux-gnu-gcc-14")
        class Tool:
            def __init__(self, path): self.path=path
            def is_file(self): return self.path in paths
            def is_symlink(self): return False
            def lstat(self): return metadata[self.path]
        metadata={p:SimpleNamespace(st_uid=0,st_mode=stat.S_IFREG|0o755,
                                     st_size=36148320 if p==paths[1] else 10)
                  for p in paths}
        digests={p:hashlib.sha256(p.encode()).hexdigest() for p in paths}
        with patch("prepare_fin02_runtime_pins.Path", Tool), \
             patch("prepare_fin02_runtime_pins.sha_file", side_effect=lambda p:digests[p.path]):
            expected=hashlib.sha256(b"".join(p.encode()+b"\0"+digests[p].encode()+b"\0" for p in paths)).hexdigest()
            self.assertEqual(linux_cargo(), expected)
            digests[paths[3]]="f"*64
            self.assertNotEqual(linux_cargo(), expected)
            for size, mode, uid in [(32*1024*1024+1,stat.S_IFREG|0o755,0),
                                    (10,stat.S_IFLNK|0o777,0),
                                    (10,stat.S_IFREG|0o777,0),
                                    (10,stat.S_IFREG|0o755,1)]:
                metadata[paths[3]]=SimpleNamespace(st_uid=uid,st_mode=mode,st_size=size)
                with self.assertRaises(Denied): linux_cargo()
            metadata[paths[3]]=SimpleNamespace(st_uid=0,st_mode=stat.S_IFREG|0o755,st_size=10)
            metadata[paths[1]].st_size=64*1024*1024+1
            with self.assertRaises(Denied): linux_cargo()

if __name__=="__main__":
    unittest.main()
