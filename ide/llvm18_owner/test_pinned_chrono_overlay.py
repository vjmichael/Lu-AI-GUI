#!/usr/bin/env python3
"""FIN-04R-1: fast exact-source overlay contract (NOT native LLVM acceptance).

Original pinned Windows job 130020178608 failed MSVC C2039/C3861 because
LLVM 18.1.8 lldb-dap/ProgressEvent.h referenced std::chrono::system_clock
without directly including <chrono>. Prove our pinned overlay repairs it
once and fails closed on changed upstream anchors/commit.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase, main
from unittest.mock import patch
import re
import apply_llvm18_owner_hook as overlay

CPP = """#include "DebuggerThread.h"
DebuggerThread()
      m_is_shutting_down(false) {
  m_debugging_ended_event = ::CreateEvent(nullptr, TRUE, FALSE, nullptr);
}
void loop() {
    if (wait_result) {
      DWORD continue_status = DBG_CONTINUE;
      ::ContinueDebugEvent(dbe.dwProcessId, dbe.dwThreadId, continue_status);
    }
}
"""
PROGRESS = """#pragma once
#include <atomic>
#include <mutex>
std::chrono::duration<double> started = std::chrono::system_clock::now().time_since_epoch();
"""


class PinnedChronoOverlayTests(TestCase):
    def fixture(self, root, progress_source=PROGRESS):
        cpp = root / overlay.SOURCE
        cpp.parent.mkdir(parents=True)
        cpp.write_text(CPP, encoding="utf-8")
        progress = root / overlay.DAP_PROGRESS_HEADER
        progress.parent.mkdir(parents=True)
        progress.write_text(progress_source, encoding="utf-8")
        process = root / overlay.WINDOWS_PROCESS_SOURCE
        process.parent.mkdir(parents=True)
        process.write_text(overlay.WINDOWS_PROCESS_ORIGINAL, encoding="utf-8")
        return cpp, progress

    def test_one_direct_include_and_original_event_hook(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            cpp, progress = self.fixture(root)
            with patch.object(overlay.subprocess, "check_output",
                              return_value=overlay.LLVM18_PIN):
                overlay.apply(root)
            text = progress.read_text(encoding="utf-8")
            self.assertEqual(text.count("#include <chrono>"), 1)
            self.assertEqual(text.count("std::chrono::system_clock::now()"), 1)
            source = cpp.read_text(encoding="utf-8")
            self.assertIn(
                '#ifndef NOMINMAX\n#define NOMINMAX 1\n#endif\n'
                '#include <winsock2.h>\n#include "DebuggerThread.h"',
                source,
            )
            self.assertEqual(source.count("#define NOMINMAX 1"), 1)
            self.assertIn('#include "LuIdeOwnerEvent.h"', source)
            self.assertIn("lu_ide_owner::OnReceived(dbe);", cpp.read_text())
            self.assertIn("lu_ide_owner::OnContinued(dbe, continued);", cpp.read_text())
            self.assertEqual((root / overlay.HEADER).read_bytes(),
                             Path(overlay.__file__).with_name("LuIdeOwnerEvent.h").read_bytes())

    def test_native_path_repair_is_fixed_tools_only_and_preserves_unrelated_denials(self):
        original = overlay.WINDOWS_PROCESS_ORIGINAL
        patched = overlay.patch_windows_process(original)
        self.assertIn("::GetLongPathNameW(ModuleName, LongModuleName, MAX_PATH)", patched)
        self.assertIn("Error != ERROR_ACCESS_DENIED", patched)
        self.assertIn('L"lldb-dap.exe", -1,', patched)
        self.assertIn('L"llvm-symbolizer.exe", -1, TRUE', patched)
        self.assertIn("(!Dap && !Symbolizer)", patched)
        self.assertIn("::WriteFile(::GetStdHandle(STD_ERROR_HANDLE)", patched)
        self.assertIn("CSTR_EQUAL", patched)
        self.assertIn("return mapWindowsError(Error);", patched)
        self.assertIn("return mapWindowsError(ERROR_INSUFFICIENT_BUFFER);", patched)
        self.assertIn("if (LongLength >= MAX_PATH)", patched)
        self.assertIn("UTF16ToUTF8(ResolvedName, LongLength, Filename)", patched)
        self.assertNotIn("SetNamedSecurityInfo", patched)
        with self.assertRaisesRegex(RuntimeError, "upstream LLDB source anchor changed"):
            overlay.patch_windows_process(original.replace("GetLongPathNameW", "Changed"))
        with self.assertRaisesRegex(RuntimeError, "upstream LLDB source anchor changed"):
            overlay.patch_windows_process(original + original)

    def test_changed_upstream_header_denied(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, PROGRESS.replace("#include <atomic>",
                                                 "#include <chrono>"))
            with patch.object(overlay.subprocess, "check_output",
                              return_value=overlay.LLVM18_PIN):
                with self.assertRaisesRegex(RuntimeError,
                                            "upstream LLVM 18 DAP chrono defect anchor changed"):
                    overlay.apply(root)

    def test_unpinned_commit_denied(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with patch.object(overlay.subprocess, "check_output",
                              return_value="0" * 40):
                with self.assertRaisesRegex(RuntimeError, "unreviewed LLVM source commit"):
                    overlay.apply(root)
            self.assertFalse((root / overlay.HEADER).exists())

    def test_missing_original_atomic_anchor_denied(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, PROGRESS.replace("#include <atomic>", "#include <array>"))
            with patch.object(overlay.subprocess, "check_output",
                              return_value=overlay.LLVM18_PIN):
                with self.assertRaisesRegex(RuntimeError,
                                            "upstream LLDB source anchor changed"):
                    overlay.apply(root)


class ProducerConsumerContractTests(TestCase):
    """Source-level scheduling and custody checks, not Windows native proof."""
    def setUp(self):
        root = Path(__file__).resolve().parents[2]
        self.producer = (root / ".github/workflows/ide-dev04-5d5-owner-event-native.yml").read_text()
        self.consumer = (root / ".github/workflows/ide-wo6-native-os-acceptance.yml").read_text()
        self.transport = (root / "ide/llvm18_owner/Transfer-PinnedOwnerBundle.ps1").read_text()

    @staticmethod
    def pr_paths(text):
        block = text.split("  pull_request:\n", 1)[1].split("  workflow_dispatch:", 1)[0]
        return set(re.findall(r"(?m)^      - '([^']+)'$", block))

    def test_all_consumer_paths_schedule_exact_head_producer(self):
        self.assertTrue(self.pr_paths(self.consumer))
        self.assertLessEqual(self.pr_paths(self.consumer), self.pr_paths(self.producer))
        self.assertIn("ide/host/src/**", self.pr_paths(self.producer))
        for text in (self.producer, self.consumer):
            self.assertIn("ref: ${{ github.event.pull_request.head.sha || github.sha }}", text)
            self.assertIn("  workflow_dispatch:", text)

    def test_bounded_wait_has_producer_and_native_budget(self):
        producer = int(re.search(r"(?m)^    timeout-minutes: (\d+)$", self.producer)[1])
        consumer = int(re.search(r"(?m)^    timeout-minutes: (\d+)$", self.consumer)[1])
        wait = int(re.search(r"AddMinutes\((\d+)\)", self.transport)[1])
        self.assertLessEqual(producer, wait)
        self.assertGreaterEqual(consumer - wait, 90)
        self.assertLessEqual(consumer, 360)

    def test_native_producer_order_and_no_substitute(self):
        markers = (
            "Fail fast on existing formatting and exact overlay contracts before LLVM",
            "Stage exact reviewed offline Windows SDK and MSVC toolchain",
            "Require a registered original Visual Studio 17.14 generator before LLVM",
            "Compile real source-pinned LLVM18 DebuggerThread and produce hashed owner binary",
            "Produce exact-head native bundle transport manifest (not acceptance)",
            "Publish measured LLVM18 bundle for original WO6 native acceptance",
            "Real SCM/AppContainer owner stop, PE/PDB, cancellation and journal restart",
        )
        order = [self.producer.index(m) for m in markers]
        self.assertEqual(order, sorted(order))
        self.assertIn("if-no-files-found: error", self.producer)
        self.assertIn("Transfer-PinnedOwnerBundle.ps1 -Mode Receive", self.consumer)
        self.assertIn("producer completed without verified candidate bundle", self.transport)
        self.assertIn("bounded original-producer artifact wait expired", self.transport)
        self.assertIn("$manifest.producerAttempt -cne [string]$producer.run_attempt", self.transport)
        self.assertIn("Assert-Inventory $actual @($manifest.files)", self.transport)
        self.assertIn("VERIFIED_NOT_KERNEL_ACCEPTANCE", self.transport)


if __name__ == "__main__":
    main()
