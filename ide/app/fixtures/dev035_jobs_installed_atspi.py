#!/usr/bin/env python3
"""DEV-03.5 real installed GTK Jobs read-only acceptance.

Uses only the existing AT-SPI and OS-keyboard harness against the installed
Debian executable. No Tauri IPC, JS, mock process, or false worker acceptance.
"""
import os
from pathlib import Path

import pyatspi

from wo8_linux_editor_atspi import (
    ROOT, SOURCE, accessible, activate, editor_project, start, stop, digest,
    deny,
)

def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("DISPLAY"):
        deny("dev035_disposable_native_gtk_required")
    if not ROOT.is_dir() or not SOURCE.is_file():
        deny("dev035_disposable_registered_project_required")
    old=digest()
    proc=start()
    try:
        accessible(ROOT.name,seconds=18)
        editor_project()  # actual persisted registered project reopen in GTK
        activate("Job requests")
        accessible("DEV-03.4 · Conventional test files",seconds=16)
        accessible("DEV-03.3B · Live protected worker output",seconds=16)
        accessible("DEV-03.5 · Protected job lifecycle",seconds=16)
        accessible("No matching protected job journal entries observed",seconds=16)
        activate("Discover test files")
        accessible("discovery incomplete by design",seconds=18)
        run=accessible("Request protected Run / Test",role="push button",seconds=16)
        if run.getState().contains(pyatspi.STATE_ENABLED):
            deny("dev035_unsigned_gtk_admitted_protected_execution")
        if digest()!=old:
            deny("dev035_jobs_inspection_changed_source")
        print("DEV035_LINUX_INSTALLED_JOBS_DISCOVERY_NO_RUN=PASS",flush=True)
        print("DEV035_LINUX_INSTALLED_PROTECTED_WORKER=NOT_ACCEPTED",flush=True)
    finally:
        stop(proc)

if __name__=="__main__":
    main()
