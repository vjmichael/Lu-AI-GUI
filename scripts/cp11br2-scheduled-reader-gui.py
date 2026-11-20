#!/usr/bin/env python3
"""Installed recurring Reader acceptance: native actions, real clock, pinned GGUF,
read-only independent journal verification. Disposable profile only."""
import argparse, hashlib, importlib.util, json, os, shutil, sqlite3, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('bm',Path(__file__).with_name('cp11a-model-benchmark-gui.py'));bm=importlib.util.module_from_spec(spec);spec.loader.exec_module(bm)
gui=bm.gui; require,wait,action,named,nodes=gui.require,gui.wait,gui.action,gui.named,gui.nodes
TITLE='Lu — Approve scheduled AI task'
NOTE='Sample inventory\nORCHID-731\nOther code LIMA-208'
EXPECTED=NOTE.splitlines()[1]
PROMPT='Retrieve the approved line exactly.'
REFUSED='Refused note retrieval.'
def journal(path):
    try:
        with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
            c.row_factory=sqlite3.Row
            return {r['prompt']:{**dict(r),'runs':[dict(x) for x in c.execute('SELECT * FROM lu_ai_runs WHERE schedule_id=? ORDER BY sequence',(r['id'],))],'receipts':[dict(x) for x in c.execute('SELECT * FROM lu_ai_reader_receipts WHERE schedule_id=? ORDER BY sequence',(r['id'],))],'grant':dict(g) if (g:=c.execute('SELECT * FROM lu_ai_reader_grants WHERE schedule_id=?',(r['id'],)).fetchone()) else None} for r in c.execute('SELECT * FROM lu_ai_schedules')}
    except sqlite3.OperationalError:return {}
def navigate(pid):
    if named(pid,'Scheduled task prompt',actionable=True):return
    if not named(pid,'Background work',actionable=True):action(pid,'Tasks',timeout=90)
    n=wait(lambda:next((n for n in nodes(gui.Atspi.get_desktop(0)) if n.get_name()=='Scheduled AI tasks' and gui.owned(n.get_process_id(),pid) and n.get_role_name() in ('push button','toggle button') and n.get_state_set().contains(gui.Atspi.StateType.SHOWING)),None),30,'Scheduled tasks tab')
    require(n.get_action_iface().do_action(0),'tab selection failed');wait(lambda:named(pid,'Scheduled task prompt',actionable=True),30,'scheduled form')
def field(pid,name,text):
    node=wait(lambda:named(pid,name,actionable=True),30,name)
    require(node.get_component_iface().grab_focus(),'focus '+name)
    wait(lambda:node.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'focus '+name)
    iface=node.get_editable_text_iface()
    if iface:require(gui.Atspi.EditableText.set_text_contents(iface,text),'editable '+name)
    else:
        subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','BackSpace'],check=True,timeout=5)
        subprocess.run(['xdotool','type','--clearmodifiers','--',text],check=True,timeout=15)
    wait(lambda:gui.Atspi.Text.get_text(node.get_text_iface(),0,-1)==text,5,'exact '+name)
def review(pid,prompt,choice):
    label=wait(lambda:named(pid,TITLE),25,'native task review')
    wins=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^'+TITLE+'$'],text=True,timeout=5).splitlines()
    require(len(wins)==1 and int(subprocess.check_output(['xdotool','getwindowpid',wins[0]],text=True,timeout=5))==pid,'wrong review owner')
    dialog=label
    for _ in range(12):
        require(dialog is not None and dialog.get_process_id()==pid,'dialog identity')
        if dialog.get_role_name() in ('alert','dialog','frame','window'):break
        dialog=dialog.get_parent()
    else:raise RuntimeError('no dialog ancestor')
    parts=[]
    for n in nodes(dialog):
        try:
            parts.append(n.get_name() or '');iface=n.get_text_iface()
            if iface:parts.append(gui.Atspi.Text.get_text(iface,0,-1))
        except Exception:pass
    disclosure=' '.join(' '.join(parts).split())
    for phrase in (prompt,'Repeat: Every 1 minute(s).','pasted note only; line 2','request this one line once per occurrence','No other tools, files, chat history or internet','final answer must match the selected line',' '.join(NOTE.splitlines()),'32 attempts','interrupted attempts are never replayed'):
        require(phrase in disclosure,'missing disclosure: '+phrase)
    action(pid,choice,dialog);wait(lambda:named(pid,TITLE) is None,15,'review closed')
