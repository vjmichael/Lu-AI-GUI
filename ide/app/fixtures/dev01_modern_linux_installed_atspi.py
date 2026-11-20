#!/usr/bin/env python3
"""Modern DEV-01 real installed Debian/GTK/AT-SPI integration regression.

The fixture is 867 *actual* files. Interactions are only native GTK, X11,
AT-SPI, OS keyboard and clipboard; no Tauri IPC, JS, DOM or backend shortcut.
Private draft recovery is an explicit user checkpoint, never an invented
automatic post-crash recovery claim. Missing UI/OS evidence fails closed.
"""
import hashlib
import os
import re
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


def _active_confirmation_window(proc, expected_title):
    active = subprocess.check_output(
        ["xdotool", "getactivewindow"], text=True, timeout=5
    ).strip()
    if not active.isdigit():
        deny("confirmation_has_no_native_window")
    title = subprocess.check_output(
        ["xdotool", "getwindowname", active], text=True, timeout=5
    ).strip()
    if title != expected_title:
        deny("confirmation_wrong_native_title")
    pid = subprocess.check_output(
        ["xdotool", "getwindowpid", active], text=True, timeout=5
    ).strip()
    if not pid.isdigit():
        deny("confirmation_has_no_native_owner")
    child = int(pid)
    for _ in range(14):
        if child == proc.pid:
            return active, int(pid), title
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
    deny("confirmation_not_owned_by_installed_desktop")


def _showing(node):
    state = node.getState()
    return all(state.contains(flag) for flag in (
        pyatspi.STATE_VISIBLE, pyatspi.STATE_SHOWING))


def _confirmation_nodes(root):
    stack = [(root, 0)]
    inspected = 0
    while stack and inspected < 256:
        node, depth = stack.pop()
        inspected += 1
        yield node
        if depth < 12:
            stack.extend((child, depth + 1) for child in native.children_of(node))


def _window_geometry(active):
    output = subprocess.check_output(
        ["xdotool", "getwindowgeometry", "--shell", active],
        text=True, timeout=5
    )
    values = {}
    for line in output.splitlines():
        key, sep, value = line.partition("=")
        if sep and key in ("X", "Y", "WIDTH", "HEIGHT"):
            values[key] = int(value)
    if len(values) != 4 or min(values["WIDTH"], values["HEIGHT"]) < 32:
        deny("confirmation_native_geometry_missing")
    return tuple(values[k] for k in ("X", "Y", "WIDTH", "HEIGHT"))


def _native_process_id(node):
    # Native GI/AT-SPI exposes process ID on the accessible or its application.
    # If neither interface supplies it, fail closed rather than guess ownership.
    for source in ("node", "application"):
        try:
            candidate = node if source == "node" else node.getApplication()
            ident = int(candidate.get_process_id())
            if ident > 0:
                return ident
        except Exception:
            continue
    return None


def _same_window_geometry(node, geometry):
    rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
    x, y, width, height = geometry
    # GTK window decorations can shift screen coordinates by a few pixels.
    return (abs(rect.x - x) <= 48 and abs(rect.y - y) <= 48
            and abs(rect.width - width) <= 48
            and abs(rect.height - height) <= 48
            and rect.width >= 32 and rect.height >= 32)


def _exact_accessible_text(node, expected):
    name = " ".join((node.name or "").split())
    if name == expected:
        return True
    try:
        text = node.queryText()
        if 0 < text.characterCount <= 1024:
            return " ".join(text.getText(0, text.characterCount).split()) == expected
    except Exception:
        pass
    return False


