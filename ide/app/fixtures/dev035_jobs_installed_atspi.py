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
    deny, children_of,
)


def native_empty_job_journal(seconds=16):
    """Validate the actual empty projected lifecycle *inside* the Jobs region.

    The old installed GTK test searched a plain <p> by accessible name.
    WebKitGTK need not expose paragraph text as an accessible object.
    React now exposes role=status only when the real native projected jobs
    list is empty. The named Jobs section and status must come from the
    installed WebView accessibility tree, not from source, IPC or a test
    fixture. This is a read-only witness, never worker authorization.
    """
    import time
    limit = time.monotonic() + seconds
    last_region = 0
    last_exact_status = 0
    last_job_lists = 0
    while time.monotonic() < limit:
        heading = accessible("DEV-03.5 · Protected job lifecycle", seconds=2)
        region = None
        current = heading
        for _ in range(8):
            try:
                current = current.parent
                if current is None:
                    break
                if (current.name or "").strip() == "DEV-03.5 verified protected job lifecycle":
                    region = current
                    break
            except Exception:
                break
        last_region = 1 if region is not None else 0
        exact_status = 0
        lists = 0
        if region is not None:
            stack = [(region, 0)]
            visited = 0
            while stack and visited < 3200:
                node, depth = stack.pop()
                visited += 1
                try:
                    name = (node.name or "").strip()
                    role = (node.getRoleName() or "").casefold()
                    state = node.getState()
                    if ("status" in role and
                            name == "No matching protected job journal entries observed" and
                            state.contains(pyatspi.STATE_VISIBLE)):
                        exact_status += 1
                    if role in ("list", "list box", "listbox"):
                        lists += 1
                    if depth < 24:
                        stack.extend((child, depth + 1) for child in children_of(node))
                except Exception:
                    continue
        last_exact_status, last_job_lists = exact_status, lists
        if last_region == 1 and exact_status == 1 and lists == 0:
            print("DEV035_LINUX_NATIVE_EMPTY_JOURNAL_STATUS=verified_region_and_status",
                  flush=True)
            return
        time.sleep(0.3)
    # Bounded fixed-category reporting: never log raw AT-SPI values, source
    # text, paths, journal content, process output or arbitrary identifiers.
    print("DEV035_LINUX_NATIVE_JOURNAL_INVENTORY=region:" +
          str(last_region) + ";exact_status:" + str(min(last_exact_status,32)) +
          ";lists:" + str(min(last_job_lists,32)), flush=True)
    deny("dev035_installed_empty_journal_status_not_verified")



def native_bounded_test_discovery_status(seconds=18):
    """Inspect actual installed WebView DEV-03.4 native discovery result.

    This is only rendered by React when native discovery returned a result
    belonging to the selected project. Do not trust generic page prose or
    synthetic completion flags. Fail if a real status is not exposed.
    """
    import time
    deadline = time.monotonic() + seconds
    last_region = 0
    last_status = 0
    last_alert = 0
    while time.monotonic() < deadline:
        heading = accessible("DEV-03.4 · Conventional test files", seconds=2)
        region = None
        current = heading
        for _ in range(8):
            try:
                current = current.parent
                if current is None:
                    break
                if (current.name or "").strip() == "DEV-03.4 bounded test discovery":
                    region = current
                    break
            except Exception:
                break
        last_region = 1 if region is not None else 0
        matches = 0
        if region is not None:
            visited = 0
            stack = [(region, 0)]
            while stack and visited < 3200:
                node, depth = stack.pop()
                visited += 1
                try:
                    name = (node.name or "").strip()
                    role = (node.getRoleName() or "").casefold()
                    state = node.getState()
                    if ("status" in role and
                            name == "Conventional test discovery incomplete by design" and
                            state.contains(pyatspi.STATE_VISIBLE)):
                        matches += 1
                    if depth < 24:
                        stack.extend((child, depth + 1) for child in children_of(node))
                except Exception:
                    continue
        alerts = 0
        # Only category counts, never print arbitrary accessible text.
        for node in accessible_error_candidates():
            alerts += 1
        last_status, last_alert = matches, alerts
        if region is not None and matches == 1 and alerts == 0:
            print("DEV035_LINUX_NATIVE_TEST_DISCOVERY_STATUS=verified_native_result",
                  flush=True)
            return
        time.sleep(0.3)
    print("DEV035_LINUX_NATIVE_DISCOVERY_INVENTORY=region:" +
          str(last_region) + ";status:" + str(min(last_status,32)) +
          ";app_alert:" + str(min(last_alert,32)), flush=True)
    deny("dev035_installed_native_test_discovery_not_verified")


def accessible_error_candidates():
    """Bounded read-only check for a visible application error alert."""
    from wo8_linux_editor_atspi import elements
    for node in elements():
        try:
            state = node.getState()
            role = (node.getRoleName() or "").casefold()
            name = (node.name or "").strip()
            if ("alert" in role and
                    name.startswith("Test discovery unavailable:") and
                    state.contains(pyatspi.STATE_VISIBLE)):
                yield node
        except Exception:
            continue


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
        native_empty_job_journal(seconds=16)
        activate("Discover test files")
        native_bounded_test_discovery_status(seconds=18)
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
