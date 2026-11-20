#!/usr/bin/env python3
"""Installed GUI acceptance of changing file grants, actual cadence, fresh review
and revocation. Writes only disposable input text; observes app journals read-only."""
import argparse, hashlib, importlib.util, json, os, shutil, sqlite3, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('base',Path(__file__).with_name('cp11br2-scheduled-reader-gui.py'));base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
spec=importlib.util.spec_from_file_location('recovery',Path(__file__).with_name('cp11br2-scheduled-reader-recovery-gui.py'));rec=importlib.util.module_from_spec(spec);spec.loader.exec_module(rec)
bm=base.bm;gui=base.gui;require,wait,action,named,nodes=base.require,base.wait,base.action,base.named,base.nodes
PROMPT='Retrieve the approved line exactly.'
rec.base.PROMPT=PROMPT
APPROVAL='Lu — Approve scheduled AI task'; REAPPROVAL='Lu — Review recurring file permission'

def journal(path):
    try:
        with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
            c.row_factory=sqlite3.Row
            row=c.execute('SELECT * FROM lu_ai_schedules WHERE prompt=?',(PROMPT,)).fetchone()
            if not row:return None
            sid=row['id'];grant=c.execute('SELECT * FROM lu_ai_live_grants WHERE schedule_id=?',(sid,)).fetchone()
            return {**dict(row),'grant':dict(grant) if grant else None,'runs':[dict(x) for x in c.execute('SELECT * FROM lu_ai_runs WHERE schedule_id=? ORDER BY sequence',(sid,))],'receipts':[dict(x) for x in c.execute('SELECT * FROM lu_ai_reader_receipts WHERE schedule_id=? ORDER BY sequence',(sid,))]}
    except sqlite3.OperationalError:return None

def review(pid,title,choice,phrases):
    label=wait(lambda:named(pid,title),35,'native review '+title)
    wins=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^'+title+'$'],text=True,timeout=5).splitlines()
    require(len(wins)==1 and int(subprocess.check_output(['xdotool','getwindowpid',wins[0]],text=True,timeout=5))==pid,'wrong review owner')
    dialog=label
    for _ in range(12):
        require(dialog is not None and dialog.get_process_id()==pid,'review ancestry')
        if dialog.get_role_name() in ('alert','dialog','window','frame'):break
        dialog=dialog.get_parent()
    else:raise RuntimeError('missing native dialog')
    parts=[]
    for n in nodes(dialog):
        try:
            parts.append(n.get_name() or '');iface=n.get_text_iface()
            if iface:parts.append(gui.Atspi.Text.get_text(iface,0,-1))
        except Exception:pass
    disclosure=' '.join(' '.join(parts).split())
    for p in phrases:require(p in disclosure,'missing review disclosure '+p)
    action(pid,choice,dialog);wait(lambda:named(pid,title) is None,15,'closed review')

def picker(pid,path):
    # GTK native file chooser; verify focus belongs to actual app before typing.
    def chooser():
        for n in nodes(gui.Atspi.get_desktop(0)):
            try:
                if n.get_process_id()==pid and n.get_role_name() in ('file chooser','dialog') and n.get_state_set().contains(gui.Atspi.StateType.SHOWING):
                    if any(child.get_name() in ('Open','Select','Choose') and child.get_role_name()=='push button' for child in nodes(n)):return n
            except Exception:pass
    wait(chooser,30,'actual native file chooser')
    window=subprocess.check_output(['xdotool','getactivewindow'],text=True,timeout=5).strip();require(int(subprocess.check_output(['xdotool','getwindowpid',window],text=True,timeout=5))==pid,'file chooser owner')
    subprocess.run(['xdotool','key','--clearmodifiers','ctrl+l'],check=True,timeout=5);time.sleep(.3)
    subprocess.run(['xdotool','type','--clearmodifiers','--delay','15','--',str(path)],check=True,timeout=20)
    subprocess.run(['xdotool','key','Return'],check=True,timeout=5)

def verify(path,seq,source):
    def terminal():
        row=journal(path)
        return row if row and any(r['sequence']==seq and r['state']!='RUNNING' for r in row['runs']) else None
    row=wait(terminal,115,'occurrence '+str(seq));r=next(x for x in row['runs'] if x['sequence']==seq);rs=[x for x in row['receipts'] if x['sequence']==seq];expected=source.splitlines()[1]
    require(r['state']=='COMPLETED' and r['output']==expected and len(rs)==1,'wrong completed outcome '+str(r));require(rs[0]['line']==2 and rs[0]['verified_text']==expected and rs[0]['sha256']==hashlib.sha256(source.encode()).hexdigest(),'source hash or line mismatch');require(r['started']>=r['due'],'early execution');return r

