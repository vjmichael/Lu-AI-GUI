#!/usr/bin/env python3
"""Patch a reviewed LLVM 18.1.8 Windows LLDB debugger event loop.

Requires the real LLVM source checkout at the exact *commit*, not an
unverified system binary or a downloaded DLL. Emits a reversible, small
first-party overlay with an explicit failure on any changed upstream anchor.
The resulting LLDB MUST be rebuilt, measured, bundled and native-tested.
"""
from pathlib import Path
import subprocess
import sys

LLVM18_PIN = "3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff"
SOURCE = "lldb/source/Plugins/Process/Windows/Common/DebuggerThread.cpp"
HEADER = "lldb/source/Plugins/Process/Windows/Common/LuIdeOwnerEvent.h"
# LLVM 18.1.8 upstream's DAP ProgressEvent.h uses std::chrono::system_clock
# without directly including <chrono>. Native Windows 2022/MSVC C2039 and
# C3861 (original job 130020178608) proved this compile failure. Make the
# smallest exact-pin, source-reviewed include fix; never alter unpinned LLVM.
DAP_PROGRESS_HEADER = "lldb/tools/lldb-dap/ProgressEvent.h"


def replace_one(source: str, original: str, replacement: str) -> str:
    if source.count(original) != 1:
        raise RuntimeError("DENIED: upstream LLDB source anchor changed")
    return source.replace(original, replacement, 1)


def apply(root: Path) -> None:
    root = root.resolve(strict=True)
    actual = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual != LLVM18_PIN:
        raise RuntimeError("DENIED: unreviewed LLVM source commit " + actual)
    cpp = root / SOURCE
    if not cpp.is_file():
        raise RuntimeError("DENIED: real LLVM DebuggerThread.cpp missing")
    content = cpp.read_text(encoding="utf-8")
    if "LuIdeOwnerEvent.h" in content:
        raise RuntimeError("DENIED: overlay already applied")
    content = replace_one(
        content,
        '#include "DebuggerThread.h"\n',
        '#include "DebuggerThread.h"\n#include "LuIdeOwnerEvent.h"\n',
    )
    content = replace_one(
        content,
        "      m_is_shutting_down(false) {\n"
        "  m_debugging_ended_event = ::CreateEvent(nullptr, TRUE, FALSE, nullptr);",
        "      m_is_shutting_down(false) {\n"
        "  // Clear owner-pipe inheritance *before* ProcessLauncherWindows can\n"
        "  // create the inferior; first-event initialization is too late.\n"
        "  lu_ide_owner::Owner::Instance();\n"
        "  m_debugging_ended_event = ::CreateEvent(nullptr, TRUE, FALSE, nullptr);",
    )
    content = replace_one(
        content,
        "    if (wait_result) {\n      DWORD continue_status = DBG_CONTINUE;",
        "    if (wait_result) {\n"
        "      // Actual OS event from *this* debugger-owner thread, while\n"
        "      // Windows still holds the debuggee in the event stop.\n"
        "      lu_ide_owner::OnReceived(dbe);\n"
        "      DWORD continue_status = DBG_CONTINUE;",
    )
    content = replace_one(
        content,
        "      ::ContinueDebugEvent(dbe.dwProcessId, dbe.dwThreadId, continue_status);",
        "      BOOL continued = ::ContinueDebugEvent(\n"
        "          dbe.dwProcessId, dbe.dwThreadId, continue_status);\n"
        "      // Revoke this exact exception event immediately after continue.\n"
        "      lu_ide_owner::OnContinued(dbe, continued);",
    )
    overlay = Path(__file__).with_name("LuIdeOwnerEvent.h")
    if not overlay.is_file():
        raise RuntimeError("DENIED: first-party owner-event header missing")
    dest = root / HEADER
    if dest.exists():
        raise RuntimeError("DENIED: upstream target unexpectedly exists")
    dest.write_bytes(overlay.read_bytes())
    cpp.write_text(content, encoding="utf-8")
    progress = root / DAP_PROGRESS_HEADER
    if not progress.is_file():
        raise RuntimeError("DENIED: pinned upstream LLVM DAP ProgressEvent.h missing")
    original_progress = progress.read_text(encoding="utf-8")
    if "#include <chrono>" in original_progress or (
        original_progress.count("std::chrono::system_clock::now()") != 1
    ):
        raise RuntimeError("DENIED: upstream LLVM 18 DAP chrono defect anchor changed")
    fixed_progress = replace_one(
        original_progress, "#include <atomic>\\n",
        "#include <atomic>\\n#include <chrono>\\n",
    )
    progress.write_text(fixed_progress, encoding="utf-8")
    print("LU_LLDB18_PINNED_DAP_CHRONO_HEADER_FIX=APPLIED_NOT_NATIVE_ACCEPTED")
    print("LU_LLDB18_FIRST_PARTY_EVENT_OWNER_OVERLAY=APPLIED_NOT_NATIVE_ACCEPTED")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: apply_llvm18_owner_hook.py PINNED_LLVM_CHECKOUT")
    apply(Path(sys.argv[1]))
