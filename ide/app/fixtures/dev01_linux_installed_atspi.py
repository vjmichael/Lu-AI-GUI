#!/usr/bin/env python3
"""DEV-01 integrated genuine Debian/WebKit/AT-SPI/X11 user actions.

Only a disposable, actually installed app is accepted. We import the reviewed
WO8 native picker/AT-SPI witnesses and NEVER call Tauri IPC, JS, or DOM code.
A failed GTK selection remains a real test failure, not an automatic skip.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time

import pyatspi
import wo8_linux_editor_atspi as native

ROOT = native.ROOT
SOURCE = native.SOURCE
SIDE = ROOT / "side.rs"
MOVE = ROOT / "move-me.rs"
RENAMED = ROOT / "renamed.rs"


def deny(code):
    raise RuntimeError("DEV01_INSTALLED_LINUX_DENIED:" + code)


def wait_for(predicate, error, limit=18):
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.35)
    deny(error)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def input_native(label, value):
    if not value or len(value) > 1024 or any(ch in value for ch in "\n\r\0"):
        deny("invalid_native_keyboard_input")
    item = native.accessible(label, seconds=18)
    try:
        if not item.queryComponent().grabFocus():
            deny("native_input_not_focusable")
    except Exception as exc:
        deny("native_input_not_focusable_" + type(exc).__name__)
    native.key("ctrl+a")
    native.type_text(value)


def is_descendant(child, ancestor):
    for _ in range(16):
        if child is ancestor:
            return True
        try:
            child = child.parent
            if child is None:
                return False
        except Exception:
            return False
    return False


def native_owned_confirmation(proc, message):
    """Require actual accessible browser confirmation and OS-owned focus."""
    native.accessible(message, seconds=15)
    window = subprocess.check_output(
        ["xdotool", "getactivewindow"], text=True, timeout=5
    ).strip()
    if not window.isdigit():
        deny("native_confirmation_window_not_observed")
    pid = subprocess.check_output(
        ["xdotool", "getwindowpid", window], text=True, timeout=5
    ).strip()
    if not pid.isdigit():
        deny("native_confirmation_window_pid_missing")
    current = int(pid)
    owners = set()
    for _ in range(12):
        owners.add(current)
        if current == proc.pid:
            break
        status = Path("/proc") / str(current) / "status"
        if not status.is_file():
            break
        match = next((line for line in status.read_text().splitlines()
                      if line.startswith("PPid:")), None)
        if not match:
            break
        current = int(match.split(":", 1)[1].strip())
    if proc.pid not in owners:
        deny("confirmation_not_owned_by_installed_desktop")
    native.activate("OK", timeout=10)
    print("DEV01_LINUX_OS_OWNED_CONFIRMATION=AT_SPI_ACTIVATED", flush=True)


def secondary_editor():
    region = native.accessible("Secondary source editor pane", seconds=16)
    nodes = [(region, 0)]
    while nodes:
        node, depth = nodes.pop()
        try:
            if node.name == "Source editor text" and                node.getState().contains(pyatspi.STATE_FOCUSABLE):
                if node.queryComponent().grabFocus():
                    return node
            if depth < 10:
                nodes.extend((child, depth + 1)
                             for child in native.children_of(node))
        except Exception:
            pass
    deny("secondary_codemirror_missing_or_cannot_focus")


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("real_disposable_x11_runner_required")
    prefix = str(Path(os.environ["RUNNER_TEMP"]).resolve()) + os.sep
    if not str(ROOT).startswith(prefix) or not all(
            path.is_file() and not path.is_symlink()
            for path in [SOURCE, SIDE, MOVE]) or RENAMED.exists():
        deny("disposable_867_file_project_not_prepared")
    initial = sha(SOURCE)
    moved_sha = sha(MOVE)
    first = native.start()
    try:
        native.select_native_folder()
        native.editor_project()
        native.activate("Quick open · Ctrl+P")
        native.accessible("Quick open · current project")
        native.accessible("Indexed 867 files", seconds=35)
        native.activate("Close · Esc")
        print("DEV01_LINUX_INSTALLED_LARGE_REPO_INDEX_UI=PASS", flush=True)

        native.activate("move-me.rs")
        native.activate("Close move-me.rs")
        native.activate("Rename / move")
        input_native("Rename/move file to existing project folder", "renamed.rs")
        native.activate("Move without overwrite")
        wait_for(lambda: RENAMED.is_file(), "installed_native_move_not_written")
        if MOVE.exists() or sha(RENAMED) != moved_sha:
            deny("installed_move_changed_source_or_bytes")
        print("DEV01_LINUX_INSTALLED_NATIVE_FILE_MOVE=PASS", flush=True)

        native.activate("renamed.rs")
        native.activate("Close renamed.rs")
        native.activate("Trash file")
        native_owned_confirmation(
            first, "Move renamed.rs to project-local recoverable trash?"
        )
        wait_for(lambda: not RENAMED.exists(), "native_file_trash_not_persisted")
        trash = ROOT / ".lu-ide-trash"
        if len(list(trash.glob("*.json"))) != 1 or            len(list(trash.glob("*.payload"))) != 1:
            deny("native_trash_manifest_payload_mismatch")
        native.activate("Project trash")
        native.accessible("Recoverable project trash")
        native.activate("Restore")
        native_owned_confirmation(
            first, "Restore renamed.rs from project trash?"
        )
        wait_for(lambda: RENAMED.is_file(), "native_restore_not_persisted")
        if sha(RENAMED) != moved_sha or list(trash.glob("*.payload")):
            deny("native_restore_damaged_original_content")
        print("DEV01_LINUX_INSTALLED_NATIVE_TRASH_RESTORE=PASS", flush=True)

        native.activate("main.rs")
        native.activate("side.rs")
        native.activate("Split editor")
        native.accessible("Primary source editor pane")
        native.accessible("Secondary source editor pane")
        secondary_editor()
        native.key("ctrl+End")
        native.key("Return")
        native.type_text("// DEV01_LINUX_SPLIT_SAVED")
        native.activate("Save secondary")
        wait_for(lambda: "DEV01_LINUX_SPLIT_SAVED" in SOURCE.read_text(),
                 "native_split_secondary_save_failed")
        print("DEV01_LINUX_INSTALLED_SPLIT_SECONDARY_SAVE=PASS", flush=True)

        saved_hash = sha(SOURCE)
        secondary_editor()
        native.key("ctrl+End")
        native.key("Return")
        native.type_text("// DEV01_LINUX_KILLED_DRAFT")
        time.sleep(2.5)
        if sha(SOURCE) != saved_hash or sha(SOURCE) == initial:
            deny("unsaved_draft_autosaved_or_split_not_saved")
        native.activate("Recover unsaved drafts")
        native.accessible("Local unsaved draft checkpoints")
        native.accessible("Current session")
        print("DEV01_LINUX_INSTALLED_DRAFT_CHECKPOINT_BEFORE_KILL=PASS", flush=True)
    finally:
        if first.poll() is None:
            first.kill()
            first.wait(timeout=8)
    # A real second OS process and a new native app session are mandatory.
    second = native.start()
    try:
        native.accessible(ROOT.name, seconds=20)
        native.editor_project()
        native.accessible("Previous-session drafts available", seconds=25)
        native.activate("Recover unsaved drafts")
        native.accessible("Older application session")
        native.activate("Recover into unsaved tab")
        native.accessible("Recovered main.rs into an unsaved editor tab", seconds=20)
        native.accessible("Unsaved")
        if "DEV01_LINUX_KILLED_DRAFT" in SOURCE.read_text():
            deny("recovery_wrote_project_without_approval")
        native.activate("Save · Ctrl+S")
        wait_for(lambda: "DEV01_LINUX_KILLED_DRAFT" in SOURCE.read_text(),
                 "native_explicit_save_after_recovery_missing")
        print("DEV01_LINUX_INSTALLED_PREVIOUS_SESSION_RECOVERY=PASS", flush=True)
        print("DEV01_LINUX_INSTALLED_FOUR_FEATURES=PASS", flush=True)
    finally:
        native.stop(second)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
        raise
