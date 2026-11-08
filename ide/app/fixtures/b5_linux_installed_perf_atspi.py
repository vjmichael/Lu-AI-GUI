#!/usr/bin/env python3
"""B5 genuine installed GTK/WebKit editor UI and descendant RSS evidence.

Only native X11/AT-SPI input, /proc kernel observations and source hashes.
No WebView JS injection, mocked Tauri API, or artificial CI benchmark pass.
The timings are OS-action-to-verified-visible-editor round trips, not render
frames or isolated filesystem throughput. They include clipboard verification.
"""
import hashlib
import json
import os
import re
from pathlib import Path
import statistics
import subprocess
import sys
import time

from wo8_linux_editor_atspi import (
    ROOT, SOURCE, accessible, activate, key, type_text, focus_editor,
    select_native_folder, editor_project, start, stop,
)

COUNT=47
SOURCE_NAMES=[f"p{i:02d}.rs" for i in range(COUNT)]
EXTRA="p47.rs"
PERF_SENTINEL="B5_EXPLICIT_SAVE_PERF_ONLY"

def deny(reason):
    raise RuntimeError("B5_LINUX_INSTALLED_PERFORMANCE_DENIED:"+reason)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def process_family(root_pid):
    # Only the actual installed app and its current kernel descendants.
    # This misses OS-reparented processes: scope and count are reported.
    ppid={}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal(): continue
        try:
            raw=(entry/"stat").read_text()
            fields=raw.rsplit(") ",1)[1].split()
            ppid[int(entry.name)]=int(fields[1])
        except (OSError,ValueError,IndexError): continue
    seen={root_pid}
    for _ in range(12):
        children={pid for pid,parent in ppid.items() if parent in seen}
        updated=seen|children
        if len(updated)==len(seen): break
        if len(updated)>128: deny("unbounded_kernel_process_family")
        seen=updated
    return seen

def sample_rss(pid,stage):
    children=process_family(pid)
    rss=0
    vmsize=0
    observed=0
    for child in children:
        try:
            status=(Path("/proc")/str(child)/"status").read_text()
            props={}
            for line in status.splitlines():
                if line.startswith(("VmRSS:","VmSize:")):
                    name,value,*_=line.split()
                    props[name.rstrip(":")]=int(value)*1024
            if "VmRSS" not in props: continue
            rss+=props["VmRSS"]
            vmsize+=props.get("VmSize",0)
            observed+=1
        except (OSError,ValueError): continue
    if not observed or rss<=0: deny("installed_process_rss_unavailable")
    return {"stage":stage,"rssBytes":rss,"virtualBytes":vmsize,
            "observedProcessCount":observed,"scope":"installed-pid-and-live-proc-descendants"}

def clipboard():
    try:
        return subprocess.check_output(["xclip","-selection","clipboard","-out"],
            text=True,stderr=subprocess.DEVNULL,timeout=5)
    except (OSError,subprocess.TimeoutExpired,subprocess.CalledProcessError):
        return ""

def assert_current_editor(marker,timeout=15):
    until=time.perf_counter()+timeout
    while time.perf_counter()<until:
        focus_editor()
        key("ctrl+a")
        key("ctrl+c")
        time.sleep(.10)
        if marker in clipboard(): return
        time.sleep(.20)
    deny("installed_editor_content_not_verified_"+marker)

def navigate_relative(name,marker):
    entry=accessible("Open project-relative file",seconds=10)
    try:
        if not entry.queryComponent().grabFocus():
            deny("native_relative_open_input_not_focusable")
    except Exception:
        deny("native_relative_open_input_not_focusable")
    start_ns=time.perf_counter_ns()
    key("ctrl+a")
    type_text(name)
    key("Return")
    assert_current_editor(marker)
    return round((time.perf_counter_ns()-start_ns)/1e6,3)

def choose_tab(direction,marker):
    picker=accessible("Jump to open file",seconds=8)
    try:
        if not picker.queryComponent().grabFocus():
            deny("native_tab_jump_not_focusable")
    except Exception:
        deny("native_tab_jump_not_focusable")
    start_ns=time.perf_counter_ns()
    if direction=="first":
        key("Home")
        key("Down")   # skip the actual native empty "Open tabs" option
    else:
        key("End")
    key("Return")
    assert_current_editor(marker)
    return round((time.perf_counter_ns()-start_ns)/1e6,3)

def percentile(samples,p):
    sorted_values=sorted(samples)
    index=max(0,min(len(sorted_values)-1,round((len(sorted_values)-1)*p)))
    return sorted_values[index]