def owned_confirmation(proc, message, expected_title="Lu IDE · Confirm source file action",
                       caption="Apply file action"):
    # Independently verify active X11 title, PID lineage, geometry and the
    # exact message and button in one unique top-level native accessible.
    # GTK may expose GtkMessageDialog with the message as its accessible
    # name, or the visible label only through Text, not the X11 title.
    # Wait only for the exact owned foreground window; do not rely on a
    # global message name (some GTK labels expose text but no accessible name).
    until = time.monotonic() + 18
    while True:
        try:
            active, pid, title = _active_confirmation_window(proc, expected_title)
            geometry = _window_geometry(active)
            break
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError):
            if time.monotonic() >= until:
                deny("confirmation_active_exact_native_window_not_verified")
            time.sleep(0.25)
    expected = " ".join(message.split())
    deadline = time.monotonic() + 12
    counters = (0, 0, 0, 0, 0)
    while time.monotonic() < deadline:
        roots = []
        top_level = owned = geometry_matches = 0
        for node in native.elements():
            try:
                if (node.getRoleName().casefold() not in
                        ("dialog", "frame", "window", "alert")
                        or node.parent.getRoleName().casefold() != "application"
                        or not _showing(node)):
                    continue
                top_level += 1
                if _native_process_id(node) != pid:
                    continue
                owned += 1
                if not _same_window_geometry(node, geometry):
                    continue
                geometry_matches += 1
                roots.append(node)
            except Exception:
                continue
        message_seen = False
        buttons = []
        if len(roots) == 1:
            for node in _confirmation_nodes(roots[0]):
                try:
                    if not _showing(node):
                        continue
                    if _exact_accessible_text(node, expected):
                        message_seen = True
                    if ((node.name or "").strip() == caption
                            and "button" in node.getRoleName().casefold()
                            and all(node.getState().contains(flag) for flag in
                                    (pyatspi.STATE_ENABLED, pyatspi.STATE_SENSITIVE))):
                        buttons.append(node)
                except Exception:
                    continue
            if message_seen and len(buttons) == 1:
                action = buttons[0].queryAction()
                indices = [i for i in range(action.nActions)
                           if action.getName(i).casefold() in
                           ("click", "press", "activate")]
                if len(indices) != 1:
                    deny("confirmation_native_action_ambiguous")
                if (_active_confirmation_window(proc, expected_title) != (active, pid, title)
                        or not _same_window_geometry(roots[0], _window_geometry(active))):
                    deny("confirmation_native_window_changed")
                if not action.doAction(indices[0]):
                    deny("confirmation_native_action_denied")
                print("DEV01_MODERN_LINUX_NATIVE_OWNED_CONFIRMATION=OBSERVED", flush=True)
                return
        counters = (top_level, owned, geometry_matches, int(message_seen), len(buttons))
        time.sleep(0.25)
    print("DEV01_NATIVE_CONFIRMATION_INVENTORY=top_level:%d;owned:%d;geometry:%d;message:%d;buttons:%d"
          % counters, flush=True)
    deny("confirmation_exact_owned_modal_or_action_not_verified")


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

    # Historical keyboard navigation: assert genuine AT-SPI focus after
    # each real OS key, then native editor admission and unchanged bytes.
    def focused(label, role):
        node = native.accessible(label, role, seconds=2)
        return (node.name == label and
                node.getState().contains(pyatspi.STATE_FOCUSED))

    native.key("ctrl+p")
    wait_until(lambda: focused("Quick Open filename search", None),
               "quick_open_ctrl_p_focus_missing")
    enter("Quick Open filename search", "src-23/file-03")
    native.key("Return")
    first = "src-23/file-030.rs"
    second = "src-23/file-031.rs"
    native.accessible(first, "button", seconds=35)
    native.accessible(second, "button", seconds=15)
    original = digest(ROOT / first)
    native.key("Down")
    wait_until(lambda: focused(first, "button"), "quick_open_first_focus_missing")
    native.key("Down")
    wait_until(lambda: focused(second, "button"), "quick_open_second_focus_missing")
    native.key("Up")
    wait_until(lambda: focused(first, "button"), "quick_open_reverse_focus_missing")
    native.key("Return")
    native.accessible("Saved source tab " + first, "tab", seconds=20)
    if digest(ROOT / first) != original:
        deny("quick_open_keyboard_changed_source")
    native.activate("Close " + first, "button")
    native.activate_exact_source_file("main.rs")
    print("DEV01_MODERN_LINUX_QUICK_OPEN_KEYBOARD=PASS", flush=True)

    # One real exclusive file/folder creation, case-alias denial and a
    # stale saved-search result after independent external file mutation.
    search_file = ROOT / "r4r4-search.rs"
    created_dir = ROOT / "r4r4-created"
    if search_file.exists() or created_dir.exists():
        deny("non_pristine_residual_editor_fixture")
    native.activate("New folder", "button")
    enter("Source action path or new name", created_dir.name)
    native.activate("Review and apply")
    wait_until(lambda: created_dir.is_dir() and not created_dir.is_symlink(),
               "native_folder_creation_not_observed")
    native.accessible("Source action completed: new folder")
    native.activate("New file", "button")
    enter("Source action path or new name", search_file.name)
    native.activate("Review and apply")
    wait_until(lambda: search_file.is_file() and not search_file.is_symlink(),
               "native_file_creation_not_observed")
    native.accessible("Source action completed: new file")
    if search_file.read_bytes() != b"":
        deny("native_created_file_not_empty")
    search_file.write_bytes(b"// IDE_R4R4_SEARCH_ORIGINAL\n")
    original_search = digest(search_file)
    native.activate("New file", "button")
    enter("Source action path or new name", "R4R4-SEARCH.rs")
    native.activate("Review and apply")
    native.accessible("SOURCE_TARGET_ALREADY_EXISTS")
    if digest(search_file) != original_search or (ROOT / "R4R4-SEARCH.rs").exists():
        deny("case_alias_creation_changed_source")
    native.activate("Cancel", "button")
    print("DEV01_MODERN_LINUX_CREATE_FILE_FOLDER_NO_CLOBBER=PASS", flush=True)

    native.activate("Find in project", "button")
    enter("Search project text", "IDE_R4R4_SEARCH_ORIGINAL")
    native.activate("Search project", "button")
    hit = "r4r4-search.rs:1:4"
    native.accessible(hit, "button", seconds=30)
    search_file.write_bytes(b"// IDE_R4R4_SEARCH_ORIGINAL_CHANGED\n")
    changed = digest(search_file)
    if changed == original_search:
        deny("external_search_fixture_revision_not_changed")
    native.activate(hit, "button")
    native.accessible("FILE_CHANGED_ON_DISK: search again", seconds=20)
    if digest(search_file) != changed:
        deny("stale_search_changed_external_bytes")
    native.activate("Search project", "button")
    native.accessible(hit, "button", seconds=30)
    native.activate(hit, "button")
    native.accessible("Saved source tab r4r4-search.rs", "tab", seconds=20)
    if digest(search_file) != changed:
        deny("fresh_search_changed_external_bytes")
    native.activate("Close r4r4-search.rs", "button")
    native.activate_exact_source_file("main.rs")
    print("DEV01_MODERN_LINUX_STALE_SEARCH_DENIED_FRESH_NAVIGATION=PASS", flush=True)

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
    native.accessible("Source action completed: restore", seconds=20)
    print("DEV01_MODERN_LINUX_MOVE_TRASH_RESTORE_NO_CLOBBER=PASS", flush=True)


