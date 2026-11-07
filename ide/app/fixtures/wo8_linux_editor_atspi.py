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


def _native_picker_confirm():
    candidates = ("select", "open", "choose folder")
    checked = 0
    for obj in elements():
        try:
            name = (obj.name or "").strip().casefold()
            role = (obj.getRoleName() or "").casefold()
            if name in candidates and ("button" in role) and _native_dialog_ancestor(obj):
                checked += 1
                action = obj.queryAction()
                for i in range(action.nActions):
                    if action.getName(i).casefold() in ("click", "press", "activate"):
                        if action.doAction(i):
                            print("P04_NATIVE_LINUX_GTK_DIALOG_CONFIRMATION=real_atspi_action", flush=True)
                            return
        except Exception:
            continue
    print("P04_NATIVE_LINUX_GTK_DIALOG_CONFIRMATION=missing_real_chooser_button;"
          "matching_dialog_buttons=" + str(checked), flush=True)
    deny("native_dialog_confirmation_not_found")


def select_native_folder():
    activate("+ Open folder")
    time.sleep(1)
    key("ctrl+l")
    type_text(str(ROOT))
    key("Return")
    time.sleep(1)
    _native_picker_confirm()
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
