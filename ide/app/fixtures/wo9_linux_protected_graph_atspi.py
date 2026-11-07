#!/usr/bin/env python3
"""WO9-F signed Linux installed two-step protected OS worker DAG acceptance.
Only real GTK/AT-SPI controls and separate native approvals; no private IPC.
"""
import os
import sys
import time
import wo8_linux_editor_atspi as ui

def deny(reason):
    raise RuntimeError("WO9_SIGNED_LINUX_DENIED:" + reason)

def wait(name, seconds=100):
    return ui.accessible(name, seconds=seconds)

def main():
    if os.environ.get("WO9_NATIVE_SIGNED_PACKAGE_VERIFIED") != "true":
        deny("signed_linux_package_admission_missing")
    app = ui.start()
    try:
        ui.select_native_folder()
        ui.activate("Packages & workflows")
        wait("Multi-project release review", 30)
        checkbox = ui.accessible(ui.ROOT.name, role="check box", seconds=20)
        action = checkbox.queryAction()
        if not any(action.doAction(i) for i in range(action.nActions)
                   if action.getName(i).lower() in ("click", "press", "activate")):
            deny("project_native_checkbox_inaccessible")
        ui.activate("Inspect selected workflow")
        wait("Eligible for human source review")
        ui.activate("Verify frozen Git and artifact evidence again")
        wait("Exact evidence matched")
        ui.activate("Add dependent step")
        ui.activate("Add dependent step")
        wait("Step 2")
        ui.activate("Record exact dependency workflow")
        wait("Durable dependent build/test workflow")
        for step in (1, 2):
            ui.activate("Request separately approved protected step", seconds=30)
            # This must be a genuine Tauri/GTK one-use confirmation, produced by
            # protected worker runtime after source and OS resource verification.
            ui.activate("Run this exact job", seconds=60)
            wait("Native step returned", 360)
            wait("passed", 35)
            print("WO9_LINUX_NATIVE_SIGNED_PROTECTED_STEP_"+str(step)+"=PASS", flush=True)
        wait("Auto-run: disabled")
        wait("Release approved: no")
        print("WO9_SIGNED_LINUX_INSTALLED_TWO_STEP_PROTECTED_GRAPH=PASS", flush=True)
    finally:
        ui.stop(app)

if __name__ == "__main__":
    try: main()
    except Exception as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
