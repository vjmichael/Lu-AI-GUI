"""Fixed Linux CMake profile packaging tests; no fixture executable is run."""
from pathlib import Path
import hashlib
import io
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import provision_cmake_candidate as p


class LinuxCMakeProfileTests(unittest.TestCase):
    def test_exact_compiler_and_resource_selection(self):
        for path in ('bin/clang-23', 'bin/lld', 'lib/clang/23/include/stddef.h'):
            self.assertTrue(p.allow_component('llvm', path, 'linux-x64'), path)
        for path in ('bin/clang-22', 'bin/clangd', 'bin/clang-tidy', 'bin/lldb',
                     'bin/clang-23.exe', 'lib/libLLVM.a', 'include/clang/x.h',
                     'lib/clang/24/include/stddef.h', 'include/llvm/Support/LICENSE.TXT'):
            self.assertFalse(p.allow_component('llvm', path, 'linux-x64'), path)
        self.assertTrue(p.allow_component('llvm','bin/clang.exe','windows-x64'))

    def test_regular_aliases_and_exact_copy_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            stage=Path(temp); remaining=[11]
            p.add_entry(stage,'llvm','bin/clang-23',io.BytesIO(b'12345'),remaining,'linux-x64')
            p.add_entry(stage,'llvm','bin/clangd',io.BytesIO(b'ignore'),remaining,'linux-x64')
            self.assertFalse((stage/'bin/clangd').exists())
            self.assertEqual(remaining,[6]); self.assertEqual((stage/'bin/clang').read_bytes(),b'12345')
            p.bounded_copy(stage/'bin/clang',stage/'bin/clang++',remaining)
            self.assertEqual(remaining,[1]); self.assertFalse((stage/'bin/clang++').is_symlink())
            with self.assertRaises(p.Refused):
                p.bounded_copy(stage/'bin/clang',stage/'bin/oversize',remaining)
            self.assertFalse((stage/'bin/oversize').exists())

    def test_pinned_license(self):
        self.assertEqual(hashlib.sha256(p.LLVM_LINUX_LICENSE.read_bytes()).hexdigest(),p.LLVM_LINUX_LICENSE_SHA)

if __name__=='__main__': unittest.main()
