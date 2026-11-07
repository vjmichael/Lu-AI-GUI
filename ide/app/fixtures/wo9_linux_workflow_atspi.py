#!/usr/bin/env python3
"""WO9-F installed GTK/X11/AT-SPI portfolio review and ZIP export.

Uses only the real accessibility tree, OS focus/keyboard and native GTK folder
picker. Never invokes Tauri IPC or mocks a release result.
"""
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile
import wo8_linux_editor_atspi as ui

def deny(code):
    raise RuntimeError("WO9_INSTALLED_LINUX_DENIED:" + code)

def main():
    root = ui.ROOT
    export = Path(os.environ["WO9_EXPORT_ROOT"]).resolve()
    if not root.is_dir() or not export.is_dir() or export.is_relative_to(root):
        deny("non_disposable_or_inside_project_export")
    original = (root / "dist" / "verified.tar").read_bytes()
    app = ui.start()
    try:
        ui.select_native_folder()
        ui.activate("Packages & workflows")
        ui.accessible("Multi-project release review", seconds=25)
        checkbox = ui.accessible(root.name, role="check box", seconds=15)
        try:
            action = checkbox.queryAction()
            if not any(action.doAction(i) for i in range(action.nActions)
                       if action.getName(i).lower() in ("click", "press", "activate")):
                deny("native_portfolio_checkbox_not_clickable")
        except Exception as exc:
            deny("native_portfolio_checkbox_unavailable_" + type(exc).__name__)
        ui.activate("Inspect selected workflow")
        ui.accessible("Native workflow review", seconds=35)
        ui.accessible("Eligible for human source review", seconds=15)
        ui.activate("Verify frozen Git and artifact evidence again")
        ui.accessible("Exact evidence matched", seconds=20)
        ui.activate("Export verified artifacts as ZIP")
        time.sleep(1)
        ui.key("ctrl+l")
        ui.type_text(str(export))
        ui.key("Return")
        time.sleep(1)
        for caption in ("Select", "Open", "Choose Folder"):
            try:
                ui.activate(caption, seconds=2)
                break
            except RuntimeError:
                continue
        deadline = time.monotonic() + 35
        files = []
        while time.monotonic() < deadline:
            files = list(export.glob("lu-ide-workflow-*.zip"))
            if len(files) == 1:
                break
            time.sleep(0.4)
        if len(files) != 1:
            deny("native_zip_not_created")
        with zipfile.ZipFile(files[0]) as bundle:
            names = bundle.namelist()
            if "workflow-manifest.json" not in names or len(names) != 2:
                deny("archive_entries_not_scoped")
            if not any(name.endswith("/dist/verified.tar") for name in names):
                deny("verified_artifact_missing")
            if not any(bundle.read(name) == original for name in names if name.endswith("/dist/verified.tar")):
                deny("archive_bytes_differ_from_source")
        if (root / "dist" / "verified.tar").read_bytes() != original:
            deny("native_export_mutated_user_project")
        print("WO9_INSTALLED_LINUX_NATIVE_PORTFOLIO_AND_VERIFIED_ZIP=PASS", flush=True)
        print("WO9_INSTALLED_SIGNED_RELEASE=NOT_ACCEPTED", flush=True)
    finally:
        ui.stop(app)

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(1)
