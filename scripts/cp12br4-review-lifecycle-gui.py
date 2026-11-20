#!/usr/bin/env python3
"""Actual installed Linux CP-12B-R4 lifecycle acceptance.
Separate ordinary Lu processes/profiles, native consent, real model activity,
read-only trace evidence. No private app IPC or artificial inference delays.
Invitations stay in the desktop clipboard or ephemeral process memory only.
"""
import argparse, importlib.util, json, os, shutil, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('r3',Path(__file__).with_name('cp12br3-two-model-review-gui.py'))
r3=importlib.util.module_from_spec(spec);spec.loader.exec_module(r3)
bm,gui,dx=r3.bm,r3.gui,r3.dx
require,wait,action,named=r3.require,r3.wait,r3.action,r3.named
CREATE,ACCEPT,SHARE,RECEIVE=r3.CREATE,r3.ACCEPT,r3.SHARE,r3.RECEIVE

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','model','out'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True)
    a=p.parse_args();a.out=a.out.resolve()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed identity')
    require(bm.sha(a.model)==a.model_sha256,'model identity')
    a.out.mkdir(parents=True,exist_ok=False)
    receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}}
    processes=[];logs=[];holders=[]
    def record(): (a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    def mark(k,v='PASS'):receipt['phases'][k]=v;record();print(k,v,flush=True)
    def focus(pid):
        wins=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(pid)],text=True,timeout=5).splitlines();require(wins,'no app window')
        subprocess.run(['xdotool','windowactivate','--sync',wins[-1]],check=True,timeout=5)
    def settings(pid):
        focus(pid)
        ready=lambda:named(pid,CREATE,actionable=True) or named(pid,'Stop sharing',actionable=True)
        if not ready():action(pid,'Settings')
        if not ready():
            # The previous Settings subsection can remain selected. The
            # title and button may both expose the same accessible name.
            time.sleep(.4)
            if not ready():action(pid,'Privacy & Observe')
        wait(ready,25,'review panel')
    def sockets():return {str(p) for p in Path('/tmp').glob('lu-dx-*') if p.is_dir() and p.stat().st_uid==os.geteuid() and (p/'s').is_socket()}
    def start(name):
        root=a.out/name;data=root/'data'/'org.lu.desktop';models=data/'lu-models';models.mkdir(parents=True,exist_ok=True)
        model=models/a.model.name
        if not model.exists():shutil.copyfile(a.model,model)
        require(bm.sha(model)==a.model_sha256,'model copy identity')
        log=(a.out/(name+'.log')).open('ab');logs.append(log)
        app=subprocess.Popen([str(a.exe)],env=dict(os.environ,XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache')),stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        processes.append(app);action(app.pid,'New Chat',timeout=90);return app,data/'lu-dx-traces.sqlite'
    def capture(label):subprocess.run(['scrot',str(a.out/(label+'.png'))],check=True,timeout=8)
    def share(pid):
        focus(pid);action(pid,CREATE);dx.base.review(pid,SHARE,'Review and share',['currently loaded local model','Only these counts and a checked finding category','No conversation content, files, model identity or tool access'])
    def receive(pid):
        focus(pid);node=wait(lambda:named(pid,'Invitation from another Lu',actionable=True),20,'invitation entry');require(node.get_component_iface().grab_focus(),'invitation focus');wait(lambda:node.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'invitation focus')
        subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','ctrl+v'],check=True,timeout=8);action(pid,ACCEPT,timeout=20)
        dx.base.review(pid,RECEIVE,'Receive and review',['five counts independently',"will not see the other model's answer",'No conversations, files, tools or repairs','consumed on receipt'])
    def active(pid,worker,before):
        # Require simultaneous real CPU advancement and actionable stop.
        def witness():
            n=named(pid,'Stop review',actionable=True);ticks=bm.ticks(worker)
            return n if n and ticks is not None and ticks>before else None
        return wait(witness,20,'active review and actual model CPU activity')
    def stopped(pid):
        wait(lambda:named(pid,'Stop review') is None and named(pid,CREATE,actionable=True),35,'review settled')
        require(named(pid,'Clear review',actionable=True) is None and named(pid,'Copy invitation',actionable=True) is None,'cancelled result published')
    def no_result(pid,label):
        stopped(pid);capture(label);mark(label,'PASS_NO_RESULT_VISUAL_STATUS_REVIEW_REQUIRED')
    def explicit_model_recovery(pid,worker,label):
        # The inference supervisor may reap its worker to enforce cancellation.
        # Never silently re-run or load: witness the idle state, then use the
        # ordinary model picker and explicit user load if it was ejected.
        previous=bm.model_worker(pid)
        if previous is None:
            require(bm.ticks(worker) is None,'cancelled owned worker still present')
            action(pid,'Chat');worker=bm.load(pid);settings(pid)
            mark(label,'PASS_EXPLICIT_RELOAD_AFTER_WORKER_REAP')
        else:
            require(previous==worker,'model replaced without explicit load');mark(label,'PASS_SAME_WORKER_READY')
        return worker
    def make_invitation(pid,worker,base):
        for _ in range(3):
            before=bm.ticks(worker);share(pid)
            wait(lambda:named(pid,'Copy invitation',actionable=True) or (named(pid,CREATE,actionable=True) and named(pid,'Stop review') is None),50,'review settled')
            if not named(pid,'Copy invitation',actionable=True):continue
            created=sockets()-base;require(len(created)==1 and bm.ticks(worker)>before,'verified native review invitation')
            ep=Path(next(iter(created)));action(pid,'Copy invitation');return ep
        raise RuntimeError('model did not produce a verified invitation in three genuine attempts')
    def crash(app,worker):
        app.kill();app.wait(10)
        wait(lambda:bm.ticks(worker) is None,25,'owned model worker exited after application crash')
    def hold_clipboard():
        # Copy only via normal UI clipboard. Keep a volatile clipboard owner
        # across sender exit; never store or print the one-use code.
        import tkinter
        tk=tkinter.Tk();tk.withdraw();code=tk.clipboard_get();tk.destroy();require(len(code)==101 and code.startswith('lu2-'),'copied review code')
        holder=subprocess.Popen(['/usr/bin/python3','-c','import sys,tkinter as t; c=t.Tk();c.withdraw();c.clipboard_clear();c.clipboard_append(sys.stdin.read());c.update();c.mainloop()'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        holder.stdin.write(code.encode());holder.stdin.close();holders.append(holder);time.sleep(.3)
    def fresh_review(pid,worker,ep):
        before=bm.ticks(worker);receive(pid);wait(lambda:not ep.exists(),20,'one-use admission')
        wait(lambda:named(pid,'Clear review',actionable=True),50,'verified two-model result');require(bm.ticks(worker)>before,'receiver real model execution');capture('fresh-two-model');action(pid,'Clear review')
    try:
        baseline=sockets();sender,spath=start('sender');focus(sender.pid);bm.send_chat(sender.pid,'Show local model status');wait(lambda:next((t for t in dx.traces(spath) if t['state']=='sealed' and [13,2,0] in t['events']),None),40,'source delivery');source=dx.traces(spath)
        require(len(source)==1 and not source[0]['dropped'],'source fixture count');sw=bm.load(sender.pid);settings(sender.pid)
        require(sockets()==baseline,'unexpected initial invitation');mark('initial_default_off')
        st=bm.ticks(sw);share(sender.pid);n=active(sender.pid,sw,st);require(n.get_action_iface().do_action(0),'native stop click');stopped(sender.pid);require(sockets()==baseline and dx.traces(spath)==source,'sender cancellation changed evidence or created invitation');capture('sender-cancel');mark('sender_active_cancel');sw=explicit_model_recovery(sender.pid,sw,'sender_model_recovery')
        # Fresh review after cancel proves the model is usable without auto-retry.
        ep=make_invitation(sender.pid,sw,baseline);receiver,rpath=start('receiver');rw=bm.load(receiver.pid);settings(receiver.pid)
        prior=dx.traces(rpath);rt=bm.ticks(rw);receive(receiver.pid);n=active(receiver.pid,rw,rt);require(n.get_action_iface().do_action(0),'native receiver stop click');stopped(receiver.pid);require(not ep.exists() and dx.traces(rpath)==prior,'receiver cancellation changed source or resurrected invitation');capture('receiver-cancel');mark('receiver_active_cancel');rw=explicit_model_recovery(receiver.pid,rw,'receiver_model_recovery')
        rt=bm.ticks(rw);receive(receiver.pid);no_result(receiver.pid,'cancelled_invitation_replay');require(bm.ticks(rw)==rt,'replayed proof ran model')
        ep=make_invitation(sender.pid,sw,baseline);focus(sender.pid);action(sender.pid,'Stop sharing');wait(lambda:not ep.exists(),15,'revoked endpoint');rt=bm.ticks(rw);receive(receiver.pid);no_result(receiver.pid,'revoked_invitation');require(bm.ticks(rw)==rt,'revoked proof ran model');mark('revocation_no_new_inference')
        st=bm.ticks(sw);share(sender.pid);active(sender.pid,sw,st);crash(sender,sw);require(sockets()==baseline,'active sender crash published an invitation');sender,spath=start('sender');settings(sender.pid);require(bm.model_worker(sender.pid) is None and sockets()==baseline and dx.traces(spath)==source,'sender restart changed evidence or auto-loaded model');mark('sender_active_crash_restart')
        action(sender.pid,'Chat');sw=bm.load(sender.pid);settings(sender.pid);ep=make_invitation(sender.pid,sw,baseline);hold_clipboard();crash(sender,sw);sender,spath=start('sender');settings(sender.pid);require(bm.model_worker(sender.pid) is None and dx.traces(spath)==source,'sender retry persisted');rt=bm.ticks(rw);receive(receiver.pid);no_result(receiver.pid,'sender_crash_stale_invitation');require(bm.ticks(rw)==rt,'stale proof ran model');require(ep.is_dir(),'unexpected crash endpoint custody');(ep/'s').unlink();ep.rmdir();mark('sender_published_crash_restart')
        action(sender.pid,'Chat');sw=bm.load(sender.pid);settings(sender.pid);ep=make_invitation(sender.pid,sw,baseline);rt=bm.ticks(rw);receive(receiver.pid);active(receiver.pid,rw,rt);crash(receiver,rw);require(not ep.exists(),'crashed receiver revived invitation');receiver,rpath=start('receiver');settings(receiver.pid);require(bm.model_worker(receiver.pid) is None and dx.traces(rpath)==prior,'receiver restart changed evidence or auto-loaded model');mark('receiver_active_crash_restart')
        action(receiver.pid,'Chat');rw=bm.load(receiver.pid);settings(receiver.pid);rt=bm.ticks(rw);receive(receiver.pid);no_result(receiver.pid,'receiver_crash_replay');require(bm.ticks(rw)==rt,'receiver crash replay ran model')
        ep=make_invitation(sender.pid,sw,baseline);fresh_review(receiver.pid,rw,ep);require(dx.traces(spath)==source and dx.traces(rpath)==prior,'source evidence modified');mark('fresh_approved_review_after_restarts','PASS_VISUAL_STATUS_REVIEW_REQUIRED')
        receipt['status']='PASS_SCOPED_VISUAL_REVIEW_REQUIRED'
    except BaseException as e:
        receipt['error']=str(e);print('INCOMPLETE',str(e),flush=True)
        try:capture('failure')
        except Exception:pass
        raise
    finally:
        for app in processes:
            if app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
        for proc in holders:
            if proc.poll() is None:proc.terminate();proc.wait(5)
        for log in logs:log.close()
        record()
if __name__=='__main__':main()
