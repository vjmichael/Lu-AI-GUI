#!/usr/bin/env python3
"""Resume an existing disposable installed R3 profile; independently verify
retained outcomes, fresh file permission, actual recurrence and crash recovery."""
import argparse, importlib.util, hashlib, json, os, signal, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('fixture',Path(__file__).with_name('cp11br3-changing-file-gui.py'));f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
bm,base,rec,gui=f.bm,f.base,f.rec,f.gui
require,wait,action=f.require,f.wait,f.action

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','data-home','out','source'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'binary identity')
    path=a.data_home.resolve()/'org.lu.desktop'/'lu-background-schedule.sqlite';source=a.source.resolve();require(source.parent==Path.home() and source.name.startswith('CP11BR3-acceptance-disposable'),'disposable source only')
    prefix=f.journal(path);require(prefix and len(prefix['runs'])==2 and len(prefix['receipts'])==2 and prefix['runs'][0]['state']=='COMPLETED','exact saved prefix missing')
    a.out.mkdir(parents=True,exist_ok=False);env=dict(os.environ,XDG_DATA_HOME=str(a.data_home.resolve()));receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'phases':{}};app=None
    with (a.out/'native.log').open('wb') as log:
        def start():return subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        def stop():
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
        def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=10)
        def record(name):receipt['phases'][name]='PASS';print(name+' PASS',flush=True)
        def review(choice):
            rec.card_action(app.pid,'Review file permission…');f.picker(app.pid,source);f.review(app.pid,f.REAPPROVAL,choice,[str(source),'Line: 2','task stays paused'])
        def preserved(row):return row['runs'][:2]==prefix['runs'] and row['receipts'][:2]==prefix['receipts']
        try:
            app=start();base.navigate(app.pid);row=wait(lambda:f.journal(path),20,'reopened schedule');require(row['state']=='PAUSED' and row['grant']['permission']=='REVIEW' and preserved(row) and not bm.model_worker(app.pid),'restart restored authority or changed reports');record('restart_requires_review');capture('review-required')
            review('Decline');require(f.journal(path)['grant']['permission']=='REVIEW','decline changed permission');record('reapproval_refusal')
            review('Approve permission');row=wait(lambda:v if (v:=f.journal(path)) and v['grant']['permission']=='APPROVED' else None,20,'fresh approval');require(row['state']=='PAUSED' and preserved(row),'approval resumed or reports changed');record('fresh_reapproval_explicit_resume_required')
            original_inode=source.stat().st_ino;text='Sample inventory\nBLUE\nOther code LIMA-208';source.write_text(text);require(source.stat().st_ino==original_inode,'source replaced')
            action(app.pid,'Chat');worker=bm.load(app.pid);base.navigate(app.pid);rec.card_action(app.pid,'Resume task');r=f.verify(path,3,text);require(r['due']>prefix['runs'][1]['due'],'replayed due');require(preserved(f.journal(path)),'changed prefix');record('resumed_verified_read');rec.card_action(app.pid,'View results');capture('resumed-reports')
            before=bm.ticks(worker);end=time.monotonic()+115;observed=None
            while time.monotonic()<end:
                row=f.journal(path);active=next((r for r in row['runs'] if r['sequence']==4 and r['state']=='RUNNING'),None);ticks=bm.ticks(worker)
                if active and ticks is not None and ticks>before:
                    got=next((x for x in row['receipts'] if x['sequence']==4),None)
                    if got is not None:
                        observed=(got,row['runs'][:3],row['receipts'][:3]);os.kill(app.pid,signal.SIGKILL);app.wait(10);break
                time.sleep(.015)
            require(observed is not None,'did not observe actual in-flight inference after source receipt');record('abrupt_inference_interruption');wait(lambda:bm.ticks(worker) is None,25,'worker exit');record('worker_exit')
            app=start();base.navigate(app.pid);row=f.journal(path);require(row['state']=='PAUSED' and row['grant']['permission']=='REVIEW' and row['runs'][:3]==observed[1] and row['receipts'][:3]==observed[2] and row['receipts'][3]==observed[0] and row['runs'][3]['state']=='INTERRUPTED' and not row['runs'][3]['output'] and not bm.model_worker(app.pid),'unsafe restart or lost report');record('abrupt_recovery_no_replay_retained_prefix');rec.card_action(app.pid,'View results');capture('recovered-reports')
            rec.card_action(app.pid,'Revoke file permission…');rec.card_action(app.pid,'Confirm permission revocation');row=wait(lambda:v if (v:=f.journal(path)) and v['grant']['permission']=='REVOKED' else None,20,'revoked');require(row['state']=='CANCELLED' and row['runs'][:3]==observed[1] and row['receipts'][:3]==observed[2] and row['receipts'][3]==observed[0],'revocation altered prior reports');record('revocation_preserves_reports');capture('revoked');stop();app=start();base.navigate(app.pid);require(f.journal(path)==row and not bm.model_worker(app.pid),'revoked permission restored');record('revoked_cold_restart');receipt['status']='PASS'
        except BaseException as e:receipt['error']=str(e);raise
        finally:
            receipt['journal']=f.journal(path);stop();(a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
