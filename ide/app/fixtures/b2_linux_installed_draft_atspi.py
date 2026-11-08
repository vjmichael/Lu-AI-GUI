#!/usr/bin/env python3
"""B2 real installed .deb GTK/AT-SPI draft and restart recovery acceptance.

Uses the existing WO8 real native chooser/editor AT-SPI driver. No Tauri
IPC, browser JS, file-based injection or faked editor state. Fails closed.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time
from wo8_linux_editor_atspi import (
    ROOT,SOURCE,accessible,activate,key,type_text,focus_editor,
    select_native_folder,editor_project,start,stop,
)
SENTINEL="B2_PRIVATE_UNSAVED_DRAFT_ONLY"

def deny(kind):
    raise RuntimeError("B2_LINUX_INSTALLED_EDITOR_DENIED:"+kind)

def clipboard():
    for _ in range(35):
        try:
            text=subprocess.check_output(
                ["xclip","-selection","clipboard","-out"],
                text=True,timeout=4,stderr=subprocess.DEVNULL)
            if text.startswith(("ide-buffer:v1","ide-draft:v1")):
                return text
        except (OSError,subprocess.CalledProcessError,subprocess.TimeoutExpired):
            pass
        time.sleep(.4)
    deny("real_os_clipboard_missing_bounded_draft_reference")

def main():
    if os.environ.get("GITHUB_ACTIONS")!="true" or not os.environ.get("DISPLAY"):
        deny("real_installed_linux_x11_session_required")
    if not ROOT.is_dir() or not str(ROOT).startswith(
            str(Path(os.environ["RUNNER_TEMP"]).resolve())+os.sep):
        deny("non_disposable_project")
    if not SOURCE.is_file() or SENTINEL in SOURCE.read_text(encoding="utf-8"):
        deny("missing_pristine_original_source")
    source_before=hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    first=start()
    try:
        select_native_folder()
        editor_project()
        focus_editor()
        key("ctrl+End")
        key("Return")
        type_text("// "+SENTINEL)
        time.sleep(.8)
        if hashlib.sha256(SOURCE.read_bytes()).hexdigest()!=source_before:
            deny("editing_implicitly_modified_source_file")
        activate("Copy draft reference")
        session=clipboard()
        if ("lifetime=session-only" not in session or
            "draftSha256=" not in session or SENTINEL in session):
            deny("session_reference_not_bounded_non_disclosing")
        activate("Preserve draft + copy durable reference")
        durable=clipboard()
        if not durable.startswith("ide-draft:v1") or "checkpoint=" not in durable or (
            "lifetime=persistent-private-app-data" not in durable
            or SENTINEL in durable):
            deny("durable_checkpoint_reference_not_verified")
        if hashlib.sha256(SOURCE.read_bytes()).hexdigest()!=source_before:
            deny("durable_reference_changed_original_project_source")
        print("B2_LINUX_REAL_INSTALLED_PERSISTENT_REFERENCE_CLIPBOARD=PASS",flush=True)
    finally:
        stop(first)

    second=start()
    try:
        accessible(ROOT.name,seconds=18)
        editor_project()
        recover=accessible("Recover preserved draft",seconds=20)
        if not recover:
            deny("restarted_editor_did_not_offer_private_recovery")
        activate("Recover preserved draft")
        accessible("Recovered private checkpoint to editor only",seconds=18)
        if hashlib.sha256(SOURCE.read_bytes()).hexdigest()!=source_before:
            deny("recovery_wrote_project_source_without_save")
        print("B2_LINUX_REAL_INSTALLED_RESTART_RECOVERY_NO_SOURCE_WRITE=PASS",flush=True)
    finally:
        stop(second)

if __name__=="__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc),file=sys.stderr)
        sys.exit(1)