def main():
    p=argparse.ArgumentParser(description=__doc__);
    for n in ('exe','model','fresh-data-home','out','source'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);p.add_argument('--refuse-only',action='store_true');a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed executable identity');require(bm.sha(a.model)==a.model_sha256,'model identity')
    home=a.fresh_data_home.resolve();root=home/'org.lu.desktop';folder=root/'lu-models';folder.mkdir(parents=True,exist_ok=False);shutil.copyfile(a.model,folder/a.model.name);require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture model copy');a.out.mkdir(parents=True,exist_ok=False)
    data=root/'lu-background-schedule.sqlite';source=a.source.resolve();require(not source.exists() and source.suffix=='.txt','fresh disposable input');first='Sample inventory\nORCHID-731\nOther code LIMA-208';second='Sample inventory\nLOTUS-482\nOther code LIMA-208';third='Sample inventory\nIRIS-293\nOther code LIMA-208';source.write_text(first);inode=source.stat().st_ino
    env=dict(os.environ,XDG_DATA_HOME=str(home));receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}};app=None
    with (a.out/'native.log').open('wb') as log:
        def start():return subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        def stop():
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
        def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=10)
        try:
            app=start();action(app.pid,'New Chat',timeout=90);worker=bm.load(app.pid);base.navigate(app.pid);base.field(app.pid,'Scheduled task prompt',PROMPT)
            action(app.pid,'Read a line from a changing text file');base.field(app.pid,'Changing file line','2');base.field(app.pid,'Task delay in minutes','1');sel=wait(lambda:named(app.pid,'Task recurrence',actionable=True),15,'repeat');require(sel.get_component_iface().grab_focus(),'recurrence focus');subprocess.run(['xdotool','key','--clearmodifiers','Home','Down','Return'],check=True,timeout=10)
            action(app.pid,'Review task…');picker(app.pid,source);review(app.pid,APPROVAL,'Decline' if a.refuse_only else 'Schedule task',[str(source),'Line: 2','Repeat: Every 1 minute(s).','In-place changes are allowed','File access ends when Lu closes','Revoke stops active and future runs'])
            if a.refuse_only:
                require(journal(data) is None,'refusal persisted a schedule');receipt['phases']['refusal']='PASS';receipt['status']='PASS_REFUSAL';return
            row=wait(lambda:journal(data),20,'persisted schedule');require(row['grant']['display']==str(source) and row['grant']['line']==2 and row['grant']['permission']=='APPROVED' and row['repeat_seconds']==60 and not row['runs'],'wrong grant scope');receipt['phases']['approval']='PASS'
            before=bm.ticks(worker);r1=verify(data,1,first);require(bm.ticks(worker)>before and bm.model_worker(app.pid)==worker,'genuine inference not observed');receipt['phases']['first_verified_read']='PASS'
            source.write_text(second);require(source.stat().st_ino==inode,'input replaced by fixture');r2=verify(data,2,second);require(r2['due']-r1['due']==60,'cadence');receipt['phases']['changed_read_on_original_cadence']='PASS';prefix=journal(data);capture('two-reports')
            stop();app=start();base.navigate(app.pid);row=wait(lambda:journal(data),20,'reopened schedule');require(row['state']=='PAUSED' and row['grant']['permission']=='REVIEW' and row['runs']==prefix['runs'] and row['receipts']==prefix['receipts'] and not bm.model_worker(app.pid),'restart restored authority or changed results');receipt['phases']['restart_requires_review']='PASS';capture('review-required')
            rec.card_action(app.pid,'Review file permission…');picker(app.pid,source);review(app.pid,REAPPROVAL,'Decline',[str(source),'Line: 2','task stays paused']);require(journal(data)['grant']['permission']=='REVIEW','decline changed permission');receipt['phases']['reapproval_refusal']='PASS'
            rec.card_action(app.pid,'Review file permission…');picker(app.pid,source);review(app.pid,REAPPROVAL,'Approve permission',[str(source),'Line: 2','task stays paused']);row=wait(lambda:v if (v:=journal(data)) and v['grant']['permission']=='APPROVED' else None,20,'new approval');require(row['state']=='PAUSED' and row['runs']==prefix['runs'],'approval resumed without explicit press');receipt['phases']['fresh_reapproval']='PASS'
            source.write_text(third);require(source.stat().st_ino==inode,'third fixture replaced file');action(app.pid,'Chat');worker=bm.load(app.pid);base.navigate(app.pid);rec.card_action(app.pid,'Resume task');r3=verify(data,3,third);require(r3['due']>r2['due'],'replayed due');receipt['phases']['resumed_verified_read']='PASS'
            rec.card_action(app.pid,'Revoke file permission…');rec.card_action(app.pid,'Confirm permission revocation');row=wait(lambda:v if (v:=journal(data)) and v['grant']['permission']=='REVOKED' else None,20,'permission revoked');require(row['state']=='CANCELLED' and len(row['runs'])==3 and row['runs'][:2]==prefix['runs'] and row['receipts'][:2]==prefix['receipts'],'revocation altered reports');receipt['phases']['revocation_preserves_reports']='PASS';capture('revoked');stop();app=start();base.navigate(app.pid);require(journal(data)==row and not bm.model_worker(app.pid),'revoked permission restored');receipt['phases']['revoked_cold_restart']='PASS';receipt['status']='PASS'
        except BaseException as e:receipt['error']=str(e);raise
        finally:
            receipt['journal']=journal(data);stop();(a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
