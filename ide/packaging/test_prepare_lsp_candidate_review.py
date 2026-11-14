import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from prepare_lsp_candidate_review import review_packet, safe_file, IDS, SCHEMA


class OfflineLspReviewTests(unittest.TestCase):
    def fixture(self, root: Path):
        bundle = root / "bundle"
        bundle.mkdir()
        rows = []
        for ident, platforms in IDS.items():
            files = {}
            for osname, filename in platforms.items():
                relative = f"{ident}/{osname}/{filename}"
                path = bundle / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(f"fixture-{ident}-{osname}".encode())
                files[osname] = relative
            notice = f"{ident}/NOTICES/LICENSE.txt"
            dst = bundle / notice
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(f"Original fixture notice for {ident}".encode())
            rows.append({
                "id": ident,
                "releaseVersion": "fixture-0.1",
                "assetByPlatform": files,
                "licenseNoticeFiles": [notice],
            })
        return bundle, {"schema": SCHEMA, "servers": rows}

    def test_complete_candidate_packet_is_measured_but_never_approved(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, proposal = self.fixture(Path(temporary))
            packet = review_packet(source, proposal)
            self.assertFalse(packet["releaseApproved"])
            self.assertFalse(packet["osContainmentVerified"])
            self.assertFalse(packet["distributorAuthorizationConfirmed"])
            self.assertFalse(packet["autoInstallOnProjectOpen"])
            self.assertFalse(packet["allowUnpinnedExecutables"])
            self.assertEqual(set(packet["servers"]), set(IDS))
            for ident, servers in packet["servers"].items():
                self.assertFalse(servers["copyrightReviewed"])
                self.assertFalse(servers["dependenciesAudited"])
                self.assertFalse(servers["licenseApproved"])
                for platform, record in servers["platforms"].items():
                    original = source / record["relativePath"]
                    self.assertEqual(record["sha256"],
                                     hashlib.sha256(original.read_bytes()).hexdigest())
                    self.assertTrue(record["bytes"] > 0)

    def test_incomplete_or_unapproved_server_cannot_be_misrepresented(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, proposal = self.fixture(Path(temporary))
            missing = copy.deepcopy(proposal)
            missing["servers"].pop()
            with self.assertRaises(ValueError):
                review_packet(source, missing)
            bad = copy.deepcopy(proposal)
            bad["servers"][0]["id"] = "pyright"
            with self.assertRaises(ValueError):
                review_packet(source, bad)
            bad = copy.deepcopy(proposal)
            bad["servers"][0]["releaseApproved"] = True
            with self.assertRaises(ValueError):
                review_packet(source, bad)
            bad = copy.deepcopy(proposal)
            bad["servers"][0]["assetByPlatform"].pop("linux-x86_64")
            with self.assertRaises(ValueError):
                review_packet(source, bad)

    def test_replacement_missing_notice_and_path_traversal_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, proposal = self.fixture(Path(temporary))
            ident = proposal["servers"][0]["id"]
            asset = proposal["servers"][0]["assetByPlatform"]["windows-x86_64"]
            (source / asset).unlink()
            with self.assertRaises(ValueError):
                review_packet(source, proposal)
            (source / asset).write_bytes(b"valid fixture")
            notice = proposal["servers"][0]["licenseNoticeFiles"][0]
            (source / notice).unlink()
            with self.assertRaises(ValueError):
                review_packet(source, proposal)
            (source / notice).write_bytes(b"notice")
            for relative in ("../secret", "/etc/passwd", "c:/abs", "hello\\file",
                             "my file", "", "x" * 1025):
                with self.assertRaises(ValueError, msg=relative):
                    safe_file(source, relative, 1024)

    def test_symlinked_ancestor_and_sparse_empty_assets_are_denied(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, proposal = self.fixture(Path(temporary))
            row = proposal["servers"][0]
            path = source / row["assetByPlatform"]["linux-x86_64"]
            path.write_bytes(b"")
            with self.assertRaises(ValueError):
                review_packet(source, proposal)
            path.write_bytes(b"fixed")
            rootdir = source / row["id"]
            saved = rootdir.with_name(rootdir.name + "-original")
            rootdir.rename(saved)
            try:
                rootdir.symlink_to(saved, target_is_directory=True)
            except (OSError, NotImplementedError):
                return
            with self.assertRaises(ValueError):
                review_packet(source, proposal)


if __name__ == "__main__":
    unittest.main()
