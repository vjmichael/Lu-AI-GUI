#!/usr/bin/env python3
"""Qualify scheduled diagnostic checkpoints against original journal and real GGUF.
Only native GUI actions and read-only SQL; no simulated clock or production data.
"""
import argparse, importlib.util, json, os, shutil, sqlite3, subprocess, time
from pathlib import Path

def load(name,file):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(file));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
base=load('reader','cp11br2-scheduled-reader-gui.py');dx=load('dx','cp12a-native-diagnostics-gui.py')
bm=base.bm;require,wait,action=base.require,base.wait,base.action

def checkpoints(path):
    try:
        with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
            return dict(c.execute('SELECT sequence,phase FROM lu_ai_diagnostic_progress'))
    except sqlite3.OperationalError:return {}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','model','fresh-data-home','out'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity');require(bm.sha(a.model)==a.model_sha256,'model identity')
    root=a.fresh_data_home.resolve()/'org.lu.desktop';folder=root/'lu-models';folder.mkdir(parents=True,exist_ok=False);shutil.copyfile(a.model,folder/a.model.name);require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture copy')
    a.out.mkdir(parents=True,exist_ok=False);path=root/'lu-background-schedule.sqlite';env=dict(os.environ,XDG_DATA_HOME=str(a.fresh_data_home.resolve()));r={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}};app=None
    def persist(): (a.out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n')
    def mark(k,v):r['phases'][k]=v;persist();print(k,v,flush=True)
    def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=10)
    def source(seq):
        rr=[v for v in base.journal(path)[base.PROMPT]['receipts'] if v['sequence']==seq]
        require(len(rr)==1 and rr[0]['line']==2 and rr[0]['verified_text']==base.EXPECTED and rr[0]['sha256']==__import__('hashlib').sha256(base.NOTE.encode()).hexdigest(),'original Reader witness mismatch');return rr[0]
    with (a.out/'native.log').open('wb') as log:
        def start(): return subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        try:
            app=start();action(app.pid,'New Chat',timeout=90);worker=bm.load(app.pid);row=base.create(app.pid,path,base.PROMPT,True);require(row['grant']['note']==base.NOTE and row['grant']['line']==2 and row['repeat_seconds']==60 and not row['runs'],'scope mismatch');mark('native_approval','PASS')
            before=bm.ticks(worker);first=base.terminal(path,1);phase=checkpoints(path).get(1);src=source(1)
            require((bm.ticks(worker) or 0)>before and bm.model_worker(app.pid)==worker,'inference witness absent');require(first['started']>=first['due'],'early execution')
            require((first['state']=='COMPLETED' and first['output']==base.EXPECTED and phase==6) or (first['state']=='FAILED' and not first['output'] and phase==7),'terminal outcome not supported by checkpoint')
            mark('first_terminal',{'state':first['state'],'phase':phase,'source_match':first['output']==base.EXPECTED,'receipt_verified':True});capture('first-result')
            before=bm.ticks(worker)
            def active():
                run=base.run_row(path,2)
                if not run or run['state']!='RUNNING' or checkpoints(path).get(2)!=5:return None
                require(run['due']-first['due']==60,'incorrect real cadence');source(2)
                return run if (bm.ticks(worker) or 0)>before else None
            second=wait(active,115,'genuine final inference with original Reader receipt');mark('active_second',{'state':second['state'],'phase':5,'receipt_verified':True});app.kill();app.wait(10);wait(lambda:bm.ticks(worker) is None,25,'original worker exit');app=start();action(app.pid,'New Chat',timeout=90)
            recovered=wait(lambda:v if (v:=base.run_row(path,2)) and v['state']=='INTERRUPTED' else None,30,'cold interruption recovery');require(checkpoints(path).get(2)==5 and source(1)==src and base.run_row(path,1)==first and bm.model_worker(app.pid) is None,'recovery rewrote result, checkpoint or launched model');source(2);mark('cold_recovery',{'state':recovered['state'],'phase':5,'previous_result_preserved':True,'no_automatic_model':True})
            action(app.pid,'Settings');action(app.pid,'Privacy & Observe');action(app.pid,'Review recent response issues…');dx.base.review(app.pid,'Lu — Troubleshooting','Prepare report',['not conversations, images, model names or file paths','Nothing is sent anywhere.']);time.sleep(1);capture('scheduled-diagnostics');mark('report','GRAPHICAL_ACTION_AND_SCREENSHOT');r['status']='PASS_SCOPED'
        except BaseException as e:
            r['error']=str(e)
            try:capture('failure')
            except Exception:pass
            raise
        finally:
            r['checkpoints']=checkpoints(path);r['journal']=base.journal(path);persist()
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
if __name__=='__main__':main()
