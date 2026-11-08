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


def _native_dialog_ancestor(node):
    # Inspect an actual AT-SPI modal/file-chooser ancestry, not an unrelated
    # app button named Open or Select. No web JS, IPC or synthetic picker.
    current = node
    for _ in range(10):
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
        key("Return" if attempt % 2 == 0 else "alt+o")
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
            # GTK action row occupies the bottom-right portion. Both attempts
            # stay entirely within the actual visible native chooser widget.
            margin = 95 if attempt % 2 == 0 else 55
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


def _native_picker_confirm(attempt=0):
    # The previous real Ubuntu 26.04 run clicked an authentic AT-SPI
    # chooser action THREE times and the dialog never closed. A generic
    # "Open" can also be GTK's navigation button. Prefer actual Select
    # Folder and bottom-right action geometry, retaining native ancestry.
    priority = {"select folder": 0, "choose folder": 1,
                "select": 2, "open folder": 3, "open": 4}
    ranked = []
    for obj in elements():
        try:
            name = (obj.name or "").strip().casefold()
            role = (obj.getRoleName() or "").casefold()
            if name not in priority or "button" not in role or not _native_dialog_ancestor(obj):
                continue
            if not obj.getState().contains(pyatspi.STATE_ENABLED):
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
    # Consecutive failed OS confirmation attempts must not hit the same
    # first generic toolbar Open when multiple native candidates exist.
    start = min(attempt, max(0, len(ranked) - 1))
    for item in ranked[start:] + ranked[:start]:
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
    # A separately witnessed native GTK X11 dialog may have zero exposed
    # AT-SPI action buttons. Try bounded real OS keyboard acceptance instead.
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


def _select_real_fixture_directory():
    # SELECT_FOLDER accepts a *selected* directory, not necessarily the
    # directory currently visible. Navigate to its parent via GTK's native
    # location entry, then select the exact ephemeral fixture child.
    exact=0
    for node in elements():
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
    key("ctrl+l")
    type_text(str(ROOT))
    key("Return")
    time.sleep(1.4)
    # GTK SELECT_FOLDER may interpret the first accept as "navigate into
    # selected directory" instead of closing. Native CI proved the real
    # dialog remained open afterward. Confirm only while the authentic OS
    # chooser remains; never issue a blind keypress on the main WebView.
    attempts = 0
    while _native_chooser_still_open() and attempts < 4:
        if attempts == 1:
            # GTK SELECT_FOLDER may have navigated INSIDE the target instead
            # of selecting its directory row. Use only a real native dialog
            # keyboard to go up and select the exact fixture child. If GTK
            # does not expose that row, reassert the exact path in its native
            # location entry without blindly accepting an unrelated window.
            key("alt+Up")
            time.sleep(0.7)
            if not _select_real_fixture_directory():
                key("ctrl+l")
                type_text(str(ROOT))
                print("P04_GTK_EXACT_PATH_ENTRY=reasserted_in_native_chooser",flush=True)
                time.sleep(0.5)
        if not _native_chooser_still_open():
            break
        _native_picker_confirm(attempts)
        attempts += 1
        time.sleep(0.7)
    if _native_chooser_still_open():
        print("P04_NATIVE_GTK_FOLDER_ACCEPT=visible_dialog_still_open_after_bounded_real_actions;"
              "attempts=" + str(attempts),flush=True)
        deny("native_folder_chooser_not_accepted")
    if not _registered_native_project_is_exposed():
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
