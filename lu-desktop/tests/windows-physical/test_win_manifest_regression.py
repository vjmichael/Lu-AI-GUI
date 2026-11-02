#!/usr/bin/env python3
"""Fail-closed resource tests for a built Lu Windows PE.

Pass LU_WIN_DESKTOP_EXE to validate an actual cross-built or native EXE.
"""
from pathlib import Path
import os,sys,tempfile,unittest
SCRIPTS=Path(__file__).resolve().parents[2]/'scripts'
sys.path.insert(0,str(SCRIPTS))
from verify_win_v6_manifest import PE, ManifestError, verify
from build_win_manifest_res import compiled_manifest
class WindowsManifestRegression(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        x=os.environ.get('LU_WIN_DESKTOP_EXE')
        if not x:raise unittest.SkipTest('set LU_WIN_DESKTOP_EXE to the built Lu Windows executable')
        cls.path=Path(x)
        if not cls.path.is_file():raise AssertionError('Windows desktop EXE missing')
        cls.binary=cls.path.read_bytes()
    def test_gui_common_controls_v6(self):
        self.assertIn('Common Controls v6',verify(self.path))
    def test_wrong_version_rejected(self):
        old=b'version="6.0.0.0"'
        self.assertEqual(self.binary.count(old),1)
        with tempfile.TemporaryDirectory() as d:
            bad=Path(d)/'v5.exe';bad.write_bytes(self.binary.replace(old,b'version="5.0.0.0"'))
            with self.assertRaisesRegex(ManifestError,'Common Controls v6'):verify(bad)
    def test_missing_pe_resource_rejected(self):
        pe=PE(self.path)
        self.assertNotEqual(pe.dirs[2],(0,0))
        with tempfile.TemporaryDirectory() as d:
            bad=Path(d)/'no-resource.exe';blob=bytearray(self.binary)
            peoff=int.from_bytes(blob[0x3c:0x40],'little')
            directory=peoff+24+112+8*2
            blob[directory:directory+8]=bytes(8);bad.write_bytes(blob)
            with self.assertRaisesRegex(ManifestError,'missing PE resource directory'):verify(bad)
    def test_console_exe_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            bad=Path(d)/'console.exe';blob=bytearray(self.binary)
            peoff=int.from_bytes(blob[0x3c:0x40],'little')
            blob[peoff+24+68:peoff+24+70]=(3).to_bytes(2,'little')
            bad.write_bytes(blob)
            with self.assertRaisesRegex(ManifestError,'GUI'):verify(bad)
    def test_truncated_exe_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            bad=Path(d)/'short.exe';bad.write_bytes(b'MZ'+bytes(14))
            with self.assertRaises(ManifestError):verify(bad)
    def test_resource_generator_rejects_wrong_v6(self):
        xml=(SCRIPTS/'lu-desktop.manifest').read_bytes()
        self.assertTrue(compiled_manifest(xml).startswith(bytes([0,0,0,0,32,0,0,0])))
        with self.assertRaisesRegex(ValueError,'Common Controls'):
            compiled_manifest(xml.replace(b'version="6.0.0.0"',b'version="5.0.0.0"'))
if __name__=='__main__':unittest.main(verbosity=2)