def session_reference():
    # Clear the OS clipboard first: a previous reference cannot forge success.
    subprocess.run(["xclip", "-selection", "clipboard", "-in"],
                   input=b"r4r4-fresh-clipboard", stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL, check=True, timeout=5)
    native.activate("Copy draft reference", "button")
    for _ in range(35):
        try:
            value = subprocess.check_output(
                ["xclip", "-selection", "clipboard", "-out"],
                text=True, timeout=4, stderr=subprocess.DEVNULL)
        except (OSError, subprocess.TimeoutExpired, subprocess.CalledProcessError):
            time.sleep(0.35)
            continue
        if value.startswith("ide-buffer:v1 "):
            match = re.search(r' path="main\.rs" buffer=([a-zA-Z0-9-]{8,128}) revision=([0-9]+) baseDisk=([a-f0-9]{64}) draftSha256=([a-f0-9]{64}) ', value)
            if not match or not value.endswith(" lifetime=session-only") or DRAFT in value:
                deny("session_clipboard_reference_invalid_or_disclosing")
            return (match[1], int(match[2]), match[3], match[4])
        time.sleep(0.35)
    deny("fresh_session_clipboard_reference_missing")


def open_exact_relative_fixture_file(filename):
    # Exercise the installed user-facing project-relative Open file form.
    # Explorer rows can be offscreen after a native modal and tree refresh;
    # do not loosen exact-file selection or expand global AT-SPI matching.
    if filename not in ("main.rs", "second.rs"):
        deny("invalid_relative_fixture_file")
    before = (digest(SOURCE), digest(SECOND))
    enter("Open project-relative file", filename)
    native.key("Return")

    def exact_selected_saved_tab():
        matches = []
        for node in native._native_source_editor_nodes():
            try:
                if ((node.name or "").strip() != "Saved source tab " + filename
                        or node.getRoleName().casefold() not in ("tab", "page tab")):
                    continue
                state = node.getState()
                if all(state.contains(flag) for flag in (
                        pyatspi.STATE_VISIBLE, pyatspi.STATE_SHOWING,
                        pyatspi.STATE_SELECTED)):
                    matches.append(node)
            except Exception:
                continue
        return len(matches) == 1

    wait_until(exact_selected_saved_tab, "exact_relative_saved_tab_not_selected_" + filename)
    if (digest(SOURCE), digest(SECOND)) != before:
        deny("relative_open_changed_fixture_source")


