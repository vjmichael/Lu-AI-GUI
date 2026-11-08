#!/usr/bin/env python3
"""Actual installed Debian GTK/WebKit shell witness for technical-only LSP assets.

No server or project script is launched. The picker must be the genuine OS
dialog. This intentionally does not claim protected LSP execution acceptance.
"""
import os
from pathlib import Path
import subprocess
import sys
import time

import wo8_linux_editor_atspi as native


def main():
    if os.environ.get("GITHUB_ACTIONS")!="true" or not os.environ.get("DISPLAY"):
        raise RuntimeError("technical installed GUI requires disposable X11 CI")
    exe=Path(os.environ["WO8_INSTALLED_EXE"])
    root=Path(os.environ["WO8_FIXTURE_ROOT"])
    if exe!=Path("/usr/bin/lu-ide-desktop") or not exe.is_file() or (
        not str(root).startswith(str(Path(os.environ["RUNNER_TEMP"]).resolve())+os.sep)
    ):
        raise RuntimeError("not a genuine installed Lu IDE or disposable fixture")
    app=native.start()
    try:
        native.accessible("Workspace projects",seconds=25)
        native.activate("+ Open folder")
        deadline=time.monotonic()+15
        found=False
        while time.monotonic()<deadline:
            if native._native_chooser_still_open():
                found=True
                break
            time.sleep(0.25)
        if not found:
            raise RuntimeError("real native GTK folder chooser was not visible")
        print("DEV02_TECHNICAL_INSTALLED_GTK_OS_CHOOSER=PASS",flush=True)
        native.key("Escape")
        native.accessible("Workspace projects",seconds=15)
        print("DEV02_TECHNICAL_INSTALLED_GTK_NATIVE_APP_LAUNCH=PASS",flush=True)
    finally:
        native.stop(app)


if __name__=="__main__":
    try:
        main()
    except Exception as exc:
        print("DENIED: "+str(exc),file=sys.stderr,flush=True)
        raise
