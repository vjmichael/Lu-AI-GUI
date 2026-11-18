from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase, main
from unittest.mock import patch
import apply_clangd20_windows_startup as overlay


class ClangdStartupTests(TestCase):
    def test_exact_leaf_and_error_only(self):
        result = overlay.patch_source(overlay.WINDOWS_PROCESS_ORIGINAL)
        self.assertIn('L"clangd.exe", -1,', result)
        self.assertIn('Error != ERROR_ACCESS_DENIED || !Clangd', result)
        self.assertIn('ResolvedName = ModuleName;', result)
        self.assertIn('LongLength = Length;', result)
        self.assertIn('if (LongLength >= MAX_PATH)', result)
        self.assertIn('LU_LLVM20_FIXED_CLANGD_IMAGE_LEAF_USED', result)
        self.assertNotIn('lldb-dap.exe', result)
        self.assertNotIn('SetNamedSecurityInfo', result)
        self.assertNotIn('llvm-symbolizer.exe', result)

    def test_anchor_missing_duplicate_and_reapply_denied(self):
        original = overlay.WINDOWS_PROCESS_ORIGINAL
        for source in (original + original, original.replace('GetLongPathNameW', 'Other'), overlay.patch_source(original)):
            with self.assertRaisesRegex(RuntimeError, 'anchor changed'):
                overlay.patch_source(source)

    def test_unpinned_dirty_and_wrong_blob_denied(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / overlay.SOURCE
            path.parent.mkdir(parents=True)
            original = overlay.WINDOWS_PROCESS_ORIGINAL
            path.write_text(original)
            for responses in (["0" * 40], [overlay.PIN, ' M file'], [overlay.PIN, '']):
                with patch.object(overlay.subprocess, 'check_output', side_effect=responses):
                    with self.assertRaises(RuntimeError):
                        overlay.apply(root)
                self.assertEqual(path.read_text(), original)

    def test_workflow_retains_real_contained_acceptance(self):
        repo = Path(__file__).resolve().parents[2]
        workflow = (repo / '.github/workflows/ide-e2-native-lsp.yml').read_text()
        build = (repo / 'ide/llvm18_owner/Build-PinnedClangdWindows.ps1').read_text()
        self.assertIn(overlay.PIN, build)
        self.assertIn('apply_clangd20_windows_startup.py', build)
        self.assertIn('clangd llvm-readobj clang-resource-headers', build)
        self.assertIn('Microsoft.VisualStudio.Component.VC.Tools.x86.x64', build)
        self.assertIn('Enter-VsDevShell -VsInstallPath $vs', build)
        self.assertIn("'-G','Ninja','-DCMAKE_BUILD_TYPE=Release'", build)
        self.assertIn('compilerSha256=(& $sha $cl)', build)
        self.assertNotIn('Visual Studio 17 2022', build)
        self.assertNotIn('Get-Command clangd.exe', workflow)
        self.assertIn('& $hostExe --cancel', workflow)
        self.assertIn('Install-ProtectedRecoveryService.ps1', workflow)
        self.assertIn('--revoke-fixed-service', workflow)
        self.assertIn('releaseApproved=$false;nativeAccepted=$false', build)


if __name__ == '__main__':
    main()
