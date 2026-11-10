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

from gtk_chooser_confirmation_policy import candidate_captions

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


# A dedicated runner-only stderr file is read by category-filtered probes.
# Never print the file's raw contents, paths, native crashes or arbitrary text.
_NATIVE_APP_LOG = Path(os.environ.get("RUNNER_TEMP", "/dev/null")) / "ide056-native-picker.log"

def _host_picker_receipts():
    try:
        if not _NATIVE_APP_LOG.is_file() or _NATIVE_APP_LOG.is_symlink():
            return "none"
        tail = _NATIVE_APP_LOG.read_bytes()[-32768:]
        known = (
            b"registered", b"cancelled", b"invalid_directory",
            b"project_capacity", b"already_open", b"io_failure",
            b"catalog_invalid", b"host_unavailable", b"unexpected_error"
        )
        markers = []
        for line in tail.splitlines():
            if b"IDE056_NATIVE_PICKER_RESULT=" not in line:
                continue
            value = line.split(b"IDE056_NATIVE_PICKER_RESULT=", 1)[1].strip()
            if value in known:
                markers.append(value.decode("ascii"))
        return ",".join(markers[-4:]) if markers else "none"
    except (OSError, UnicodeError):
        return "unavailable"

def start():
    stderr_target = open(_NATIVE_APP_LOG, "ab") if os.environ.get("GITHUB_ACTIONS") == "true" else subprocess.DEVNULL
    app = subprocess.Popen([EXE], stdout=subprocess.DEVNULL, stderr=stderr_target)
    if stderr_target != subprocess.DEVNULL:
        stderr_target.close()
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


def _native_dialog_ancestor(node):
    # Inspect an actual AT-SPI modal/file-chooser ancestry, not an unrelated
    # app button named Open or Select. No web JS, IPC or synthetic picker.
    current = node
    for _ in range(20):
        try:
            current = current.parent
            if current is None:
                return False
            role = (current.getRoleName() or "").casefold()
            if "dialog" in role or "file chooser" in role:
                return True
        except Exception:
            return False
    return False


def _x11_dialog_activation(attempt):
    # AT-SPI can expose a GTK chooser role while omitting its action buttons.
    # Only the active visible *OS chooser window*, verified by both AT-SPI
    # role and X11 title, is eligible for a native keyboard fallback.
    # Never send keys to the Tauri main window or an unrelated application.
    if not _native_chooser_still_open():
        return False
    try:
        active = subprocess.check_output(
            ["xdotool", "getactivewindow"], text=True, timeout=5).strip()
        if not active.isdigit():
            return False
        title = subprocess.check_output(
            ["xdotool", "getwindowname", active], text=True, timeout=5).strip().casefold()
    except (OSError, ValueError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired):
        print("P04_GTK_X11_CHOOSER=active_window_unavailable", flush=True)
        return False
    allowed = ("open", "open folder", "select folder", "select a folder",
               "choose folder", "choose a folder", "select")
    if title not in allowed and not any(
            text in title for text in ("select folder", "choose folder", "open folder")):
        print("P04_GTK_X11_CHOOSER=unrecognized_active_window_title", flush=True)
        return False
    # xdotool genuinely delivers OS keyboard events to the focused GTK
    # dialog. A sent key is never itself accepted as a successful project
    # registration; the installed WebView must expose the exact project.
    try:
        subprocess.run(["xdotool", "windowactivate", "--sync", active],
                       check=True, timeout=5)
        # GTK versions differ: an X11 folder chooser can expose no
        # accessible Select Folder button despite a real dialog window.
        # Try only bounded standard native confirmation keys, never a
        # backend/JS selection or an unrelated-window click.
        # A real Ubuntu 24.04 run navigated to the selected folder on the
        # first Enter, while later attempts clicked inside the dialog rather
        # than activating its missing accessibility-exposed accept button.
        # In GTK SELECT_FOLDER the native affirmative mnemonic is frequently
        # Alt+S ("Select"); attempt that BEFORE an ambiguous spatial click.
        # Every event remains a candidate only: exact project registration
        # and native dialog closure are independently mandatory.
        native_key = ("Return", "alt+s", "alt+o", "Return")[attempt]
        key(native_key)
        print("P04_GTK_NATIVE_CONFIRM_KEY=" +
              ("enter" if native_key == "Return" else
               "alt_select" if native_key == "alt+s" else "alt_open") +
              ";attempt=" + str(attempt), flush=True)
        print("P04_GTK_X11_CHOOSER=visible_real_os_dialog_keyboard;"
              "attempt=" + str(attempt), flush=True)
        return True
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        print("P04_GTK_X11_CHOOSER=native_key_unavailable", flush=True)
        return False


