#!/usr/bin/env python3
"""Resume the accepted scheduled Reader profile, interrupt an actual occurrence,
then reopen without model load and verify the completed prefix and no replay."""
import argparse, importlib.util, json, os, signal, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('base',Path(__file__).with_name('cp11br2-scheduled-reader-gui.py'));base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
bm=base.bm;gui=base.gui;require,wait,action,named=base.require,base.wait,base.action,base.named

def card_action(pid,label):
    def find(name,root=None):
        found=[]
        for n in gui.nodes(root or gui.Atspi.get_desktop(0)):
            try:
                if n.get_name()==name and gui.owned(n.get_process_id(),pid):found.append(n)
            except Exception:pass
        return found[0] if len(found)==1 else None
    card=wait(lambda:find('Scheduled task: '+base.PROMPT),20,'task card');button=wait(lambda:find(label,card),20,label)
    try:button.get_component_iface().scroll_to(gui.Atspi.ScrollType.ANYWHERE)
    except Exception:pass
    for _ in range(10):
        card=find('Scheduled task: '+base.PROMPT)
        if card is not None and named(pid,label,card,True):break
        subprocess.run(['xdotool','mousemove','900','750','click','5','click','5','click','5'],check=True,timeout=5);time.sleep(.15)
    action(pid,label,wait(lambda:find('Scheduled task: '+base.PROMPT),10,'card'),timeout=15)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('exe','data-home','out'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--sha256',required=True);a=p.parse_args();require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity')
    path=a.data_home.resolve()/'org.lu.desktop'/'lu-background-schedule.sqlite';a.out.mkdir(parents=True,exist_ok=False);env=dict(os.environ,XDG_DATA_HOME=str(a.data_home.resolve()));verdict={'status':'INCOMPLETE','binary_sha256':a.sha256,'phases':{}};app=None
    prefix=base.journal(path)[base.PROMPT];require(prefix['run_count']==2 and all(r['state']=='COMPLETED' for r in prefix['runs']) and len(prefix['receipts'])==2,'missing exact previous prefix')
    with (a.out/'native.log').open('wb') as log:
        try:
            app=subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log);base.navigate(app.pid);require(not bm.model_worker(app.pid),'unexpected auto load');require(base.journal(path)[base.PROMPT]==prefix,'cold restart changed prefix');verdict['phases']['cold_restart_prefix_no_autoload']='PASS'
            action(app.pid,'Chat');action(app.pid,'Choose a model',timeout=60);action(app.pid,'Load selected',timeout=30);worker=wait(lambda:bm.model_worker(app.pid),90,'owned inference worker');before=bm.ticks(worker);require(before is not None,'no original worker ticks')
            end=time.monotonic()+125;interrupted=None
            # Do not artificially hold, inject, or delay a tool. Poll the real
            # journal and process for a genuine in-flight occurrence.
            while time.monotonic()<end:
                r=base.journal(path)[base.PROMPT]
                active=next((x for x in r['runs'] if x['sequence']>=3 and x['state']=='RUNNING'),None)
                advance=bm.ticks(worker)
                if active and advance is not None and advance>before:
                    observed=next((x for x in r['receipts'] if x['sequence']==active['sequence']),None)
                    if observed:
                        interrupted=(active['sequence'],observed);os.kill(app.pid,signal.SIGKILL);app.wait(10);break
                time.sleep(.015)
            require(interrupted is not None,'did not witness in-flight inference after Reader receipt')
            seq,observed=interrupted;verdict['phases']['active_inference_interrupted_after_read']={'sequence':seq,'status':'PASS'}
            wait(lambda:bm.ticks(worker) is None,25,'original worker gone');verdict['phases']['worker_exit']='PASS'
            app=subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log);base.navigate(app.pid);r=base.journal(path)[base.PROMPT];require(not bm.model_worker(app.pid),'model autoload after crash');require(r['runs'][:2]==prefix['runs'] and r['receipts'][:2]==prefix['receipts'],'changed completed prefix');attempt=next(x for x in r['runs'] if x['sequence']==seq);require(attempt['state']=='INTERRUPTED' and not attempt['output'],'interrupted attempt promoted');require(next(x for x in r['receipts'] if x['sequence']==seq)==observed,'lost verified receipt');require(r['run_count']==seq,'duplicate claim');verdict['phases']['cold_recovery_prefix_receipt_no_replay']='PASS'
            card_action(app.pid,'View results');time.sleep(.6);subprocess.run(['scrot',str(a.out/'recovered-results.png')],check=True,timeout=10)
            card_action(app.pid,'Cancel task…');card_action(app.pid,'Confirm task cancellation');r=wait(lambda:v if (v:=base.journal(path)[base.PROMPT])['state']=='CANCELLED' else None,20,'cancel durable');require(r['runs'][:2]==prefix['runs'] and r['receipts'][:2]==prefix['receipts'],'cancel changed completed prefix');verdict['phases']['cancel_preserved_results']='PASS';verdict['status']='PASS'
        except BaseException as e:verdict['error']=str(e);raise
        finally:
            verdict['journal']=base.journal(path)
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
            (a.out/'receipt.json').write_text(json.dumps(verdict,indent=2)+'\n')
if __name__=='__main__':main()