def select_existing_saved_fixture_tab(filename):
    # An already-open buffer must be selected via its actual native tab.
    # Disk readback can precede React's Save receipt/clean-tab update.
    if filename not in ('main.rs', 'second.rs'):
        deny('invalid_existing_fixture_tab')
    before = (digest(SOURCE), digest(SECOND))

    def exact_saved_tab():
        matches = []
        for node in native._native_source_editor_nodes():
            try:
                if ((node.name or '').strip() == 'Saved source tab ' + filename
                        and node.getRoleName().casefold() in ('tab', 'page tab')
                        and node.getState().contains(pyatspi.STATE_VISIBLE)):
                    matches.append(node)
            except Exception:
                continue
        if len(matches) != 1:
            return None
        node = matches[0]
        state = node.getState()
        if not all(state.contains(flag) for flag in (
                pyatspi.STATE_ENABLED, pyatspi.STATE_SENSITIVE)):
            return None
        return node

    def select_once():
        node = exact_saved_tab()
        if node is None:
            return False
        state = node.getState()
        if not state.contains(pyatspi.STATE_SHOWING):
            try:
                node.queryComponent().scrollTo(pyatspi.SCROLL_ANYWHERE)
            except Exception:
                pass
            return False
        if state.contains(pyatspi.STATE_SELECTED):
            return True
        action = node.queryAction()
        choices = [index for index in range(action.nActions)
                   if action.getName(index).casefold() in ('click', 'press', 'activate')]
        if len(choices) != 1 or not action.doAction(choices[0]):
            deny('existing_saved_tab_native_action_denied')
        return True

    wait_until(select_once, 'existing_saved_tab_not_ready_' + filename)

    def selected():
        node = exact_saved_tab()
        return node is not None and all(node.getState().contains(flag) for flag in (
            pyatspi.STATE_SHOWING, pyatspi.STATE_SELECTED))

    wait_until(selected, 'existing_saved_tab_not_selected_' + filename)
    if (digest(SOURCE), digest(SECOND)) != before:
        deny('existing_tab_selection_changed_fixture_source')


