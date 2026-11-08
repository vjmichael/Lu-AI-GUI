#!/usr/bin/env python3
"""Real installed Linux Tauri/AT-SPI protected LSP acceptance witness.

No Tauri IPC, DOM, JS, unapproved PATH executables, direct worker calls or
fabricated diagnostics. A real GTK approval and a kernel-contained worker are
required. This source fixture is NOT a substitute for successful OS jobs.
"""
from __future__ import annotations

import os
from pathlib import Path
import sys
import time

import pyatspi
import wo8_linux_editor_atspi as native

ROOT = native.ROOT


def deny(category):
    raise RuntimeError("DEV02_LINUX_INSTALLED_DENIED:" + category)


def genuine_native_approval():
    control = native.accessible("Allow this session", seconds=18)
    if not native._native_dialog_ancestor(control):
        deny("human_approval_button_not_in_real_os_dialog")
    native.activate("Allow this session", timeout=12)
    print("DEV02_LINUX_NATIVE_GTK_HUMAN_APPROVAL=PASS", flush=True)


def editor_cursor():
    editor = native.accessible("Source editor text", seconds=15)
    try:
        if not editor.queryComponent().grabFocus():
            deny("source_editor_not_keyboard_focusable")
        text = editor.queryText()
        caret = int(text.caretOffset)
        if caret < 0:
            deny("native_editor_caret_not_exposed")
        return caret
    except Exception as e:
        deny("native_editor_caret_unavailable_" + type(e).__name__)


def position_in_call():
    native.focus_editor()
    native.key("ctrl+Home")
    # Reviewed fixture places the same-file definition call on line 3.
    native.key("Down", "Down")
    native.key("End")
    for _ in range(5):
        native.key("Left")
    time.sleep(0.35)


def wait_accessible(label, seconds=20):
    return native.accessible(label, seconds=seconds)


def run_semantic(language, filename, provider):
    native.activate(filename)
    native.focus_editor()
    native.activate("Start " + provider)
    genuine_native_approval()
    wait_accessible("Protected " + provider + " session started", seconds=30)
    # The installed native client must poll this real worker, not a mocked
    # native response or a source fixture with synthetic diagnostics.
    native.activate("Problems")
    wait_accessible("Language server problems", seconds=30)
    print("DEV02_LINUX_" + language.upper() +
          "_PROTECTED_DIAGNOSTICS=PASS", flush=True)

    position_in_call()
    native.activate("Hover")
    wait_accessible("Symbol information", seconds=25)
    native.activate("Complete")
    wait_accessible("Suggestions", seconds=25)
    before = editor_cursor()
    native.activate("Definition")
    time.sleep(1.3)
    after = editor_cursor()
    if after >= before:
        deny("native_same_file_definition_did_not_reposition_caret_" + language)
    print("DEV02_LINUX_" + language.upper() +
          "_HOVER_COMPLETION_DEFINITION=PASS", flush=True)
    native.activate("Stop " + provider)
    wait_accessible("Language-server session stopped", seconds=15)


def run_python():
    native.activate("hello.py")
    native.focus_editor()
    wait_accessible("Ruff · diagnostics only", seconds=18)
    native.activate("Start Ruff")
    genuine_native_approval()
    wait_accessible("Protected Ruff session started", seconds=30)
    native.activate("Problems")
    wait_accessible("Language server problems", seconds=30)
    wait_accessible("unused", seconds=20)
    print("DEV02_LINUX_PYTHON_RUFF_PROTECTED_DIAGNOSTICS=PASS", flush=True)
    # A Python semantic command MUST remain disabled by both UI and native
    # provider policy. Do not silently substitute clangd or unreviewed Pyright.
    for label in ["Complete", "Hover", "Definition"]:
        node = native.accessible(label, seconds=10)
        try:
            if node.getState().contains(pyatspi.STATE_ENABLED):
                deny("unapproved_ruff_semantic_method_enabled_" + label)
        except RuntimeError:
            raise
        except Exception as e:
            deny("cannot_verify_disabled_method_" + type(e).__name__)
    native.activate("Stop Ruff")
    wait_accessible("Language-server session stopped", seconds=15)


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("native_runner_required")
    if not ROOT.is_dir() or not str(ROOT).startswith(
        str(Path(os.environ["RUNNER_TEMP"]).resolve()) + os.sep
    ):
        deny("project_must_be_disposable_runner_directory")
    for filename in ("main.rs", "main.cpp", "hello.py", "Cargo.toml"):
        target = ROOT / filename
        if not target.is_file() or target.is_symlink():
            deny("fixture_file_not_real_" + filename)
    desktop = native.start()
    try:
        native.select_native_folder()
        native.editor_project()
        # Exact-source Rust and C++ language-service navigation. The source
        # fixture must contain real identifiers with server-resolvable
        # same-file declarations, not pre-rendered diagnostics.
        run_semantic("rust", "main.rs", "rust-analyzer")
        run_semantic("cpp", "main.cpp", "clangd")
        run_python()
        print("DEV02_LINUX_INSTALLED_ALL_THREE_PROTECTED_PROVIDERS=PASS", flush=True)
    finally:
        native.stop(desktop)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc), file=sys.stderr, flush=True)
        raise
