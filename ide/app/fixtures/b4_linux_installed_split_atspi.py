#!/usr/bin/env python3
"""B4 genuine installed Debian/GTK split CodeMirror source separation.

Uses real OS AT-SPI and xdotool events; no browser JS, Tauri IPC or file
injection into the editor. Fails if any visible editor/UI control is missing.
"""
import hashlib
import os
from pathlib import Path
import sys
import time
import pyatspi
from wo8_linux_editor_atspi import (
    ROOT,SOURCE,accessible,activate,key,type_text,start,stop,
    select_native_folder,editor_project,elements,
)
SECOND=ROOT/"second.rs"
PRIMARY_SENTINEL="B4_LINUX_PRIMARY_ONLY"
SECONDARY_SENTINEL="B4_LINUX_SECONDARY_ONLY"

def deny(reason):
    raise RuntimeError("B4_LINUX_SPLIT_INSTALLED_DENIED:"+reason)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def real_code_editors():
    verified={}
    expected=("Source editor text · First editor: second.rs",
              "Source editor text · Second editor: main.rs")
    for obj in elements():
        try:
            if (obj.name or "").strip() not in expected:
                continue
            rect=obj.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            if rect.width<60 or rect.height<40:
                continue
            key=(round(rect.x),round(rect.y),round(rect.width),round(rect.height))
            if key not in verified:
                verified[key]=(obj,rect)
        except Exception:
            continue
    if len(verified)!=2:
        deny("expected_two_distinct_native_atspi_editor_surfaces_"+str(len(verified)))
    by_name={name:[obj for obj,_ in verified.values() if obj.name==name]
             for name in expected}
    if any(len(items)!=1 for items in by_name.values()):
        deny("split_editor_native_identity_mismatch")
    return [by_name[name][0] for name in expected]

def focus(node):
    try:
        if node.queryComponent().grabFocus():
            return
    except Exception:
        pass
    deny("split_editor_native_focus_unavailable")

def wait_saved(path,marker):
    limit=time.monotonic()+18
    while time.monotonic()<limit:
        if marker in path.read_text(encoding="utf-8"):
            return
        time.sleep(.35)
    deny("installed_explicit_save_did_not_persist_"+marker)

def main():
    if os.environ.get("GITHUB_ACTIONS")!="true" or not os.environ.get("DISPLAY"):
        deny("genuine_installed_linux_gui_required")
    temp=Path(os.environ["RUNNER_TEMP"]).resolve()
    if not ROOT.is_dir() or not str(ROOT).startswith(str(temp)+os.sep):
        deny("non_disposable_fixture")
    if not SOURCE.is_file() or not SECOND.is_file():
        deny("two_registered_fixture_files_missing")
    first=digest(SOURCE)
    second=digest(SECOND)
    proc=start()
    try:
        select_native_folder()
        editor_project()  # Picks main.rs through the real GTK chooser.
        activate("second.rs")
        activate("Toggle split editor")
        accessible("Second editor file",seconds=15)
        accessible("Save second file",seconds=15)
        primary,secondary=real_code_editors()
        focus(secondary)
        key("ctrl+End")
        key("Return")
        type_text("// "+SECONDARY_SENTINEL)
        time.sleep(.65)
        if digest(SOURCE)!=first or digest(SECOND)!=second:
            deny("unsaved_editor_modified_source")
        activate("Save second file")
        wait_saved(SOURCE,SECONDARY_SENTINEL)
        if digest(SECOND)!=second:
            deny("secondary_save_modified_primary_source")
        main_saved=digest(SOURCE)
        primary,secondary=real_code_editors()
        focus(primary)
        key("ctrl+End")
        key("Return")
        type_text("// "+PRIMARY_SENTINEL)
        time.sleep(.55)
        if digest(SECOND)!=second:
            deny("primary_editor_implicitly_saved_before_ctrl_s")
        key("ctrl+s")
        wait_saved(SECOND,PRIMARY_SENTINEL)
        if digest(SOURCE)!=main_saved:
            deny("focused_ctrl_s_modified_secondary_source")
        print("B4_REAL_INSTALLED_LINUX_TWO_DISTINCT_EDITOR_PANES=PASS",flush=True)
        print("B4_REAL_INSTALLED_LINUX_SEPARATE_SECONDARY_SAVE_AND_FOCUSED_CTRL_S=PASS",flush=True)
    finally:
        stop(proc)

if __name__=="__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc),file=sys.stderr)
        sys.exit(1)
