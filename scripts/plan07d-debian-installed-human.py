#!/usr/bin/env python3
"""PLAN-07D strict real Debian/GTK/AT-SPI human review acceptance.

Only an actual dpkg-installed Lu process and a real X11/AT-SPI desktop can
produce PASS. All decisions are independently checked in original ANSF bytes.
This does NOT represent physical Windows / Radeon / genuine GGUF acceptance.
"""
import argparse
import hashlib
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time
import gi
gi.require_version("Atspi","2.0")
from gi.repository import Atspi

PHASES = ("refused","cancelled","approved_for_review","interrupted")
TITLE = "Lu — Human evidence follow-up decision"

def check(cond, why):
    if not cond:
        raise RuntimeError("PLAN07D_DEBIAN_HUMAN_REFUSED: "+why)

def command(*args,timeout=16):
    proc=subprocess.run(args,capture_output=True,text=True,timeout=timeout)
    check(proc.returncode==0,"OS command failed "+repr(args)+": "+proc.stderr[:350])
    return proc.stdout.strip()

def descendants(root, cap=2400):
    queue=[root]; out=[]; seen=0
    while queue and seen<cap:
        item=queue.pop(0);seen+=1
        try:
            name=item.get_name() or ""
            role=item.get_role_name() or ""
            pid=item.get_process_id()
            out.append((item, name, role, pid))
            for i in range(min(item.get_child_count(),180)):
                child=item.get_child_at_index(i)
                if child is not None:queue.append(child)
        except Exception:continue
    return out

def tree():
    return descendants(Atspi.get_desktop(0))

def observe(label, name, out, timeout=40):
    until=time.time()+timeout
    while time.time()<until:
        nodes=tree()
        matches=[n for n in nodes if n[1]==name]
        if matches:
            snapshot=[{"name":n,"role":r,"pid":pid} for _,n,r,pid in nodes]
            (out/("atspi-"+label+".json")).write_text(json.dumps(snapshot,indent=2)+"\n",encoding="utf-8")
            return matches[0][0]
        time.sleep(.4)
    snapshot=[{"name":n,"role":r,"pid":pid} for _,n,r,pid in tree()]
    (out/("atspi-missing-"+label+".json")).write_text(json.dumps(snapshot,indent=2)+"\n",encoding="utf-8")
    check(False,"real AT-SPI element never appeared: "+name)

def action(name,out,scope_pid=None):
    el=observe("action-"+str(abs(hash(name))),name,out)
    if scope_pid is not None:
        check(el.get_process_id()==scope_pid,"foreign process accessibility control "+name)
    try:
        act=el.get_action_iface()
        check(act is not None and act.get_n_actions()>0,"no accessible action for "+name)
        check(act.do_action(0),"OS AT-SPI action refused "+name)
    except Exception as exc:check(False,"native AT-SPI action failed "+name+": "+str(exc))

def set_note(text,out,scope_pid):
    check(5<=len(text)<=250,"human note bounds invalid")
    el=observe("note","Reason for human evidence decision",out)
    check(el.get_process_id()==scope_pid,"human note belongs to foreign process")
    try:
        editable=el.get_editable_text_iface()
        check(editable is not None,"human note has no AT-SPI editable text")
        editable.set_text_contents(text)
        read=el.get_text_iface()
        check(read is not None and read.get_text(0,-1)==text,
              "accessible note text was not applied")
    except Exception as exc:check(False,"human note not editable via accessibility: "+str(exc))

def wms():
    raw=subprocess.run(["xdotool","search","--onlyvisible","--name","."],
                       capture_output=True,text=True,timeout=12)
    if raw.returncode!=0:return []
    result=[]
    for line in raw.stdout.splitlines():
        wid=line.strip()
        if not wid.isdecimal():continue
        name=subprocess.run(["xdotool","getwindowname",wid],capture_output=True,text=True).stdout.strip()
        result.append((wid,name))
    return result