def _atspi_bounded_chooser_mouse(attempt):
    # Xvfb + GTK can expose a visible file-chooser widget without an X11
    # active-window property or accessible action button. Use only its actual
    # OS AT-SPI bounding rectangle, never guessed global screen coordinates.
    try:
        size = subprocess.check_output(
            ["xdotool", "getdisplaygeometry"], text=True, timeout=5).strip().split()
        if len(size) != 2:
            return False
        screen_w, screen_h = (int(v) for v in size)
    except (OSError, ValueError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired):
        print("P04_GTK_CHOOSER_SPATIAL=display_geometry_unavailable", flush=True)
        return False
    for node in elements():
        try:
            if "file chooser" not in (node.getRoleName() or "").casefold():
                continue
            flags = node.getState()
            if not (flags.contains(pyatspi.STATE_SHOWING)
                    and flags.contains(pyatspi.STATE_VISIBLE)):
                continue
            rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            if (rect.width < 320 or rect.height < 180 or rect.x < 0 or rect.y < 0
                    or rect.x + rect.width > screen_w
                    or rect.y + rect.height > screen_h):
                continue
            # Retry genuinely distinct right-hand action-row positions only
            # inside the OS-verified visible chooser, never the main WebView.
            # Attempt 1 checks the rightmost confirmation region; attempt 3
            # checks the adjacent region if the dialog remains visible.
            margin = 55 if attempt < 2 else 95
            px = int(rect.x + rect.width - margin)
            py = int(rect.y + rect.height - 32)
            if not (rect.x + 8 < px < rect.x + rect.width - 8
                    and rect.y + 8 < py < rect.y + rect.height - 8):
                continue
            subprocess.run(
                ["xdotool", "mousemove", "--sync", str(px), str(py)],
                check=True, timeout=5)
            subprocess.run(["xdotool", "click", "1"], check=True, timeout=5)
            print("P04_GTK_CHOOSER_SPATIAL=real_os_click_inside_visible_atspi_chooser;"
                  "attempt=" + str(attempt), flush=True)
            return True
        except (ValueError, OSError, subprocess.CalledProcessError,
                subprocess.TimeoutExpired):
            continue
    print("P04_GTK_CHOOSER_SPATIAL=no_valid_visible_native_chooser_geometry",flush=True)
    return False


