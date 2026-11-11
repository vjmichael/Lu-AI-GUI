#!/usr/bin/env python3
"""PLAN-07D real installed Debian GTK/AT-SPI smoke, not full human approval.

Runs the installed ELF under an actual X11/DBus desktop, observes the OS window
and accessibility tree, opens the independent task list and restarts Lu. A
scripted control or source-only check cannot substitute for actual UI evidence.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys
import time
import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

def refuse(message):
    raise RuntimeError("PLAN07D_DEBIAN_INSTALLED_REFUSED: " + message)

def nodes(root, max_nodes=1800):
    pending=[root]
    out=[]
    while pending and len(out)<max_nodes:
        el=pending.pop(0)
        try:
            name=el.get_name() or ""
            role=el.get_role_name() or ""
            pid=el.get_process_id()
            out.append({"name":name,"role":role,"pid":pid})
            n=min(el.get_child_count(),200)
            for i in range(n):
                kid=el.get_child_at_index(i)
                if kid: pending.append(kid)
        except Exception:
            continue
    return out

def wait_tree(pid, want, timeout=55):
    deadline=time.time()+timeout
    last=[]
    while time.time()<deadline:
        last=nodes(Atspi.get_desktop(0))
        found=[x for x in last if want in x["name"]]
        if found:
            return last
        time.sleep(.55)
    refuse("AT-SPI never exposed actual visible "+want+"; last="+str(last[:40]))

def find_accessible(root, name):
    pending=[root]
    seen=0
    while pending and seen<1800:
        x=pending.pop(0);seen+=1
        try:
            if x.get_name()==name:return x
            for i in range(min(x.get_child_count(),200)):
                c=x.get_child_at_index(i)
                if c:pending.append(c)
        except Exception:
            continue
    return None

def click_action(name):
    root=Atspi.get_desktop(0)
    button=find_accessible(root,name)
    if not button:refuse("missing AT-SPI actionable control: "+name)
    try:
        act=button.get_action_iface()
        if not act or act.get_n_actions()<1 or not act.do_action(0):
            refuse("control lacks a real accessible action: "+name)
    except Exception as e:
        refuse("cannot invoke real installed action "+name+": "+str(e))

def snap(profile,seed,dest):
    subprocess.run([sys.executable,"scripts/plan07d-readonly-ansf-witness.py",
                    "--profile",str(profile),"--seed",str(seed),"--output",str(dest)],
                   check=True)
    return json.loads(dest.read_text(encoding="utf-8"))

def launch(exe):
    proc=subprocess.Popen([str(exe)],stdin=subprocess.DEVNULL,
                          stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    if proc.poll() is not None:refuse("installed ELF exited before Linux UI")
    return proc

def stop(proc):
    proc.terminate()
    try:proc.wait(timeout=20)
    except subprocess.TimeoutExpired:
        proc.kill();proc.wait(timeout=5)
        refuse("installed Lu ignored desktop close/termination")
    if proc.returncode not in (0,-15):
        refuse("installed Lu abnormal exit="+str(proc.returncode))

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--installed-exe",type=pathlib.Path,required=True)
    p.add_argument("--profile",type=pathlib.Path,required=True)
    p.add_argument("--seed",type=pathlib.Path,required=True)
    p.add_argument("--evidence-dir",type=pathlib.Path,required=True)
    args=p.parse_args()
    if not args.installed_exe.is_file() or not os.access(args.installed_exe,os.X_OK):
        refuse("not a real installed executable")
    if not str(args.installed_exe).startswith("/usr/"):
        refuse("refuse unpacked build-tree executable")
    if args.evidence_dir.exists():refuse("refuse overwriting Linux acceptance evidence")
    args.evidence_dir.mkdir(parents=True)
    result={"schema":"lu-plan07d-debian-atspi-smoke-v1",
            "realInstalledElf":str(args.installed_exe),
            "originalFixtureIsSyntheticNotGguf":True,
            "actualAppWindow":"NOT_TESTED",
            "accessibleTaskList":"NOT_TESTED",
            "restart":"NOT_TESTED",
            "ansfPreserved":"NOT_TESTED",
            "humanApproveRefuse":"NOT_TESTED",
            "acceptance":"NOT_ACCEPTED", "errors":[]}
    proc=None
    try:
        original=snap(args.profile,args.seed,args.evidence_dir/"original.json")
        proc=launch(args.installed_exe)
        tree=wait_tree(proc.pid,"Project tasks")
        (args.evidence_dir/"atspi-main.json").write_text(json.dumps(tree,indent=2))
        result["actualAppWindow"]="PASS_REAL_GTK_ATSPI_MAIN"
        click_action("Project tasks")
        tree=wait_tree(proc.pid,"Human evidence decisions · PLAN-07C")
        (args.evidence_dir/"atspi-task-list.json").write_text(json.dumps(tree,indent=2))
        if not any(x["name"]=="Project selector" for x in tree):
            refuse("native task-list selector not accessible")
        if not any(x["name"]=="Reason for human evidence decision" for x in tree):
            refuse("native human note control not accessible")
        result["accessibleTaskList"]="PASS_ATSPI_TASK_LIST_AND_OPERATOR_LABELS"
        stop(proc);proc=None
        after_close=snap(args.profile,args.seed,args.evidence_dir/"after-close.json")
        proc=launch(args.installed_exe)
        wait_tree(proc.pid,"Project tasks")
        click_action("Project tasks")
        wait_tree(proc.pid,"Human evidence decisions · PLAN-07C")
        after_restart=snap(args.profile,args.seed,args.evidence_dir/"after-restart.json")
        if (original["original"]!=after_restart["original"] or
                original["decisions"]!=after_restart["decisions"] or
                after_close["original"]!=after_restart["original"]):
            refuse("original ANSF changed across genuine installed Debian restart")
        result["restart"]="PASS_INSTALLED_ELF_EXIT_RESTART"
        result["ansfPreserved"]="PASS_INDEPENDENT_READONLY_SQLITE"
        # This is deliberately only a hosted Linux smoke. No human OS
        # approve/refuse, native folder chooser, or project switching here.
        result["acceptance"]="PASS_HOSTED_DEBIAN_INSTALLED_UI_SMOKE_ONLY"
    except Exception as e:
        result["errors"].append(str(e))
        raise
    finally:
        if proc is not None and proc.poll() is None:
            proc.terminate()
            try:proc.wait(timeout=12)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
        (args.evidence_dir/"plan07d-debian-smoke.json").write_text(json.dumps(result,indent=2)+"\n")
if __name__=="__main__":
    try:main()
    except Exception as exc:
        print(str(exc),file=sys.stderr)
        sys.exit(2)