def modern_split_and_private_checkpoint(proc):
    main_before, second_before = digest(SOURCE), digest(SECOND)
    open_exact_relative_fixture_file("second.rs")
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

    # Explicitly wait for the post-save primary buffer to be clean before navigating.
    select_existing_saved_fixture_tab("second.rs")
    native.activate("Toggle split editor")
    select_existing_saved_fixture_tab("main.rs")
    native.focus_editor()
    original = digest(SOURCE)
    native.key("ctrl+End")
    native.key("Return")
    native.type_text("// " + DRAFT)
    if digest(SOURCE) != original:
        deny("unsaved_draft_written_before_explicit_save")
    time.sleep(0.8)
    first_ref = session_reference()
    if first_ref[2] != original or first_ref[3] == original:
        deny("session_reference_not_exact_unsaved_base")
    native.focus_editor()
    native.key("ctrl+End")
    native.type_text("x")
    time.sleep(0.8)
    changed_ref = session_reference()
    native.focus_editor()
    native.key("ctrl+z")
    time.sleep(0.8)
    undo_ref = session_reference()
    if not (first_ref[0] == changed_ref[0] == undo_ref[0]
            and first_ref[1] < changed_ref[1] < undo_ref[1]
            and first_ref[3] != changed_ref[3] and undo_ref[3] == first_ref[3]
            and undo_ref[2] == original and digest(SOURCE) == original):
        deny("session_reference_undo_lifecycle_not_verified")
    print("DEV01_MODERN_LINUX_SESSION_REFERENCE_UNDO=PASS", flush=True)
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
    native.activate("Remember open tabs for restart")
    owned_confirmation(proc, "Remember file tabs and pane layout only? Unsaved edits are NOT stored in this layout. Use Preserve draft for private recovery before a restart.",
                       expected_title="Lu IDE · Remember tab layout", caption="Remember navigation")
    native.accessible("Remembered file tab layout for restart", seconds=20)
    if digest(SOURCE) != original:
        deny("private_checkpoint_or_tab_layout_wrote_original")
    print("DEV01_MODERN_LINUX_PRE_KILL_PRIVATE_CHECKPOINT=PASS", flush=True)
    return undo_ref


def main():
    verify_fixture()
    first = native.start()
    try:
        native.select_native_folder()
        native.editor_project()
        modern_file_operations(first)
        previous_ref = modern_split_and_private_checkpoint(first)
    finally:
        # Deliberate hard OS process death, not a graceful editor shutdown.
        if first.poll() is None:
            first.kill()
            first.wait(timeout=10)

    second = native.start()
    try:
        native.accessible(ROOT.name, seconds=22)
        native.editor_project()
        select_existing_saved_fixture_tab("main.rs")
        native.accessible("Recover preserved draft", seconds=25)
        native.activate("Recover preserved draft")
        native.accessible("Recovered private checkpoint into unsaved editor only", seconds=20)
        if DRAFT in SOURCE.read_text(encoding="utf-8"):
            deny("restart_recovery_implicitly_wrote_source")
        native.focus_editor()
        native.key("ctrl+End")
        recovered_ref = session_reference()
        if recovered_ref[0] == previous_ref[0] or recovered_ref[2:] != previous_ref[2:]:
            deny("restarted_session_draft_identity_not_distinct")
        native.activate("Save · Ctrl+S")
        wait_until(lambda: DRAFT in SOURCE.read_text(encoding="utf-8"),
                   "recovered_draft_explicit_save_missing")
        select_existing_saved_fixture_tab("main.rs")
        native.focus_editor()
        native.key("ctrl+End")
        saved_ref = session_reference()
        if saved_ref[2] != digest(SOURCE) or saved_ref[3] != digest(SOURCE):
            deny("explicit_save_reference_not_rebased")
        print("DEV01_MODERN_LINUX_SESSION_REFERENCE_RESTART_SAVE_REBASE=PASS", flush=True)
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