def _native_accept_button_from_visible_chooser(attempt):
    """One bounded native button candidate, never an acceptance assertion.

    GTK on installed Ubuntu exposed exactly ONE enabled visible OS button
    outside the original textual allowlist. This fallback acts only on
    a non-cancel button whose geometry is fully inside a visible native
    file-chooser's lower-right action row. The caller must still prove a
    closed modal AND the exact registered canonical project directory.
    """
    if attempt != 1 or not _native_chooser_still_open():
        return False
    try:
        active = subprocess.check_output(
            ["xdotool", "getactivewindow"], text=True, timeout=5).strip()
        title = subprocess.check_output(
            ["xdotool", "getwindowname", active], text=True, timeout=5
        ).strip().casefold()
        if not active.isdigit() or (
                title not in ("open", "select", "select folder", "choose folder",
                              "open folder", "choose a folder", "select a folder")
                and not any(word in title for word in (
                    "select folder", "choose folder", "open folder"))):
            return False
        dim = subprocess.check_output(
            ["xdotool", "getdisplaygeometry"], text=True,
            timeout=5).split()
        if len(dim) != 2:
            return False
        width, height = map(int, dim)
    except (OSError, ValueError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired):
        return False
    choosers = []
    for node in elements():
        try:
            if "file chooser" not in (node.getRoleName() or "").casefold():
                continue
            state = node.getState()
            if not (state.contains(pyatspi.STATE_VISIBLE) and
                    state.contains(pyatspi.STATE_SHOWING)):
                continue
            rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            if (rect.width < 480 or rect.height < 340 or
                rect.x < 0 or rect.y < 0 or
                rect.x + rect.width > width or rect.y + rect.height > height):
                continue
            choosers.append(rect)
        except Exception:
            continue
    candidates = []
    for node in elements():
        try:
            if "button" not in (node.getRoleName() or "").casefold():
                continue
            if not _native_dialog_ancestor(node):
                continue
            state = node.getState()
            if not (state.contains(pyatspi.STATE_VISIBLE) and
                    state.contains(pyatspi.STATE_SHOWING) and
                    state.contains(pyatspi.STATE_ENABLED)):
                continue
            caption = (node.name or "").strip().casefold()
            # A Cancel/Close button must never be treated as an affirmative
            # folder-selection action merely because it is visible.
            if caption in ("cancel", "close", "back", "previous", "up", "abort"):
                continue
            rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            if rect.width < 32 or rect.height < 16:
                continue
            for chooser in choosers:
                if (rect.x >= chooser.x + chooser.width * 0.55 and
                    rect.y >= chooser.y + chooser.height * 0.71 and
                    rect.x + rect.width <= chooser.x + chooser.width - 4 and
                    rect.y + rect.height <= chooser.y + chooser.height - 3):
                    candidates.append((rect.x + rect.width, rect.y, node, rect))
                    break
        except Exception:
            continue
    if len(candidates) != 1:
        print("P04_GTK_NATIVE_ACTION_ROW=not_single_verified_button", flush=True)
        return False
    _, _, node, rect = candidates[0]
    try:
        action = node.queryAction()
        for index in range(action.nActions):
            if action.getName(index).casefold() in ("click", "press", "activate"):
                if action.doAction(index):
                    print("P04_GTK_NATIVE_ACTION_ROW=real_atspi_button_attempt",
                          flush=True)
                    return True
    except Exception:
        pass
    try:
        x = int(rect.x + rect.width / 2)
        y = int(rect.y + rect.height / 2)
        subprocess.run(["xdotool", "mousemove", "--sync", str(x), str(y)],
                       check=True, timeout=5)
        subprocess.run(["xdotool", "click", "1"], check=True, timeout=5)
        print("P04_GTK_NATIVE_ACTION_ROW=real_x11_button_attempt", flush=True)
        return True
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return False


def _native_picker_confirm(attempt=0):
    # The previous real Ubuntu 26.04 run clicked an authentic AT-SPI
    # chooser action THREE times and the dialog never closed. A generic
    # "Open" can also be GTK's navigation button. Prefer actual Select
    # Folder and bottom-right action geometry, retaining native ancestry.
    # The real Ubuntu26 failure reported only one generic "Open" action,
    # repeatedly activated with no dialog closure. That may be navigation,
    # not acceptance. Suppress the same ineffective action on retries.
    permitted = candidate_captions(
        ("select folder", "choose folder", "select", "open folder", "open"),
        attempt)
    priority = {name: i for i, name in enumerate(permitted)}
    ranked = []
    for obj in elements():
        try:
            name = (obj.name or "").strip().casefold()
            role = (obj.getRoleName() or "").casefold()
            if name not in priority or "button" not in role or not _native_dialog_ancestor(obj):
                continue
            states = obj.getState()
            if not (states.contains(pyatspi.STATE_ENABLED)
                    and states.contains(pyatspi.STATE_SHOWING)
                    and states.contains(pyatspi.STATE_VISIBLE)):
                continue
            try:
                rect = obj.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                if rect.width < 8 or rect.height < 8:
                    continue
                y, x = int(rect.y), int(rect.x)
            except Exception:
                # A genuine OS action may lack component geometry; sort last,
                # never accept an untrusted non-dialog element as a substitute.
                y, x = -1, -1
            ranked.append((priority[name], -y, -x, len(ranked), name, obj))
        except Exception:
            continue
    ranked.sort(key=lambda record: record[:4])
    # An explicit folder-selection button may still be retried on the next
    # attempt, but never the same generic 'Open' that failed to close GTK.
    for item in ranked:
        try:
            _, _, _, _, name, obj = item
            action = obj.queryAction()
            for i in range(action.nActions):
                if action.getName(i).casefold() in ("click", "press", "activate"):
                    if action.doAction(i):
                        print("P04_NATIVE_LINUX_GTK_DIALOG_CONFIRMATION=real_atspi_action;"
                              "caption=" + name.replace(" ", "_") +
                              ";ranked_candidates=" + str(len(ranked)) +
                              ";attempt=" + str(attempt), flush=True)
                        return
        except Exception:
            continue
    # When there is no authentic explicit Select Folder action, or it fails
    # to activate, use independent native OS witnesses. No repeating a
    # generic toolbar Open/navigation action for each failed attempt.
    if attempt > 0 and not ranked:
        print("P04_GTK_RETRY_POLICY=generic_open_suppressed;"
              "attempt=" + str(attempt), flush=True)
    # The real Ubuntu 26.04 run proved that successfully SENDING a keyboard
    # key does not imply a folder was selected: four keystroke deliveries
    # all left the dialog open. Previously this unconditional early return
    # made the independently bounded native chooser click unreachable.
    # Alternate mechanisms after the caller verifies the dialog remains
    # showing. Every spatial attempt is inside an actual visible AT-SPI
    # file-chooser rectangle; a close without exact project registration
    # still fails the acceptance test.
    # At the exact-directory confirmation attempt, prefer a real visible,
    # enabled native action-row button over guessed GTK keyboard mnemonics.
    # A dispatched action is never accepted without exact Tauri catalog and
    # installed WebView project identity evidence.
    if _native_accept_button_from_visible_chooser(attempt):
        return
    # Send the *authentic* native GTK keyboard confirmation before trying
    # AT-SPI geometry. Earlier real runs never exercised the Alt+S mnemonic:
    # a successful mouse dispatch returned early even though the chooser
    # remained open. Dispatch is not success; the caller still checks GTK
    # modal closure AND the exact registered project identity.
    if _x11_dialog_activation(attempt):
        return
    if _atspi_bounded_chooser_mouse(attempt):
        return
    print("P04_NATIVE_LINUX_GTK_DIALOG_CONFIRMATION=missing_real_chooser_button;"
          "matching_dialog_buttons=" + str(len(ranked)), flush=True)
    deny("native_dialog_confirmation_not_found")

