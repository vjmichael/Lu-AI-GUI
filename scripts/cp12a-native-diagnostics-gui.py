#!/usr/bin/env python3
"""Installed exact-file Reader and consented DX report; disposable input, read-only journal inspection."""
import argparse, importlib.util, json, os, shutil, sqlite3, subprocess, time
from pathlib import Path
spec=importlib.util.spec_from_file_location('base',Path(__file__).with_name('cp11br3-changing-file-gui.py'))
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
bm=base.bm;gui=base.gui;require,wait,action,named=base.require,base.wait,base.action,base.named

def fields(b):
    i=0
    def varint():
        nonlocal i
        n=0
        for k in range(0,70,7):
            v=b[i];i+=1;n|=(v&127)<<k
            if v<128:return n
        raise ValueError('varint overflow')
    while i<len(b):
        key=varint();w=key&7
        if w==0:value=varint()
        elif w==2:
            size=varint();value=b[i:i+size];i+=size
            require(len(value)==size,'truncated event')
        else:raise ValueError('unexpected wire type')
        yield key>>3,value

def traces(path):
    try:
        with sqlite3.connect(path.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
            result=[]
            for ordinal,body,state in c.execute('SELECT ordinal,body,state FROM dx_trace ORDER BY ordinal'):
                fs=list(fields(body));ev=[dict(fields(v)) for k,v in fs if k==4];result.append({'ordinal':ordinal,'state':state,'dropped':next((v for k,v in fs if k==3),0),'events':[[e.get(3,0),e.get(4,0),e.get(7,0)] for e in ev]})
            return result
    except sqlite3.OperationalError:return []

def audit(root):
    for p in root.rglob('lu-reader-turn-audit.sqlite'):
        try:
            with sqlite3.connect(p.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
                return c.execute('SELECT kind,COUNT(*) FROM lu_model_boundary_audit GROUP BY kind').fetchall()
        except sqlite3.OperationalError:continue
    return []

def saved(root,prompt):
    for p in (root/'ansf'/'books').glob('*.sqlite'):
        try:
            with sqlite3.connect(p.as_uri()+'?mode=ro',uri=True,timeout=.1) as c:
                row=c.execute("""SELECT e.text FROM ansf_live_chat_events u JOIN ansf_visible_chat d ON d.session_id=u.session_id AND d.generation_id=u.generation_id JOIN ansf_live_chat_events e ON e.seq=d.latest_event_seq WHERE u.text=? AND u.kind='reader_user' AND u.origin='host' AND d.role='assistant' AND d.status='final' AND e.kind='assistant_progress' AND e.origin='host' AND EXISTS(SELECT 1 FROM ansf_live_chat_events c WHERE c.session_id=u.session_id AND c.generation_id=u.user_turn_id AND c.kind='assistant_completed' AND c.origin='host')""",(prompt,)).fetchone()
                if row:return row[0]
        except sqlite3.OperationalError:continue
    return None

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('exe','model','fresh-data-home','out','source'):p.add_argument('--'+n,type=Path,required=True)
    p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);a=p.parse_args()
    require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity');require(bm.sha(a.model)==a.model_sha256,'model identity')
    root=a.fresh_data_home.resolve()/'org.lu.desktop';folder=root/'lu-models';folder.mkdir(parents=True,exist_ok=False);shutil.copyfile(a.model,folder/a.model.name);require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture copy');a.out.mkdir(parents=True,exist_ok=False);require(not a.source.exists(),'fresh disposable file');a.source.write_text('The inventory verification code is ORCHID-731.\n')
    path=root/'lu-dx-traces.sqlite';prompt='Read file '+str(a.source);r={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}};app=None
    def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=10)
    with (a.out/'native.log').open('wb') as log:
        try:
            app=subprocess.Popen([str(a.exe)],env=dict(os.environ,XDG_DATA_HOME=str(a.fresh_data_home.resolve())),stdin=subprocess.DEVNULL,stdout=log,stderr=log)
            action(app.pid,'New Chat',timeout=90);worker=bm.load(app.pid);bm.send_chat(app.pid,prompt);base.review(app.pid,'Allow file access?','Cancel',[str(a.source),'for this request only','No files will be changed.'])
            denied=wait(lambda:next((v for v in traces(path) if v['state']=='sealed' and [20,3,3] in v['events']),None),20,'native refusal trace');require(not audit(root),'refusal executed tool');r['phases']['read_refusal']=denied;print('REFUSAL PASS',flush=True)
            # New chat avoids retaining a rejected composer request.
            action(app.pid,'New Chat');bm.send_chat(app.pid,prompt);base.review(app.pid,'Allow file access?','Allow once',[str(a.source),'for this request only','No files will be changed.'])
            result=wait(lambda:next((v for v in traces(path) if v['ordinal']>denied['ordinal'] and v['state']=='sealed'),None),130,'terminal genuine Reader diagnostic');r['phases']['read_terminal']=result;r['audit_counts']=dict(audit(root));answer=saved(root,prompt);r['saved_answer_bytes']=len(answer.encode()) if answer else None;r['answer_contains_fixture_code']=bool(answer and 'ORCHID-731' in answer);print('TRACE',result,'AUDIT',r['audit_counts'],flush=True)
            if [15,2,0] in result['events']:
                require([14,1,0] in result['events'] and [15,1,0] in result['events'] and r['audit_counts'].get('TOOL_FEEDBACK',0)>0,'feedback without actual dispatch/audit');r['phases']['independent_feedback']='PASS'
            if [12,2,0] in result['events']:
                require(answer is not None,'saved checkpoint without original saved response');r['phases']['independent_save']='PASS'
            require([13,2,0] not in result['events'],'false renderer confirmation');r['phases']['no_false_renderer']='PASS'
            action(app.pid,'Settings');action(app.pid,'Privacy & Observe');action(app.pid,'Review recent response issues…');base.review(app.pid,'Lu — Troubleshooting','Cancel',['not conversations, images, model names or file paths','Nothing is sent anywhere.']);r['phases']['report_refusal']='PASS'
            action(app.pid,'Review recent response issues…');base.review(app.pid,'Lu — Troubleshooting','Prepare report',['not conversations, images, model names or file paths','Nothing is sent anywhere.']);wait(lambda:named(app.pid,'The request was not authorized.'),25,'plain-language refusal');capture('troubleshooting');r['phases']['report_approval']='PASS';r['status']='PASS_SCOPED' if r['phases'].get('independent_save')=='PASS' and r['phases'].get('independent_feedback')=='PASS' else 'PARTIAL_NATIVE_OUTCOME'
        except Exception as e:r['blocker']=str(e);capture('failure');raise
        finally:
            (a.out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r),flush=True)
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
if __name__=='__main__':main()
