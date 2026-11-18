#!/usr/bin/env python3
"""Modern DEV-01 real installed Debian/GTK/AT-SPI integration regression.

The fixture is 867 *actual* files. Interactions are only native GTK, X11,
AT-SPI, OS keyboard and clipboard; no Tauri IPC, JS, DOM or backend shortcut.
Private draft recovery is an explicit user checkpoint, never an invented
automatic post-crash recovery claim. Missing UI/OS evidence fails closed.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time

import pyatspi
import wo8_linux_editor_atspi as native
from b4_linux_installed_split_atspi import real_code_editors

ROOT = native.ROOT
SOURCE = native.SOURCE
SECOND = ROOT / "second.rs"
TO_MOVE = ROOT / "move-me.rs"
RENAMED = ROOT / "renamed.rs"
DRAFT = "DEV01_MODERN_UNSAVED_PRIVATE_AFTER_KILL"
SECONDARY = "DEV01_MODERN_SECONDARY_SAVE"
PRIMARY = "DEV01_MODERN_FOCUSED_SAVE"


def deny(reason):
    raise RuntimeError("DEV01_MODERN_INSTALLED_LINUX_DENIED:" + reason)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait_until(predicate, reason, seconds=22):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return
        time.sleep(0.35)
    deny(reason)


def enter(label, value):
    if not value or len(value) > 128 or any(c in value for c in "\r\n\0"):
        deny("invalid_native_input")
    node = native.accessible(label)
    try:
        if not node.queryComponent().grabFocus():
            deny("input_not_focusable_" + label)
    except Exception:
        deny("input_not_focusable_" + label)
    native.key("ctrl+a")
    native.type_text(value)


def owned_confirmation(proc, message):
    # The exact visible native modal must belong to the installed app.
    native.accessible(message, seconds=18)
    active = subprocess.check_output(
        ["xdotool", "getactivewindow"], text=True, timeout=5
    ).strip()
    pid = subprocess.check_output(
        ["xdotool", "getwindowpid", active], text=True, timeout=5
    ).strip()
    if not pid.isdigit():
        deny("confirmation_has_no_native_owner")
    child = int(pid)
    for _ in range(14):
        if child == proc.pid:
            break
        status = Path("/proc") / str(child) / "status"
        if not status.is_file():
            deny("confirmation_owner_gone")
        parent = next(
            (x for x in status.read_text().splitlines() if x.startswith("PPid:")),
            None,
        )
        if parent is None:
            deny("confirmation_owner_unknown")
        child = int(parent.split(":", 1)[1].strip())
    else:
        deny("confirmation_not_owned_by_installed_desktop")
    button = native.accessible("Apply file action", role="button", seconds=12)
    if not native._native_dialog_ancestor(button):
        deny("confirmation_action_not_in_native_dialog")
    native.activate("Apply file action", role="button")
    print("DEV01_MODERN_LINUX_NATIVE_OWNED_CONFIRMATION=OBSERVED", flush=True)


def verify_fixture():
    temp = str(Path(os.environ["RUNNER_TEMP"]).resolve()) + os.sep
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("real_disposable_linux_x11_required")
    if not str(ROOT).startswith(temp) or not ROOT.is_dir() or ROOT.is_symlink():
        deny("untrusted_fixture_root")
    if RENAMED.exists() or not all(
        p.is_file() and not p.is_symlink() for p in (SOURCE, SECOND, TO_MOVE)
    ):
        deny("original_source_missing")
    if len([p for p in ROOT.rglob("*.rs") if p.is_file()]) != 867:
        deny("not_original_867_file_fixture")
    if any((ROOT / f"src-{d:02d}" / f"file-{n:03d}.rs").is_symlink()
           for d in range(24) for n in range(36)):
        deny("linked_fixture_file")


def modern_file_operations(proc):
    # The modern UI no longer has the original 'Indexed 867 files' label.
    enter("Quick Open filename search", "src-23/file-035.rs")
    native.activate("Find files")
    native.accessible("Quick Open filename results", seconds=35)
    native.accessible("src-23/file-035.rs", seconds=15)
    print("DEV01_MODERN_LINUX_BOUNDED_QUICK_OPEN_REAL_UI=PASS", flush=True)

    before = digest(TO_MOVE)
    native.activate("Move file")
    enter("Source action path or new name", "move-me.rs")
    enter("Project-relative move destination", "renamed.rs")
    native.activate("Review and apply")
    owned_confirmation(
        proc, "Move move-me.rs to renamed.rs? No existing file will be overwritten."
    )
    wait_until(lambda: RENAMED.is_file() and not TO_MOVE.exists(),
               "native_move_not_persisted")
    if digest(RENAMED) != before:
        deny("move_damaged_original_source")

    native.activate("Trash file")
    enter("Source action path or new name", "renamed.rs")
    native.activate("Review and apply")
    owned_confirmation(
        proc, "Move renamed.rs to recoverable project trash? "
              "This will not erase private draft checkpoints."
    )
    wait_until(lambda: not RENAMED.exists(), "native_trash_not_persisted")
    if not (ROOT / ".lu-ide-trash").is_dir():
        deny("recoverable_project_trash_missing")

    native.activate("Restore file")
    choice = native.accessible("Recoverable project source file", seconds=20)
    try:
        if not choice.queryComponent().grabFocus():
            deny("native_trash_choice_not_focusable")
    except Exception:
        deny("native_trash_choice_not_focusable")
    # One exact original recoverable file; first item is the empty placeholder.
    native.key("End")
    native.key("Return")
    native.activate("Review and apply")
    owned_confirmation(
        proc, "Restore renamed.rs from project trash? "
              "No existing file will be overwritten."
    )
    wait_until(lambda: RENAMED.is_file(), "native_restore_not_persisted")
    if digest(RENAMED) != before:
        deny("restored_source_not_original_bytes")
    print("DEV01_MODERN_LINUX_MOVE_TRASH_RESTORE_NO_CLOBBER=PASS", flush=True)


def modern_split_and_private_checkpoint():
    main_before, second_before = digest(SOURCE), digest(SECOND)
    native.activate_exact_source_file("second.rs")
    native.activate("Toggle split editor")
    native.accessible("Second editor file")
    native.accessible("Save second file")
    primary, secondary = real_code_editors()
    if not secondary.queryComponent().grabFocus():
        deny("secondary_native_codemirror_not_focusable")
    native.key("ctrl+End")
    native.key("Return")
    native.type_text("// " + SECONDARY)
    if digest(SOURCE) != main_before or digest(SECOND) != second_before:
        deny("unsaved_secondary_draft_implicitly_persisted")
    native.activate("Save second file")
    wait_until(lambda: SECONDARY in SOURCE.read_text(encoding="utf-8"),
               "secondary_explicit_save_missing")
    if digest(SECOND) != second_before:
        deny("secondary_save_changed_wrong_source")
    main_saved = digest(SOURCE)

    primary, secondary = real_code_editors()
    if not primary.queryComponent().grabFocus():
        deny("primary_native_codemirror_not_focusable")
    native.key("ctrl+End")
    native.key("Return")
    native.type_text("// " + PRIMARY)
    native.key("ctrl+s")
    wait_until(lambda: PRIMARY in SECOND.read_text(encoding="utf-8"),
               "focused_ctrl_s_missing")
    if digest(SOURCE) != main_saved:
        deny("focused_save_changed_wrong_pane")
    print("DEV01_MODERN_LINUX_TWO_PANE_FOCUSED_SAVE=PASS", flush=True)

    native.activate("Toggle split editor")
    native.activate_exact_source_file("main.rs")
    native.focus_editor()
    original = digest(SOURCE)
    native.key("ctrl+End")
    native.key("Return")
    native.type_text("// " + DRAFT)
    if digest(SOURCE) != original:
        deny("unsaved_draft_written_before_explicit_save")
    native.activate("Preserve draft + copy durable reference")
    clipboard = ""
    for _ in range(35):
        try:
            clipboard = subprocess.check_output(
                ["xclip", "-selection", "clipboard", "-out"],
                text=True, timeout=4, stderr=subprocess.DEVNULL
            )
            if clipboard.startswith("ide-draft:v1"):
                break
        except (OSError, subprocess.TimeoutExpired, subprocess.CalledProcessError):
            pass
        time.sleep(0.35)
    if not (clipboard.startswith("ide-draft:v1")
            and "lifetime=persistent-private-app-data" in clipboard
            and "checkpoint=" in clipboard and DRAFT not in clipboard):
        deny("durable_private_non_disclosing_checkpoint_missing")
    native.activate("Remember tabs")
    if digest(SOURCE) != original:
        deny("private_checkpoint_or_tab_layout_wrote_original")
    print("DEV01_MODERN_LINUX_PRE_KILL_PRIVATE_CHECKPOINT=PASS", flush=True)


def main():
    verify_fixture()
    first = native.start()
    try:
        native.select_native_folder()
        native.editor_project()
        modern_file_operations(first)
        modern_split_and_private_checkpoint()
    finally:
        # Deliberate hard OS process death, not a graceful editor shutdown.
        if first.poll() is None:
            first.kill()
            first.wait(timeout=10)

    second = native.start()
    try:
        native.accessible(ROOT.name, seconds=22)
        native.editor_project()
        native.activate_exact_source_file("main.rs")
        native.accessible("Recover preserved draft", seconds=25)
        native.activate("Recover preserved draft")
        native.accessible("Recovered private checkpoint to editor only", seconds=20)
        if DRAFT in SOURCE.read_text(encoding="utf-8"):
            deny("restart_recovery_implicitly_wrote_source")
        native.activate("Save · Ctrl+S")
        wait_until(lambda: DRAFT in SOURCE.read_text(encoding="utf-8"),
                   "recovered_draft_explicit_save_missing")
        print("DEV01_MODERN_LINUX_KILL_RESTART_PRIVATE_RECOVERY=PASS", flush=True)
        print("DEV01_MODERN_LINUX_INSTALLED_FOUR_FEATURES=PASS", flush=True)
    finally:
        native.stop(second)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
        sys.exit(1)