def _native_chooser_still_open():
    # GTK AT-SPI providers may retain a hidden chooser node *after* an OS
    # action closes its visible window. A role name alone was previously used
    # as the open check, producing false "still open" observations.
    for node in elements():
        try:
            role = (node.getRoleName() or "").casefold()
            if "file chooser" not in role:
                continue
            states = node.getState()
            if states.contains(pyatspi.STATE_SHOWING) and states.contains(pyatspi.STATE_VISIBLE):
                return True
        except Exception:
            continue
    return False


def _native_chooser_diagnostics():
    """Bounded, category-only evidence for the visible native OS chooser.

    Never print raw accessible names, filesystem paths, full window titles,
    keyboard contents or screenshots. A diagnostic is not an approval.
    """
    chooser_count = 0
    chooser_rect_valid = 0
    dialog_buttons = 0
    enabled_visible_buttons = 0
    explicit_folder_actions = 0
    generic_open_actions = 0
    other_actions = 0
    for node in elements():
        try:
            role = (node.getRoleName() or "").casefold()
            state = node.getState()
            visible = (state.contains(pyatspi.STATE_SHOWING)
                       and state.contains(pyatspi.STATE_VISIBLE))
            if "file chooser" in role and visible:
                chooser_count += 1
                try:
                    rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                    if rect.width >= 320 and rect.height >= 180:
                        chooser_rect_valid += 1
                except Exception:
                    pass
            if "button" not in role or not _native_dialog_ancestor(node):
                continue
            dialog_buttons += 1
            if not visible or not state.contains(pyatspi.STATE_ENABLED):
                continue
            enabled_visible_buttons += 1
            caption = (node.name or "").strip().casefold()
            if caption in {"select folder", "choose folder", "select", "open folder"}:
                explicit_folder_actions += 1
            elif caption == "open":
                generic_open_actions += 1
            else:
                other_actions += 1
        except Exception:
            continue
    print("P04_GTK_NATIVE_CHOOSER_INVENTORY="
          "visible_choosers:" + str(min(chooser_count, 16)) +
          ";sized_choosers:" + str(min(chooser_rect_valid, 16)) +
          ";dialog_buttons:" + str(min(dialog_buttons, 256)) +
          ";enabled_visible_buttons:" + str(min(enabled_visible_buttons, 256)) +
          ";explicit_confirmations:" + str(min(explicit_folder_actions, 256)) +
          ";generic_open:" + str(min(generic_open_actions, 256)) +
          ";other_buttons:" + str(min(other_actions, 256)), flush=True)