def native_dialog(name,out):
    until=time.time()+45
    while time.time()<until:
        matches=[(wid,title) for wid,title in wms() if title==name]
        if matches:
            check(len(matches)==1,"ambiguous native OS dialog "+name)
            wid,_=matches[0]
            accessible=observe("dialog-"+str(abs(hash(name))),name,out,10)
            check(accessible is not None,"dialog absent from AT-SPI")
            return wid
        time.sleep(.4)
    check(False,"no actual X11 native dialog with exact title "+name)

def native_folder_picker(original,out,cancel=False):
    check(original.is_dir(),"native picker source directory missing")
    # Picker may be a GTK dialog or xdg-desktop-portal dialog. It must have a
    # real X11 toplevel and an accessible dialog/file-chooser identity.
    until=time.time()+50; picker=None
    while time.time()<until:
        roots=wms()
        matches=[(wid,title) for wid,title in roots if
                 any(x in title.lower() for x in ("select folder","choose folder","open folder","select a folder","file chooser","select file","choose a folder","open file","select directory"))]
        if len(matches)==1:picker=matches[0];break
        time.sleep(.35)
    check(picker is not None,"real native GTK/portal folder chooser not present")
    wid,title=picker
    (out/("folder-picker-"+str(time.time_ns())+".json")).write_text(
        json.dumps({"windowId":wid,"title":title,
            "accessibility":[{"name":n,"role":role,"pid":pid}
                             for _,n,role,pid in tree()][:500]},indent=2)+"\n")
    command("xdotool","windowactivate","--sync",wid)
    if cancel:
        command("xdotool","key","--clearmodifiers","Escape")
        until=time.time()+16
        while time.time()<until:
            if not any(wid==k for k,_ in wms()):return
            time.sleep(.35)
        check(False,"native folder cancel failed to dismiss chooser")
    command("xdotool","key","--clearmodifiers","ctrl+l")
    command("xdotool","type","--clearmodifiers","--delay","3",str(original))
    command("xdotool","key","Return")
    time.sleep(.6)
    # Only confirm a second time if the original native chooser still owns
    # its X11 window. Otherwise a stray Enter could activate the Lu renderer.
    if any(wid==k for k,_ in wms()):
        command("xdotool","key","Return")
    until=time.time()+22
    while time.time()<until:
        if not any(wid==k for k,_ in wms()):return
        time.sleep(.35)
    check(False,"native folder chooser remained open; original path not admitted")

def readonly(profile,seed,out,name,phases=()):
    target=out/("ansf-"+name+".json")
    call=[sys.executable,"scripts/plan07d-readonly-ansf-witness.py",
          "--profile",str(profile),"--seed",str(seed),"--output",str(target)]
    for phase in phases:call.extend(["--require-phase",phase])
    command(*call,timeout=35)
    return json.loads(target.read_text(encoding="utf-8"))

