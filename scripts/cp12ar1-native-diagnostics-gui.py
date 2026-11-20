#!/usr/bin/env python3
"""Installed Chat/Reader delivery, cancellation and cold recovery. Only real GUI
interaction and read-only application journals; disposable input and profile."""
import argparse, importlib.util, json, os, shutil, sqlite3, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('dx',Path(__file__).with_name('cp12a-native-diagnostics-gui.py'))
dx=importlib.util.module_from_spec(spec);spec.loader.exec_module(dx)
bm=dx.bm;require,wait,action,named=dx.require,dx.wait,dx.action,dx.named

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','model','fresh-data-home','out','source'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed executable identity')
    require(bm.sha(a.model)==a.model_sha256,'model identity')
    root=a.fresh_data_home.resolve()/'org.lu.desktop';folder=root/'lu-models';folder.mkdir(parents=True,exist_ok=False);shutil.copyfile(a.model,folder/a.model.name)
    require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture model copy');a.out.mkdir(parents=True,exist_ok=False);require(not a.source.exists(),'fresh input file');a.source.write_text('The test inventory code is ORCHID-731.\n')
    path=root/'lu-dx-traces.sqlite';env=dict(os.environ,XDG_DATA_HOME=str(a.fresh_data_home.resolve()));r={'status':'INCOMPLETE','binary_sha256':a.sha256,'phases':{}};app=None
    with (a.out/'native.log').open('wb') as log:
        def start():return subprocess.Popen([str(a.exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
        def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=10)
        def newest():return max((v['ordinal'] for v in dx.traces(path)),default=0)
        def find(after,criterion,timeout=120):return wait(lambda:next((v for v in dx.traces(path) if v['ordinal']>after and criterion(v)),None),timeout,'exact next diagnostic witness')
        def active(after,worker):
            before=bm.ticks(worker);require(before is not None,'owned model worker absent')
            return find(after,lambda v:v['state']=='live' and [8,1,0] in v['events'] and [9,2,0] not in v['events'] and (bm.ticks(worker) or 0)>before,30)
        def mark(key,value):r['phases'][key]=value;print(key,value,flush=True);(a.out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n')
        try:
            app=start();action(app.pid,'New Chat',timeout=90);worker=bm.load(app.pid)
            baseline=newest();prompt='Say hello in one very short sentence. /no_think';bm.send_chat(app.pid,prompt)
            chat=find(baseline,lambda v:v['state']=='sealed' and [13,2,0] in v['events']);require(chat['dropped']==0 and [10,2,0] in chat['events'] and [11,2,0] in chat['events'] and bm.completed_chat(root,prompt),'ordinary final response not independently saved');mark('ordinary_chat_display',chat);capture('ordinary-chat')
            action(app.pid,'New Chat');baseline=newest();prompt='Read file '+str(a.source);bm.send_chat(app.pid,prompt);dx.base.review(app.pid,'Allow file access?','Cancel',[str(a.source),'for this request only','No files will be changed.']);refused=find(baseline,lambda v:v['state']=='sealed' and [20,3,3] in v['events']);require(not dx.audit(root),'refused Reader executed');mark('reader_refusal',refused)
            action(app.pid,'New Chat');baseline=newest();bm.send_chat(app.pid,prompt);dx.base.review(app.pid,'Allow file access?','Allow once',[str(a.source),'for this request only','No files will be changed.']);reader=find(baseline,lambda v:v['state']=='sealed' and [13,2,0] in v['events']);answer=dx.saved(root,prompt);require(reader['dropped']==0 and [14,1,0] in reader['events'] and [15,2,0] in reader['events'] and [12,2,0] in reader['events'] and dict(dx.audit(root)).get('TOOL_FEEDBACK',0)>0 and answer and 'ORCHID-731' in answer,'Reader final delivery not supported by independent source and saved answer');mark('reader_display',reader);capture('reader-chat')
            action(app.pid,'New Chat');baseline=newest();long='Write a long original story of a thousand sentences about a lighthouse. Do not summarize. /no_think';worker=bm.model_worker(app.pid);require(worker is not None,'original model absent');bm.send_chat(app.pid,long);inflight=active(baseline,worker);action(app.pid,'Cancel',timeout=10);cancelled=find(baseline,lambda v:v['ordinal']==inflight['ordinal'] and v['state']=='sealed' and any(e[2]==9 for e in v['events']),45);require([13,2,0] not in cancelled['events'] and not any(e[0] in (17,18) for e in cancelled['events']),'cancel trace fabricated delivery or exit');mark('active_chat_cancel',cancelled);capture('cancelled')
            # A stopped model may need explicit reload. Do not adjust startup limits.
            action(app.pid,'New Chat');worker=bm.model_worker(app.pid)
            if worker is None:worker=bm.load(app.pid)
            baseline=newest();long='Write a long original story about a mountain with a thousand detailed sentences. Do not summarize. /no_think';bm.send_chat(app.pid,long);inflight=active(baseline,worker);mark('actual_inflight_before_crash',inflight)
            # Kill only this disposable tested host; never write journal or change clock.
            app.kill();app.wait(10);wait(lambda:bm.ticks(worker) is None,25,'owned original model worker exit');before=dx.traces(path);require(any(v['ordinal']==inflight['ordinal'] and v['state']=='live' for v in before),'no live persisted trace remained');app=start();action(app.pid,'New Chat',timeout=90)
            recovered=find(baseline,lambda v:v['ordinal']==inflight['ordinal'] and v['state']=='recovered',30);require(recovered['events']==inflight['events'] or (recovered['events'][:len(inflight['events'])]==inflight['events']),'recovery rewrote prefix');require([13,2,0] not in recovered['events'] and bm.model_worker(app.pid) is None,'recovery replay or false display');after=dx.traces(path);require(all(v in after for v in before if v['state']=='sealed'),'cold recovery changed sealed historical records');mark('abrupt_recovery',recovered)
            action(app.pid,'Settings');action(app.pid,'Privacy & Observe');action(app.pid,'Review recent response issues…');dx.base.review(app.pid,'Lu — Troubleshooting','Prepare report',['not conversations, images, model names or file paths','Nothing is sent anywhere.']);time.sleep(1);capture('troubleshooting');mark('report_approval','GRAPHICAL_ACTION_AND_SCREENSHOT');r['status']='PASS_SCOPED'
        except BaseException as e:
            r['error']=str(e)
            try:capture('failure')
            except Exception:pass
            raise
        finally:
            r['final_traces']=dx.traces(path);(a.out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n')
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
if __name__=='__main__':main()