def _native_catalog_project_disposition():
    """Diagnostic only, never a substitute for the genuine installed UI row.

    Read the one fixed Tauri app-data catalog for the disposable test account.
    Classify the canonical root; don't print, leak or accept arbitrary paths.
    """
    import json
    catalogs=(
        Path.home() / ".local/share/org.lu.ide/projects.json",
        Path.home() / ".config/org.lu.ide/projects.json",
    )
    observed = "not_written"
    for path in catalogs:
        try:
            if not path.is_file() or path.is_symlink():
                continue
            payload = json.loads(path.read_text(encoding="utf-8"))
            rows = payload.get("projects")
            if not isinstance(rows, list):
                return "invalid_records"
            if any(isinstance(row, dict) and
                   str(row.get("root", "")) == str(ROOT) and
                   row.get("display_name") == ROOT.name
                   for row in rows):
                return "exact_native_root_persisted"
            if any(isinstance(row, dict) and
                   str(row.get("root", "")) == str(ROOT.parent)
                   for row in rows):
                return "wrong_parent_persisted"
            if rows:
                observed="unexpected_project_persisted"
            else:
                observed="catalog_without_projects"
        except (OSError, ValueError, TypeError):
            return "unreadable_catalog"
    return observed


def _wait_for_native_registered_project(seconds=20):
    """Wait for Tauri's asynchronous picker callback and React render.

    A closed OS chooser is *not* registration. A catalog file is *not* a
    clickable UI control. Require both exact persisted root and observed
    native installed WebView project row; never accept a parent/other root.
    """
    deadline = time.monotonic() + seconds
    observed = "not_written"
    while time.monotonic() < deadline:
        if _native_chooser_still_open():
            return "chooser_reopened"
        observed = _native_catalog_project_disposition()
        if observed in ("wrong_parent_persisted", "unexpected_project_persisted"):
            return observed
        if observed == "exact_native_root_persisted" and _registered_native_project_is_exposed():
            return "registered_exact_root_and_webview"
        time.sleep(0.35)
    return observed if observed != "exact_native_root_persisted" else "catalog_exact_but_ui_missing"


def _registered_native_project_is_exposed():
    # Distinguish the exact project row in the desktop WebView from its name
    # echoed inside a GTK folder chooser. No direct catalog reads or IPC.
    for node in elements():
        try:
            if (node.name or "").strip() != ROOT.name:
                continue
            current = node
            within_dialog = False
            for _ in range(14):
                current = current.parent
                if current is None:
                    break
                role = (current.getRoleName() or "").casefold()
                if "file chooser" in role or "dialog" in role:
                    within_dialog = True
                    break
            if not within_dialog:
                return True
        except Exception:
            continue
    return False


def _deep_visible_gtk_chooser_nodes():
    """Bounded native AT-SPI subtree traversal, never a WebView shortcut.

    GTK's directory list may be deeper than the desktop's 13-level cap.
    Descend only within independently visible native file-chooser subtrees.
    """
    seen = 0
    for chooser in elements():
        try:
            if "file chooser" not in (chooser.getRoleName() or "").casefold():
                continue
            state = chooser.getState()
            if not (state.contains(pyatspi.STATE_SHOWING) and
                    state.contains(pyatspi.STATE_VISIBLE)):
                continue
            stack = [(chooser, 0)]
            while stack and seen < 6000:
                node, depth = stack.pop()
                seen += 1
                yield node
                if depth < 16:
                    stack.extend((child, depth + 1)
                                 for child in children_of(node))
        except Exception:
            continue


def _focus_native_gtk_file_list():
    """Only send typeahead after native GTK's real file list owns focus."""
    for turn in range(14):
        for node in _deep_visible_gtk_chooser_nodes():
            try:
                if not _native_dialog_ancestor(node):
                    continue
                state = node.getState()
                if not state.contains(pyatspi.STATE_FOCUSED):
                    continue
                role = (node.getRoleName() or "").casefold()
                if any(x in role for x in
                       ("table", "tree", "list", "icon", "grid", "cell")):
                    print("P04_GTK_NATIVE_FILE_LIST_FOCUS=verified;tabs=" +
                          str(turn), flush=True)
                    return True
            except Exception:
                continue
        key("Tab")
        time.sleep(0.12)
    print("P04_GTK_NATIVE_FILE_LIST_FOCUS=not_visible_after_bounded_tabs",
          flush=True)
    return False