def launch(exe):
    p=subprocess.Popen([str(exe)],stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    time.sleep(.6)
    check(p.poll() is None,"installed Debian ELF exited before native window")
    return p

def switch_project(out,pid,initial):
    selector=observe("project-selector","Project selector",out)
    check(selector.get_process_id()==pid,"foreign project selector")
    try:
        component=selector.get_component_iface()
        check(component is not None and component.grab_focus(),"cannot keyboard-focus source-project selector")
    except Exception as exc:check(False,"project selector not keyboard accessible: "+str(exc))
    command("xdotool","key","--clearmodifiers","Home","Return")
    time.sleep(.65)
    other=visible_source(out)
    if other==initial:
        command("xdotool","key","--clearmodifiers","Down","Return")
        time.sleep(.65)
        other=visible_source(out)
    check(other in ("north","south") and other!=initial,
          "displayed project never actually switched")
    return other

def visible_source(out):
    choices=set()
    for _,name,role,_ in tree():
        if name.startswith("Review north source requirement"):choices.add("north")
        if name.startswith("Review south source requirement"):choices.add("south")
    check(len(choices)==1,"source-project identity ambiguous in live task list: "+repr(choices))
    return choices.pop()

def os_review(out,pid,original,note,choice):
    set_note(note,out,pid)
    action("Open native OS approve/refuse decision…",out,pid)
    wid=native_dialog(TITLE,out)
    if choice=="crash":
        return wid
    name="Refuse evidence review" if choice=="refused" else "Approve evidence review only"
    # Verify this control really belongs to a visible OS modal.
    match=observe("native-"+choice,name,out)
    check(match.get_action_iface() is not None,"real OS decision has no AT-SPI action")
    check(match.get_action_iface().do_action(0),"native OS decision was not applied")
    if choice=="cancelled":native_folder_picker(original,out,cancel=True)
    if choice=="approved_for_review":native_folder_picker(original,out,cancel=False)
    time.sleep(.8)

def close_app(p,out):
    # Request native GUI close, not a background SIGTERM smoke.
    action("Close project task list",out,p.pid)
    wins=[wid for wid,title in wms() if title=="Lu"]
    check(len(wins)==1,"installed Debian main X11 Lu window missing/ambiguous")
    command("xdotool","windowclose",wins[0])
    try:p.wait(timeout=22)
    except subprocess.TimeoutExpired:check(False,"real installed Lu did not exit gracefully")
    check(p.returncode==0,"installed Lu main window exited nonzero")

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--installed-exe",type=pathlib.Path,required=True)
    ap.add_argument("--profile",type=pathlib.Path,required=True)
    ap.add_argument("--seed",type=pathlib.Path,required=True)
    ap.add_argument("--projects-dir",type=pathlib.Path,required=True)
    ap.add_argument("--evidence-dir",type=pathlib.Path,required=True)
    args=ap.parse_args()
    check(str(args.installed_exe).startswith("/usr/lib/") and args.installed_exe.is_file() and
          os.access(args.installed_exe,os.X_OK),"not an installed executable under /usr/lib")
    check(not args.evidence_dir.exists(),"refuse replacing existing acceptance evidence")
    args.evidence_dir.mkdir(parents=True)
    report={"schema":"lu-plan07d-debian-installed-human-v1",
            "originalFixtureNotGenuineGguf":True,"nativeWindow":"NOT_TESTED",
            "accessibleControls":"NOT_TESTED","realSourcePicker":"NOT_TESTED",
            "refusal":"NOT_TESTED","cancellation":"NOT_TESTED",
            "approvalForReview":"NOT_TESTED","projectSwitch":"NOT_TESTED",
            "gracefulRestart":"NOT_TESTED","interruptionRestart":"NOT_TESTED",
            "ansfUnchanged":"NOT_TESTED","completionAuthority":False,
            "executionAuthority":False,"acceptance":"NOT_ACCEPTED","errors":[]}
    p=None
    try:
        initial=readonly(args.profile,args.seed,args.evidence_dir,"original")
        check(not initial["decisions"],"reused nonempty original review journal")
        p=launch(args.installed_exe)
        observe("main-action","Project tasks",args.evidence_dir)
        action("Project tasks",args.evidence_dir,p.pid)
        observe("independent-review","Human evidence decisions · PLAN-07C",args.evidence_dir)
        report["nativeWindow"]="PASS_REAL_DEBIAN_ELF_AND_NATIVE_TASKS"
        for name in ("Project selector","Reopen original decision and restart history…",
                     "Reason for human evidence decision",
                     "Recheck project source using native folder picker…"):
            el=observe("accessible-"+str(abs(hash(name))),name,args.evidence_dir)
            check(el.get_process_id()==p.pid,"foreign OS accessibility element "+name)
        report["accessibleControls"]="PASS_LIVE_ATSPI_HUMAN_CONTROLS"
        label=visible_source(args.evidence_dir)
        root=args.projects_dir/label
        action("Recheck project source using native folder picker…",args.evidence_dir,p.pid)
        native_folder_picker(root,args.evidence_dir)
        observe("original-task-selection","Original task for human evidence review",args.evidence_dir)
        report["realSourcePicker"]="PASS_ACTUAL_GTK_OR_PORTAL_SELECTED_PROJECT"
        for phase,note in [
            ("refused","Refuse model completion until independent native source evidence is reviewed."),
            ("cancelled","Cancel the native project source picker without giving work approval."),
            ("approved_for_review","Approve only a new evidence follow-up without granting task or execution authority.")]:
            os_review(args.evidence_dir,p.pid,root,note,phase)
            current=readonly(args.profile,args.seed,args.evidence_dir,
                             "after-"+phase, [phase])
            check(sum(x["phase"]==phase for x in current["decisions"])==1,
                  "no unique original terminal "+phase+" review")
            check(initial["original"]==current["original"],
                  "human suggestion review unexpectedly changed PLAN05/06 original data")
            report[{"refused":"refusal","cancelled":"cancellation",
                    "approved_for_review":"approvalForReview"}[phase]]="PASS_GENUINE_OS_DIALOG_AND_ANSF_"+phase.upper()
        before=readonly(args.profile,args.seed,args.evidence_dir,"before-switch",PHASES[:3])
        switch_project(args.evidence_dir,p.pid,label)
        report["projectSwitch"]="PASS_TWO_ORIGINAL_SOURCE_SCOPES"
        close_app(p,args.evidence_dir);p=None
        p=launch(args.installed_exe)
        action("Project tasks",args.evidence_dir,p.pid)
        action("Reopen original decision and restart history…",args.evidence_dir,p.pid)
        after=readonly(args.profile,args.seed,args.evidence_dir,"after-graceful-restart",PHASES[:3])
        check(before["decisions"]==after["decisions"] and before["original"]==after["original"],
              "original ANSF review history changed after graceful installed restart")
        report["gracefulRestart"]="PASS_INSTALLED_DEBIAN_GUI_RESTART"
        newlabel=visible_source(args.evidence_dir);newroot=args.projects_dir/newlabel
        action("Recheck project source using native folder picker…",args.evidence_dir,p.pid)
        native_folder_picker(newroot,args.evidence_dir)
        os_review(args.evidence_dir,p.pid,newroot,
                  "Force a real process interruption at pending native operator approval.",
                  "crash")
        p.kill();p.wait(timeout=12);p=None
        pending=readonly(args.profile,args.seed,args.evidence_dir,"after-native-crash",PHASES[:3])
        check(sum(x["phase"]=="prepared" for x in pending["decisions"])==1,
              "no one-use Prepared event present after actual installed crash")
        p=launch(args.installed_exe)
        action("Project tasks",args.evidence_dir,p.pid)
        action("Reopen original decision and restart history…",args.evidence_dir,p.pid)
        until=time.time()+16
        while time.time()<until:
            # History command asynchronously performs recovery; avoid
            # labelling any prepared event as approved while pending.
            conn=sqlite3.connect((args.profile/"ansf/library.db").as_uri()+"?mode=ro",uri=True)
            count=conn.execute("SELECT COUNT(*) FROM ansf_project_task_update_review_events WHERE CAST(event_json AS TEXT) LIKE ?",(b'%"phase":"interrupted"%',)).fetchone()[0]
            conn.close()
            if count==1:break
            time.sleep(.4)
        finished=readonly(args.profile,args.seed,args.evidence_dir,"after-interrupted-recovery",PHASES)
        check(sum(x["phase"]=="interrupted" for x in finished["decisions"])==1,
              "genuine restart failed to seal Interrupted without approval")
        report["interruptionRestart"]="PASS_PREPARED_INTERRUPTED_NOT_APPROVED"
        close_app(p,args.evidence_dir);p=None
        final=readonly(args.profile,args.seed,args.evidence_dir,"after-final-exit",PHASES)
        check(final["original"]==initial["original"],
              "review accidentally modified original ANSF user/task/plan history")
        report["ansfUnchanged"]="PASS_PRESERVED_ORIGINAL_ANSF_DATA_AND_DECISIONS"
        report["acceptance"]="PASS_HOSTED_DEBIAN_INSTALLED_HUMAN_MATRIX"
    except Exception as e:
        report["errors"].append(str(e))
        raise
    finally:
        if p is not None and p.poll() is None:
            p.kill();p.wait(timeout=10)
        (args.evidence_dir/"plan07d-debian-human.json").write_text(
            json.dumps(report,indent=2)+"\n",encoding="utf-8")

if __name__=="__main__":
    try:main()
    except Exception as exc:
        print(str(exc),file=sys.stderr);sys.exit(2)
