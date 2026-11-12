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
import apply_llvm18_owner_hook as overlay

CPP = """#include "DebuggerThread.h"
DebuggerThread() : m_is_shutting_down(false) {
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
            self.assertIn('#include "LuIdeOwnerEvent.h"', cpp.read_text())
            self.assertIn("lu_ide_owner::OnReceived(dbe);", cpp.read_text())
            self.assertIn("lu_ide_owner::OnContinued(dbe, continued);", cpp.read_text())
            self.assertEqual((root / overlay.HEADER).read_bytes(),
                             Path(overlay.__file__).with_name("LuIdeOwnerEvent.h").read_bytes())

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


if __name__ == "__main__":
    main()