def create(pid,path,prompt,approve):
    navigate(pid);field(pid,'Scheduled task prompt',prompt)
    if not named(pid,'Approved note',actionable=True):action(pid,'Read a line from an approved note')
    field(pid,'Approved note',NOTE);field(pid,'Approved note line','2');field(pid,'Task delay in minutes','1')
    sel=wait(lambda:named(pid,'Task recurrence',actionable=True),15,'recurrence field');require(sel.get_component_iface().grab_focus(),'recurrence focus')
    subprocess.run(['xdotool','key','--clearmodifiers','Home','Down','Return'],check=True,timeout=10)
    action(pid,'Review task…');review(pid,prompt,'Schedule task' if approve else 'Decline')
    if approve:return wait(lambda:journal(path).get(prompt),20,'durable schedule')
    require(prompt not in journal(path),'decline persisted task')
def run_row(path,seq):
    return next((r for r in journal(path).get(PROMPT,{}).get('runs',[]) if r['sequence']==seq),None)
def terminal(path,seq,timeout=115):
    return wait(lambda:r if (r:=run_row(path,seq)) and r['state']!='RUNNING' else None,timeout,'terminal attempt '+str(seq))
def verify(path,seq):
    r=terminal(path,seq);row=journal(path)[PROMPT]
    rs=[t for t in row['receipts'] if t['sequence']==seq]
    require(r['state']=='COMPLETED',f'attempt {seq} terminal {r["state"]}; receipt count {len(rs)}')
    require(r['output']==EXPECTED and len(rs)==1,'wrong output or receipt count')
    require(rs[0]['line']==2 and rs[0]['verified_text']==EXPECTED and rs[0]['sha256']==hashlib.sha256(NOTE.encode()).hexdigest(),'independent tool witness mismatch')
    require(r['started']>=r['due'],'early execution');return r
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','model','fresh-data-home','out'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity');require(bm.sha(a.model)==a.model_sha256,'model identity')
    home=a.fresh_data_home.resolve();root=home/'org.lu.desktop';folder=root/'lu-models';folder.mkdir(parents=True,exist_ok=False);shutil.copyfile(a.model,folder/a.model.name);require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture copy')
    path=root/'lu-background-schedule.sqlite';a.out.mkdir(parents=True,exist_ok=False);env=dict(os.environ,XDG_DATA_HOME=str(home));receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}};app=None
    with (a.out/'native.log').open('wb') as log:
        try:
            app=subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log);action(app.pid,'New Chat',timeout=90);worker=bm.load(app.pid);navigate(app.pid)
            create(app.pid,path,REFUSED,False);receipt['phases']['refusal']='PASS'
            row=create(app.pid,path,PROMPT,True);require(row['grant']['note']==NOTE and row['grant']['line']==2 and row['repeat_seconds']==60 and not row['runs'],'incorrect persisted scope');receipt['phases']['approval']='PASS'
            # Actual elapsed scheduled cadence; witness worker CPU advance before terminal.
            before=bm.ticks(worker);first=verify(path,1);require(bm.ticks(worker)>before and bm.model_worker(app.pid)==worker,'no genuine inference witness');receipt['phases']['first_verified_result']='PASS'
            second=verify(path,2);require(second['due']-first['due']==60,'wrong cadence');receipt['phases']['second_verified_result']='PASS'
            subprocess.run(['scrot',str(a.out/'results.png')],check=True,timeout=10)
            receipt['status']='PASS_RECURRING_READER'
        except BaseException as e:receipt['error']=str(e);raise
        finally:
            receipt['journal']=journal(path)
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
            (a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
