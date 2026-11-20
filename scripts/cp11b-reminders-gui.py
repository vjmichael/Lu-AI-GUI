#!/usr/bin/env python3
"""Installed native reminder acceptance. Real GUI actions and natural elapsed time.
Uses a new disposable local profile and only read-only journal inspection. No
model is loaded, no IPC is invoked, no clock or stored schedule is modified.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time

spec=importlib.util.spec_from_file_location('gui_helpers',Path(__file__).with_name('cp06er3-native-web-gui.py'))
gui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)
require,wait,action,named,nodes=gui.require,gui.wait,gui.action,gui.named,gui.nodes
TITLE='Lu — Approve reminder'
RECURRING='CP11B repeat sample'
ONCE='CP11B one time sample'
CANCEL='CP11B cancelled sample'
REFUSED='CP11B refused sample'


def journal(path):
    if not path.is_file():return {}
    try:
        with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True,timeout=.1) as con:
            con.row_factory=sqlite3.Row
            return {r['text']:dict(r) for r in con.execute('SELECT * FROM lu_user_reminders')}
    except sqlite3.OperationalError:return {}


def field(pid,name,text):
    node=wait(lambda:named(pid,name,actionable=True),30,name)
    iface=node.get_component_iface()
    require(iface and iface.grab_focus(),'focus refused: '+name)
    wait(lambda:node.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'focus '+name)
    subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','BackSpace'],check=True,timeout=5)
    subprocess.run(['xdotool','type','--clearmodifiers','--',text],check=True,timeout=10)
    return node


def send(pid,prompt):
    node=field(pid,'Message Lu',prompt)
    wait(lambda:gui.Atspi.Text.get_text(node.get_text_iface(),0,-1)==prompt,5,'exact original chat input')
    action(pid,'Send')


def review(pid,text,minutes,repeat,choice):
    label=wait(lambda:named(pid,TITLE),30,'native reminder confirmation')
    wins=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^Lu — Approve reminder$'],text=True,timeout=5).splitlines()
    require(len(wins)==1 and int(subprocess.check_output(['xdotool','getwindowpid',wins[0]],text=True,timeout=5))==pid,'wrong native dialog identity')
    dialog=label
    for _ in range(12):
        require(dialog is not None and dialog.get_process_id()==pid,'wrong native dialog owner')
        if dialog.get_role_name() in ('alert','dialog','frame','window'):break
        dialog=dialog.get_parent()
    else:raise RuntimeError('native dialog ancestor unavailable')
    parts=[]
    for node in nodes(dialog):
        try:
            parts.append(node.get_name() or '')
            iface=node.get_text_iface()
            if iface:parts.append(gui.Atspi.Text.get_text(iface,0,-1))
        except Exception:pass
    disclosure=' '.join(' '.join(parts).split())
    for phrase in (text,f'First reminder: {minutes} minute(s) after approval.',f'Repeat: {repeat}.','Missed reminders appear when Lu is reopened','never runs AI, tools, or network requests','Conversation:'):
        require(phrase in disclosure,'missing exact disclosure: '+phrase)
    action(pid,choice,dialog)
    wait(lambda:named(pid,TITLE) is None,10,'native review closed')


def navigate(pid):
    action(pid,'Tasks',timeout=90)
    node=wait(lambda:next((n for n in nodes(gui.Atspi.get_desktop(0)) if n.get_name() in ('Reminders','Reminders (1)','Reminders (2)','Reminders (3)') and gui.owned(n.get_process_id(),pid) and n.get_state_set().contains(gui.Atspi.StateType.SHOWING) and n.get_action_iface() and n.get_action_iface().get_n_actions()>0),None),30,'Reminders tab')
    require(node.get_action_iface().do_action(0),'Reminders tab click failed')
    wait(lambda:named(pid,'Reminder text',actionable=True),20,'Reminders form')


def card(pid,text):
    node=wait(lambda:named(pid,'Reminder: '+text),25,'reminder card '+text)
    return node


def card_action(pid,text,label):
    action(pid,label,card(pid,text),timeout=25)


def new_form(pid,path,text,delay):
    field(pid,'Reminder text',text)
    field(pid,'Reminder delay in minutes',str(delay))
    action(pid,'Review reminder…')
    review(pid,text,delay,'Once','Schedule reminder')
    return wait(lambda:journal(path).get(text),20,'saved '+text)


def no_model(pid):
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal():continue
        info=gui.proc_identity(int(entry.name))
        require(not(info and info[2].name=='llama-server' and gui.owned(int(entry.name),pid)),'reminder started inference')


def capture(out,name):
    subprocess.run(['scrot',str(out/(name+'.png'))],check=True,timeout=10)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('exe','fresh-data-home','out'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--sha256',required=True)
    a=p.parse_args()
    require(os.geteuid()!=0,'non-root desktop required')
    exe=a.exe.resolve(strict=True)
    require(exe.stat().st_uid==0 and not exe.stat().st_mode&0o022,'protected installed binary required')
    actual=hashlib.sha256(exe.read_bytes()).hexdigest();require(actual==a.sha256,'wrong binary')
    home=a.fresh_data_home.resolve();home.mkdir(parents=True,exist_ok=False)
    root=home/'org.lu.desktop';path=root/'lu-background-schedule.sqlite'
    a.out.mkdir(parents=True,exist_ok=False)
    env=dict(os.environ,XDG_DATA_HOME=str(home));app=None
    receipt={'status':'INCOMPLETE','binary_sha256':actual,'phases':{},'scope':'Installed Debian native GUI, actual elapsed clock, no model; no Windows or arbitrary scheduled AI work acceptance'}
    with (a.out/'native.log').open('wb') as log:
        def launch():return subprocess.Popen([str(exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        try:
            app=launch();action(app.pid,'New Chat',timeout=90)
            require(not journal(path),'startup created reminders')
            send(app.pid,'Remind me in 1 minute to '+REFUSED)
            review(app.pid,REFUSED,1,'Once','Decline')
            wait(lambda:named(app.pid,'Message Lu',actionable=True),20,'composer restored')
            require(not journal(path),'refusal created reminder')
            receipt['phases']['chat_refusal']='PASS'
            send(app.pid,'Remind me every minute to '+RECURRING)
            review(app.pid,RECURRING,1,'Every 1 minute(s)','Schedule reminder')
            r=wait(lambda:journal(path).get(RECURRING),20,'chat-approved recurrence')
            require(r['repeat_seconds']==60 and r['fired_count']==0 and r['state']=='ACTIVE','wrong recurring schedule')
            no_model(app.pid);receipt['phases']['chat_approval']='PASS'
            # Independent second conversation, selected by the ordinary native form.
            wait(lambda:named(app.pid,'Message Lu',actionable=True),20,'composer restored')
            action(app.pid,'New Chat');navigate(app.pid)
            once=new_form(app.pid,path,ONCE,1)
            require(once['workspace_id']!=r['workspace_id'] and once['repeat_seconds'] is None and once['state']=='ACTIVE','one-time ownership mismatch')
            receipt['phases']['form_approval_and_conversation_ownership']='PASS'
            card_action(app.pid,RECURRING,'Pause reminder')
            wait(lambda:journal(path).get(RECURRING,{}).get('state')=='PAUSED',15,'paused')
            card_action(app.pid,RECURRING,'Resume reminder')
            wait(lambda:journal(path).get(RECURRING,{}).get('state')=='ACTIVE',15,'resumed')
            canceled=new_form(app.pid,path,CANCEL,5)
            card_action(app.pid,CANCEL,'Cancel reminder…');card_action(app.pid,CANCEL,'Confirm cancellation')
            wait(lambda:journal(path).get(CANCEL,{}).get('state')=='CANCELLED',15,'cancelled')
            receipt['phases']['pause_resume_and_cancel']='PASS'
            capture(a.out,'scheduled')
            # Let the two exact first due times elapse while the app is absent.
            # Kill the ordinary app rather than change the clock or DB.
            before=journal(path);due=max(before[RECURRING]['next_due'],before[ONCE]['next_due'])
            require(time.time()<min(before[RECURRING]['next_due'],before[ONCE]['next_due']),'could not prove closed-app due time; fixture too slow')
            app.kill();app.wait(timeout=15);app=None
            time.sleep(max(0,due-time.time()+2))
            closed=journal(path)
            require(closed[ONCE]['fired_count']==0 and closed[RECURRING]['fired_count']==0,'background app unexpectedly active after close')
            app=launch()
            def delivered():
                rows=journal(path)
                return rows if rows.get(ONCE,{}).get('fired_count')==1 and rows.get(RECURRING,{}).get('fired_count',0)>=1 else None
            after=wait(delivered,45,'missed durable reminders after reopen')
            require(after[ONCE]['state']=='COMPLETED' and after[ONCE]['unread_count']==1,'one-shot notice missing')
            require(after[RECURRING]['state']=='ACTIVE' and after[RECURRING]['last_due']>=before[RECURRING]['next_due'] and after[RECURRING]['next_due']==after[RECURRING]['last_due']+60,'recurrence cadence changed')
            require(after[CANCEL]['state']=='CANCELLED' and after[CANCEL]['fired_count']==0,'cancelled reminder replayed')
            receipt['phases']['closed_app_and_cold_recovery']='PASS'
            navigate(app.pid);capture(a.out,'due')
            card_action(app.pid,ONCE,'Mark read');wait(lambda:journal(path).get(ONCE,{}).get('unread_count')==0,15,'one-shot marked read')
            card_action(app.pid,RECURRING,'Mark read');wait(lambda:journal(path).get(RECURRING,{}).get('unread_count')==0,15,'repeat marked read')
            prior=journal(path)[RECURRING]['fired_count']
            recurrence=wait(lambda:(row if (row:=journal(path).get(RECURRING)) and row['fired_count']>prior else None),90,'actual next repeat')
            require(recurrence['unread_count']>0 and recurrence['state']=='ACTIVE','repeat missing unread result')
            receipt['phases']['actual_recurring_delivery']='PASS'
            card_action(app.pid,RECURRING,'Mark read');wait(lambda:journal(path).get(RECURRING,{}).get('unread_count')==0,15,'second repeat marked read')
            card_action(app.pid,RECURRING,'Cancel reminder…');card_action(app.pid,RECURRING,'Confirm cancellation')
            wait(lambda:journal(path).get(RECURRING,{}).get('state')=='CANCELLED',15,'repeat cancelled')
            no_model(app.pid);terminal=journal(path)
            app.kill();app.wait(timeout=15);app=launch();navigate(app.pid)
            require(journal(path)==terminal,'terminal reminder history replayed or changed')
            no_model(app.pid)
            receipt['phases']['terminal_restart_no_replay']='PASS'
            receipt['phases']['no_inference_worker']='PASS'
            receipt['results']=[{k:r[k] for k in ('text','state','fired_count','unread_count','repeat_seconds')} for r in terminal.values()]
            receipt['status']='PASS'
        except Exception as exc:
            receipt['failure']=str(exc)[:300]
            raise
        finally:
            if app is not None and app.poll() is None:
                app.terminate()
                try:app.wait(timeout=10)
                except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
            (a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')

if __name__=='__main__':main()