def _focus_gtk_file_list_via_bounded_pointer():
    """Attempt actual GTK list focus only within a visible OS chooser.

    AT-SPI on the installed Ubuntu 24.04 runner reported zero folder rows
    and no focused list after 14 tabs. A bounded X11 click inside a verified
    visible chooser's content region is an alternative native OS input, not
    an assertion of selection or authority. Exact registration stays required.
    """
    try:
        active = subprocess.check_output(
            ["xdotool", "getactivewindow"], text=True, timeout=5).strip()
        title = subprocess.check_output(
            ["xdotool", "getwindowname", active], text=True,
            timeout=5).strip().casefold()
        allowed = ("open", "open folder", "select folder", "select a folder",
                   "choose folder", "choose a folder", "select")
        if not active.isdigit() or (
                title not in allowed and not any(
                    label in title for label in
                    ("select folder", "choose folder", "open folder"))):
            print("P04_GTK_FILE_LIST_POINTER=active_native_window_not_verified",
                  flush=True)
            return False
        dimensions = subprocess.check_output(
            ["xdotool", "getdisplaygeometry"], text=True,
            timeout=5).split()
        if len(dimensions) != 2:
            return False
        screen_w, screen_h = map(int, dimensions)
        for chooser in elements():
            try:
                if "file chooser" not in (
                        chooser.getRoleName() or "").casefold():
                    continue
                state = chooser.getState()
                if not (state.contains(pyatspi.STATE_SHOWING) and
                        state.contains(pyatspi.STATE_VISIBLE)):
                    continue
                rect = chooser.queryComponent().getExtents(
                    pyatspi.DESKTOP_COORDS)
                if (rect.width < 480 or rect.height < 340 or
                    rect.x < 0 or rect.y < 0 or
                    rect.x + rect.width > screen_w or
                    rect.y + rect.height > screen_h):
                    continue
                # Constrained native chooser body, away from the title,
                # sidebar, path entry, and bottom action buttons.
                x = rect.x + int(rect.width * 0.63)
                y = rect.y + int(rect.height * 0.48)
                if not (rect.x + 200 < x < rect.x + rect.width - 40 and
                        rect.y + 110 < y < rect.y + rect.height - 100):
                    continue
                subprocess.run(["xdotool", "mousemove", "--sync",
                                str(x), str(y)], check=True, timeout=5)
                subprocess.run(["xdotool", "click", "1"],
                               check=True, timeout=5)
                print("P04_GTK_FILE_LIST_POINTER=verified_native_chooser_interior_attempt",
                      flush=True)
                return True
            except (AttributeError, ValueError, OSError,
                    subprocess.CalledProcessError, subprocess.TimeoutExpired):
                continue
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        pass
    print("P04_GTK_FILE_LIST_POINTER=no_verified_native_target", flush=True)
    return False


def _select_real_fixture_directory():
    # SELECT_FOLDER accepts a *selected* directory, not necessarily the
    # directory currently visible. Navigate to its parent via GTK's native
    # location entry, then select the exact ephemeral fixture child.
    exact=0
    for node in _deep_visible_gtk_chooser_nodes():
        try:
            if (node.name or "").strip() != ROOT.name or not _native_dialog_ancestor(node):
                continue
            role=(node.getRoleName() or "").casefold()
            if not any(kind in role for kind in ("row", "cell", "icon", "list item", "text", "label")):
                continue
            exact+=1
            try:
                if node.queryComponent().grabFocus():
                    key("space")
                    print("P04_GTK_FIXTURE_DIRECTORY_SELECTED=real_native_keyboard",flush=True)
                    return True
            except Exception:
                pass
            try:
                parent=node.parent
                index=node.getIndexInParent()
                if parent is not None and index>=0 and parent.querySelection().selectChild(index):
                    print("P04_GTK_FIXTURE_DIRECTORY_SELECTED=real_atspi_selection",flush=True)
                    return True
            except Exception:
                pass
        except Exception:
            continue
    print("P04_GTK_FIXTURE_DIRECTORY_CANDIDATES="+str(exact),flush=True)
    return False


