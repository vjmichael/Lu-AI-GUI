#!/usr/bin/env python3
"""WO8-D native Linux installed Tauri accessibility edit/restart/conflict witness.

Driven through actual AT-SPI accessibility and OS keyboard input in the X11
display. No Tauri IPC, internal React state, localStorage or JS automation.
Never treat a missing accessible control as a pass.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time

import pyatspi

EXE = os.environ["WO8_INSTALLED_EXE"]
ROOT = Path(os.environ["WO8_FIXTURE_ROOT"]).resolve()
SOURCE = ROOT / "main.rs"
SENTINEL = "WO8_LINUX_EXPLICIT_SAVE_SENTINEL"
CONFLICT = "WO8_LINUX_EXTERNAL_CONFLICT"


def deny(reason):
    raise RuntimeError("WO8_LINUX_EDITOR_DENIED:" + reason)


def digest():
    return hashlib.sha256(SOURCE.read_bytes()).hexdigest()


def children_of(node):
    try:
        count = min(node.childCount, 256)
        for index in range(count):
            try:
                yield node.getChildAtIndex(index)
            except Exception:
                continue
    except Exception:
        return


def elements():
    desktop = pyatspi.Registry.getDesktop(0)
    stack = [(desktop, 0)]
    inspected = 0
    while stack and inspected < 3200:
        node, depth = stack.pop()
        inspected += 1
        yield node
        if depth < 13:
            stack.extend((child, depth + 1) for child in children_of(node))


def accessible(name, role=None, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        for obj in elements():
            try:
                title = obj.name or ""
                kind = obj.getRoleName() or ""
                if name.casefold() in title.casefold() and (role is None or role in kind.casefold()):
                    return obj
            except Exception:
                pass
        time.sleep(0.3)
    deny("missing_native_accessible_control_" + name.replace(" ", "_"))


def activate(name, role=None, timeout=20):
    obj = accessible(name, role, timeout)
    try:
        action = obj.queryAction()
        for index in range(action.nActions):
            if action.getName(index).lower() in {"click", "press", "activate"}:
                if action.doAction(index):
                    return
    except Exception:
        pass
    try:
        if obj.queryComponent().grabFocus():
            subprocess.run(["xdotool", "key", "Return"], check=True, timeout=5)
            return
    except Exception:
        pass
    deny("native_accessibility_action_unavailable_" + name.replace(" ", "_"))


def key(*keys):
    subprocess.run(["xdotool", "key", *keys], check=True, timeout=5)


def type_text(data):
    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "15", "--", data],
                   check=True, timeout=20)


def focus_editor():
    node = accessible("Source editor text", seconds=15)
    try:
        if node.queryComponent().grabFocus():
            return
    except Exception:
        pass
    deny("native_codemirror_not_keyboard_focusable")


def wait_source(fragment, seconds=12):
    limit = time.monotonic() + seconds
    while time.monotonic() < limit:
        if fragment in SOURCE.read_text(encoding="utf-8"):
            return
        time.sleep(0.35)
    deny("explicit_save_not_written_to_disk")


def start():
    app = subprocess.Popen([EXE], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        accessible("Workspace projects", seconds=35)
    except Exception:
        app.terminate()
        app.wait(timeout=8)
        raise
    return app


def stop(proc):
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=4)


def select_native_folder():
    activate("+ Open folder")
    time.sleep(1)
    key("ctrl+l")
    type_text(str(ROOT))
    key("Return")
    time.sleep(1)
    # GTK folder picker normally accepts via the Open/Select action.
    for button in ("Select", "Open", "Choose Folder"):
        try:
            activate(button, timeout=2)
            break
        except RuntimeError:
            continue
    accessible("Registered projects", seconds=15)
    accessible(ROOT.name, seconds=15)


def editor_project():
    activate("Edit")
    accessible("Source explorer and editor", seconds=15)
    activate("main.rs")
    focus_editor()


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("dedicated_native_x11_runner_required")
    if not ROOT.is_dir() or not str(ROOT).startswith(str(Path(os.environ["RUNNER_TEMP"]).resolve()) + os.sep):
        deny("non_disposable_fixture")
    if not SOURCE.is_file() or SENTINEL in SOURCE.read_text(encoding="utf-8"):
        deny("fixture_not_pristine")
    original = digest()
    first = start()
    try:
        select_native_folder()
        editor_project()
        key("ctrl+End")
        key("Return")
        type_text("// " + SENTINEL)
        time.sleep(0.8)
        if digest() != original:
            deny("unsaved_ide_draft_changed_source")
        activate("Review changes")
        accessible("Save reviewed changes", seconds=12)
        activate("Return to editor")
        activate("Save · Ctrl+S")
        wait_source(SENTINEL)
        print("WO8_LINUX_NATIVE_EDITOR_REVIEW_AND_EXPLICIT_SAVE=PASS", flush=True)
    finally:
        stop(first)

    second = start()
    try:
        accessible(ROOT.name, seconds=16)
        editor_project()
        if SENTINEL not in SOURCE.read_text(encoding="utf-8"):
            deny("saved_source_missing_after_restart")
        print("WO8_LINUX_NATIVE_RESTART_REOPEN=PASS", flush=True)

        # Conflict from a second OS writer while a local CodeMirror draft exists.
        focus_editor()
        key("ctrl+End")
        key("Return")
        type_text("// UNSAVED_WO8_DRAFT")
        time.sleep(0.6)
        before = SOURCE.read_text(encoding="utf-8")
        SOURCE.write_text(before + "\n// " + CONFLICT + "\n", encoding="utf-8")
        outside_sha = digest()
        activate("Save · Ctrl+S")
        accessible("File changed on disk", seconds=16)
        if digest() != outside_sha or CONFLICT not in SOURCE.read_text(encoding="utf-8"):
            deny("conflicting_external_edit_overwritten")
        accessible("Unsaved", seconds=10)
        print("WO8_LINUX_NATIVE_EXTERNAL_CONFLICT_DENIED=PASS", flush=True)
    finally:
        stop(second)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(1)