def main():
    if os.environ.get("GITHUB_ACTIONS")!="true" or not os.environ.get("DISPLAY"):
        deny("real_x11_installed_session_required")
    tmp=Path(os.environ["RUNNER_TEMP"]).resolve()
    if not ROOT.is_dir() or not str(ROOT).startswith(str(tmp)+os.sep):
        deny("non_disposable_native_picker_fixture")
    files=[SOURCE]+[ROOT/name for name in SOURCE_NAMES]+[ROOT/EXTRA]
    if any(not path.is_file() or path.is_symlink() for path in files):
        deny("missing_or_symlinked_native_test_sources")
    originals={p.name:sha(p) for p in files}
    if SOURCE.stat().st_size<130*1024 or SOURCE.stat().st_size>512*1024:
        deny("large_file_fixture_outside_native_128_512_kib_window")
    exact=os.environ.get("B5_EXACT_CHECKOUT_SHA","")
    if not re.fullmatch(r"[a-f0-9]{40}",exact):
        deny("missing_exact_checkout_sha")
    observed=subprocess.check_output(["git","rev-parse","HEAD"],
        text=True,timeout=5).strip()
    if observed!=exact: deny("perf_receipt_source_sha_not_checked_out")
    metric={"schemaVersion":1,"sourceCommit":exact,
        "os":"ubuntu-24.04","method":"installed-GTK-WebKit-native-AT-SPI-X11-and-proc",
        "sourceBytes":SOURCE.stat().st_size,
        "fileCountToOpen":48,
        "openRoundTripMs":[],"switchRoundTripMs":[],
        "memorySamples":[],"keyboardRoundTripMs":None,"undoRoundTripMs":None,
        "saveRoundTripMs":None,"explicitSaveVerified":False,
        "bounded49thDenial":False,"noImplicitSaveVerified":False}
    proc=start()
    try:
        started=time.perf_counter_ns()
        select_native_folder()
        editor_project()
        assert_current_editor("B5_SOURCE_main")
        metric["firstSourceActivationMs"]=round((time.perf_counter_ns()-started)/1e6,3)
        metric["memorySamples"].append(sample_rss(proc.pid,"initial_large_source"))
        for i,name in enumerate(SOURCE_NAMES):
            metric["openRoundTripMs"].append(navigate_relative(
                name,"B5_SOURCE_"+name.removesuffix(".rs")))
            if i+2 in (12,24,36,48):
                metric["memorySamples"].append(sample_rss(proc.pid,
                    "opened_"+str(i+2)+"_distinct_files"))
        # A 49th source must be denied, never silently evict old Undo/draft.
        entry=accessible("Open project-relative file",seconds=8)
        if not entry.queryComponent().grabFocus():
            deny("relative_file_input_lost_after_full_working_set")
        key("ctrl+a")
        type_text(EXTRA)
        key("Return")
        accessible("Open-file budget",seconds=10)
        metric["bounded49thDenial"]=True
        if any(sha(path)!=originals[path.name] for path in files):
            deny("unsaved_multi_tab_open_changed_source")
        metric["noImplicitSaveVerified"]=True
        for iteration in range(6):
            metric["switchRoundTripMs"].append(choose_tab("first","B5_SOURCE_main"))
            metric["switchRoundTripMs"].append(choose_tab("last","B5_SOURCE_p46"))
        choose_tab("first","B5_SOURCE_main")
        focus_editor()
        key("ctrl+End")
        key("Return")
        started=time.perf_counter_ns()
        type_text("// "+PERF_SENTINEL)
        assert_current_editor(PERF_SENTINEL)
        metric["keyboardRoundTripMs"]=round((time.perf_counter_ns()-started)/1e6,3)
        if sha(SOURCE)!=originals[SOURCE.name]:
            deny("typing_modified_native_source_before_save")
        focus_editor()
        started=time.perf_counter_ns()
        key("ctrl+z")
        until=time.perf_counter()+8
        while time.perf_counter()<until:
            focus_editor()
            key("ctrl+a")
            key("ctrl+c")
            time.sleep(.12)
            if PERF_SENTINEL not in clipboard(): break
        else: deny("native_editor_undo_not_observed")
        metric["undoRoundTripMs"]=round((time.perf_counter_ns()-started)/1e6,3)
        focus_editor()
        key("ctrl+End")
        key("Return")
        type_text("// "+PERF_SENTINEL)
        started=time.perf_counter_ns()
        activate("Save · Ctrl+S")
        until=time.perf_counter()+20
        while time.perf_counter()<until:
            if PERF_SENTINEL in SOURCE.read_text(encoding="utf-8"): break
            time.sleep(.20)
        else: deny("real_explicit_save_never_persisted")
        metric["saveRoundTripMs"]=round((time.perf_counter_ns()-started)/1e6,3)
        metric["explicitSaveVerified"]=True
        if any(sha(path)!=originals[path.name] for path in files if path!=SOURCE):
            deny("saving_primary_modified_other_registered_source")
        metric["memorySamples"].append(sample_rss(proc.pid,"after_explicit_save"))
        opens=metric["openRoundTripMs"]
        switches=metric["switchRoundTripMs"]
        metric["openMedianMs"]=round(statistics.median(opens),3)
        metric["openP95Ms"]=round(percentile(opens,.95),3)
        metric["switchMedianMs"]=round(statistics.median(switches),3)
        metric["switchP95Ms"]=round(percentile(switches,.95),3)
        metric["rssPeakObservedBytes"]=max(p["rssBytes"] for p in metric["memorySamples"])
        if len(opens)!=47 or len(switches)!=12 or len(metric["memorySamples"])<6:
            deny("incomplete_real_installed_measurements")
        out=tmp/"ide-b5-linux-installed-performance.json"
        out.write_text(json.dumps(metric,sort_keys=True,indent=2),encoding="utf-8")
        print("B5_LINUX_INSTALLED_48_SOURCE_RSS_AND_UI_ROUNDTRIPS=MEASURED",flush=True)
        print("B5_LINUX_INSTALLED_OPEN_P95_MS="+str(metric["openP95Ms"]),flush=True)
        print("B5_LINUX_INSTALLED_SWITCH_P95_MS="+str(metric["switchP95Ms"]),flush=True)
        print("B5_LINUX_INSTALLED_RSS_OBSERVED_PEAK_BYTES="+str(metric["rssPeakObservedBytes"]),flush=True)
    finally:
        stop(proc)

if __name__=="__main__":
    try: main()
    except Exception as exc:
        print(str(exc),file=sys.stderr)
        sys.exit(1)
