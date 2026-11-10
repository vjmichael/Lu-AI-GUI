#!/usr/bin/env python3
"""Source-only tests of the strict PE RT_MANIFEST decoder.

These do NOT replace verification on the actual linked Windows executable.
"""
import unittest
from pathlib import Path
from verify_win_v6_manifest import ManifestError,parse_manifest_resource,PE

MANIFEST=Path(__file__).with_name("lu-desktop.manifest").read_bytes()

class ManifestDecoderTests(unittest.TestCase):
    def test_bounded_unambiguous_resource_encodings(self):
        variants=[
            MANIFEST,MANIFEST+b'\x00',b'\xef\xbb\xbf'+MANIFEST,
            MANIFEST.decode('utf-8').encode('utf-16'),
            MANIFEST.decode('utf-8').encode('utf-16-le')+b'\x00\x00'
        ]
        for blob in variants:
            with self.subTest(start=blob[:6],size=len(blob)):
                root=parse_manifest_resource(blob)
                self.assertEqual(root.tag,
                  '{urn:schemas-microsoft-com:asm.v1}assembly')
    def test_malformed_or_multi_document_resource_rejected(self):
        for invalid in [
            MANIFEST+b'\x00\x00',
            MANIFEST+b'<evil/>',
            MANIFEST+b'\x00garbage',
            b'<?xml?><a/><a/>',
            b'\xff\xfe\x00',
            MANIFEST[:-1],
            MANIFEST+b'\x00\x00\x00'
        ]:
            with self.subTest(size=len(invalid),end=invalid[-9:]):
                with self.assertRaises(ManifestError):
                    parse_manifest_resource(invalid)
    def test_non_pe_binary_never_passes_manifest_gate(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as folder:
            file=Path(folder)/"fake.exe"
            file.write_bytes(MANIFEST)
            with self.assertRaises(ManifestError):
                PE(file)

if __name__=="__main__":
    unittest.main()