def _native_parent_and_exact_child():
    # The real Ubuntu 24.04 installed run at 5788f9a opened an authentic
    # GTK folder chooser but exposed zero AT-SPI child rows after Alt+Up.
    # Navigate to the *exact ephemeral fixture parent* through GTK's own
    # native location entry; never assume that Alt+Up landed correctly.
    # A keyboard type-ahead of the exact child basename is an OS action,
    # NOT acceptance. Registration and dialog closure are still mandatory.
    if not _native_chooser_still_open():
        return False
    key("ctrl+l")
    type_text(str(ROOT.parent))
    key("Return")
    time.sleep(0.6)
    print("P04_GTK_PARENT_LOCATION=exact_native_parent_submitted", flush=True)
    if _select_real_fixture_directory():
        return True
    # The real Ubuntu GTK chooser did not expose a row at 13 AT-SPI
    # levels. Scan only its bounded native subtree and focus the actual
    # file-list control before typing the exact ephemeral folder name.
    if not _focus_native_gtk_file_list():
        if not _focus_gtk_file_list_via_bounded_pointer():
            return False
    if _select_real_fixture_directory():
        return True
    type_text(ROOT.name)
    print("P04_GTK_CHILD_TYPEAHEAD=native_candidate_preserved_until_real_confirmation",
          flush=True)
    # This is only a native keyboard attempt; modal closure and exact
    # project registration are still required by the caller.
    return True


def select_native_folder():
    # The former parent-path chooser test saw zero AT-SPI rows for the child.
    # GTK's native Ctrl+L location entry accepts an absolute path to the
    # actual directory. Stay inside GTK and require a real OS dialog action.
    try:
        accessible(ROOT.name, seconds=1)
    except RuntimeError:
        pass
    else:
        deny("ephemeral_fixture_was_already_registered_before_picker")
    activate("+ Open folder")
    time.sleep(1)
    # GTK SELECT_FOLDER requires an actual directory selection on some
    # versions. Enter on an absolute path can navigate *inside* the folder
    # without enabling the accept action; the real installed GTK run left
    # the chooser open after four confirmations on that route. First use
    # genuine GTK parent navigation and child selection/typeahead, then
    # fall back to the original exact-path OS keyboard route.
    parent_child_candidate = _native_parent_and_exact_child()
    if parent_child_candidate:
        print("P04_GTK_SELECTION_ROUTE=parent_child_native_candidate", flush=True)
    else:
        key("ctrl+l")
        type_text(str(ROOT))
        key("Return")
        print("P04_GTK_SELECTION_ROUTE=exact_path_native_candidate", flush=True)
    time.sleep(1.4)
    # GTK SELECT_FOLDER may interpret the first accept as "navigate into
    # selected directory" instead of closing. Native CI proved the real
    # dialog remained open afterward. Confirm only while the authentic OS
    # chooser remains; never issue a blind keypress on the main WebView.
    attempts = 0
    while _native_chooser_still_open() and attempts < 4:
        if attempts == 1:
            if parent_child_candidate:
                # A previously accepted parent folder is not evidence that
                # ROOT was selected. Re-enter the real chooser's parent and
                # select its exact child again, rather than approving a
                # potentially unrelated directory.
                parent_child_candidate = _native_parent_and_exact_child()
                print("P04_GTK_PARENT_CHILD_RETRY=native_selection_attempt",
                      flush=True)
            else:
                key("ctrl+l")
                type_text(str(ROOT))
                key("Return")
                print("P04_GTK_EXACT_PATH_SELECTION=exact_fixture_reasserted_in_native_chooser",
                      flush=True)
            time.sleep(0.6)
        elif attempts == 2 and parent_child_candidate:
            # The direct path remains a last bounded native GTK candidate.
            # Neither a successful key dispatch nor dialog closure alone
            # grants acceptance: the catalog must contain exactly ROOT.
            key("ctrl+l")
            type_text(str(ROOT))
            key("Return")
            print("P04_GTK_EXACT_PATH_SELECTION=exact_fixture_reasserted_in_native_chooser",
                  flush=True)
            time.sleep(0.6)
        if not _native_chooser_still_open():
            break
        _native_picker_confirm(attempts)
        attempts += 1
        time.sleep(0.7)
    if _native_chooser_still_open():
        _native_chooser_diagnostics()
        print("P04_NATIVE_GTK_FOLDER_ACCEPT=visible_dialog_still_open_after_bounded_real_actions;"
              "attempts=" + str(attempts),flush=True)
        deny("native_folder_chooser_not_accepted")
    outcome = _wait_for_native_registered_project(seconds=20)
    print("P04_NATIVE_GTK_REGISTRATION_RESULT=" + outcome, flush=True)
    print("P04_NATIVE_GTK_HOST_PICKER_RESULT=" + _host_picker_receipts(), flush=True)
    if outcome != "registered_exact_root_and_webview":
        print("P04_NATIVE_GTK_FOLDER_ACCEPT=closed_without_exact_project_registration",flush=True)
        deny("native_folder_chooser_did_not_register_project")
    print("P04_NATIVE_GTK_FOLDER_ACCEPT=dialog_closed_after_native_action;"
          "confirmations=" + str(attempts),flush=True)
    accessible("Registered projects", seconds=15)
    try:
        accessible(ROOT.name, seconds=15)
    except RuntimeError:
        # Distinguish actual Tauri host catalog from stale UI/selection.
        # Inspect only known OS app-data locations, never open arbitrary
        # paths supplied by the WebView. Emit fixed categories, no filenames.
        import json
        candidates = (
            Path.home() / ".local/share/org.lu.ide/projects.json",
            Path.home() / ".config/org.lu.ide/projects.json",
        )
        catalog_state = "not_written"
        for candidate in candidates:
            if not candidate.is_file() or candidate.is_symlink():
                continue
            try:
                data = json.loads(candidate.read_text(encoding="utf-8"))
                records = data.get("projects", [])
                if not isinstance(records, list):
                    catalog_state = "invalid_records"
                elif any(isinstance(r, dict) and
                         r.get("display_name") == ROOT.name for r in records):
                    catalog_state = "expected_project_persisted"
                elif any(isinstance(r, dict) and
                         Path(str(r.get("root", "/"))).name == ROOT.parent.name
                         for r in records):
                    catalog_state = "wrong_parent_persisted"
                elif records:
                    catalog_state = "unexpected_project_persisted"
                else:
                    catalog_state = "catalog_without_projects"
            except (ValueError, OSError, TypeError):
                catalog_state = "unreadable_catalog"
        print("P04_NATIVE_LINUX_PROJECT_CATALOG=" + catalog_state, flush=True)
        # Only fixed-vocabulary native AT-SPI diagnostics. Never log a user
        # project path, source text, arbitrary accessible names or credentials.
        categories = {"dialog": 0, "file chooser": 0, "button": 0, "text": 0}
        for obj in elements():
            try:
                role = (obj.getRoleName() or "").casefold()
                for label in categories:
                    if label in role:
                        categories[label] += 1
            except Exception:
                pass
        print("P04_NATIVE_LINUX_PICKER_ATSPI_ROLES=" +
              ",".join(k + ":" + str(v) for k, v in sorted(categories.items())), flush=True)
        raise


def editor_project():
    # After a true desktop restart, the persisted project is registered but
    # CLOSED. Reopen through the installed GTK/WebKit accessibility UI; never
    # jump directly into an IPC or assume a saved catalog means an open editor.
    try:
        accessible("Closed · ready to reopen", seconds=2)
    except RuntimeError:
        pass
    else:
        activate("Reopen", timeout=15)
        print("WO8_LINUX_PROJECT_REOPEN=real_atspi_action", flush=True)
    accessible("Open · editor available", seconds=15)
    activate("Edit")
    accessible("Source explorer and editor", seconds=15)
    activate("main.rs")
    focus_editor()


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("dedicated_native_x11_runner_required")
    if not ROOT.is_dir() or not str(ROOT).startswith(str(Path(os.environ["RUNNER_TEMP"]).resolve()) + os.sep):
        deny("non_disposable_fixture")
    if os.environ.get("WO8_P04_REOPEN_ONLY") == "true":
        # A separately installed, reviewed signed version must reopen the
        # original project via the persisted native catalog, not pick it anew.
        if not SOURCE.is_file():
            deny("installed_upgrade_source_missing")
        reopened = start()
        try:
            accessible(ROOT.name, seconds=18)
            editor_project()
            focus_editor()
            print("P04_NATIVE_SIGNED_DEB_REGISTERED_PROJECT_REOPEN=PASS", flush=True)
        finally:
            stop(reopened)
        return
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
