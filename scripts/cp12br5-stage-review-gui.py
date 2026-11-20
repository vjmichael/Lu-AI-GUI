#!/usr/bin/env python3
"""Installed two-instance opt-in stage review; only UI actions and read-only witnesses.
No invitation, prompt/model output, application database or user data exported.
"""
import argparse,importlib.util,json,os,shutil,subprocess,time
from pathlib import Path
spec=importlib.util.spec_from_file_location('dx',Path(__file__).with_name('cp12a-native-diagnostics-gui.py'));dx=importlib.util.module_from_spec(spec);spec.loader.exec_module(dx)
bm=dx.bm;gui=dx.gui;require,wait,action,named=dx.require,dx.wait,dx.action,dx.named
SHARE='Lu — Review and share stages';RECEIVE='Lu — Review received stages'
CREATE='Review affected steps with loaded model…';ACCEPT='Receive and review affected steps…'
def main():
 p=argparse.ArgumentParser(description=__doc__)
 for n in ('exe','model','out'):p.add_argument('--'+n,type=Path,required=True)
 p.add_argument('--sha256',required=True);p.add_argument('--model-sha256',required=True);p.add_argument('--fault',action='store_true',help='qualify an actual refused native request with sequential workers');a=p.parse_args()
 require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity');require(bm.sha(a.model)==a.model_sha256,'model identity')
 a.out.mkdir(parents=True,exist_ok=False);receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{}};apps=[];logs=[];profiles={}
 def record(): (a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 def mark(k,v='PASS'):receipt['phases'][k]=v;record();print(k,v,flush=True)
 def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=8)
 def focus(pid):
  wins=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(pid)],text=True,timeout=5).splitlines();require(wins,'no app window');subprocess.run(['xdotool','windowactivate','--sync',wins[-1]],check=True,timeout=5)
 def start(name):
  root=a.out/name;root.mkdir();data=root/'data'/'org.lu.desktop';folder=data/'lu-models';folder.mkdir(parents=True);shutil.copyfile(a.model,folder/a.model.name);require(bm.sha(folder/a.model.name)==a.model_sha256,'fixture copy mismatch')
  log=(a.out/(name+'.log')).open('ab');logs.append(log);app=subprocess.Popen([str(a.exe)],env=dict(os.environ,XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache')),stdin=subprocess.DEVNULL,stdout=log,stderr=log);apps.append(app);profiles[app.pid]=data;action(app.pid,'New Chat',timeout=90);return app
 def settings(pid):
  focus(pid);action(pid,'Settings');action(pid,'Privacy & Observe');wait(lambda:named(pid,CREATE,actionable=True),25,'review control')
 def sockets():return {str(p) for p in Path('/tmp').glob('lu-dx-*') if p.is_dir() and p.stat().st_uid==os.geteuid() and (p/'s').is_socket()}
 def scroll(pid):focus(pid);subprocess.run(['xdotool','mousemove','1050','850','click','--repeat','8','5'],check=True,timeout=8);time.sleep(.5)
 def consent(pid,which,approve):
  if which==SHARE:dx.base.review(pid,which,'Review and share' if approve else 'Cancel',['up to 64 recent records','observed stops or failures and unconfirmed steps','No individual traces, timings, conversations, files, model identities or tool access'])
  else:dx.base.review(pid,which,'Receive and review' if approve else 'Cancel',['independently classify the same bounded outcome and checkpoint counts',"will not see the other model's answer",'No individual traces, timings, conversations, files, tools or repairs','consumed on receipt'])
 def invite(pid,approve):focus(pid);action(pid,CREATE);consent(pid,SHARE,approve)
 def receive(pid,approve):
  focus(pid);n=wait(lambda:named(pid,'Invitation from another Lu',actionable=True),20,'invitation entry');require(n.get_component_iface().grab_focus(),'invitation focus');wait(lambda:n.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'invitation focus');subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','ctrl+v'],check=True,timeout=8);action(pid,ACCEPT,timeout=20);consent(pid,RECEIVE,approve)
 try:
  before=sockets();sender=start('sender');focus(sender.pid);bm.send_chat(sender.pid,'Show local model status');tracepath=profiles[sender.pid]/'lu-dx-traces.sqlite';trace=wait(lambda:next((t for t in dx.traces(tracepath) if t['state']=='sealed' and [13,2,0] in t['events']),None),40,'source CPU delivery trace');require(len(dx.traces(tracepath))==1 and trace['dropped']==0 and bm.model_worker(sender.pid) is None,'source trace mismatch');mark('source_witness',{'confirmed':1,'others':0,'native_cpu':True})
  # Two native processes and independent workers; fault case loads sequentially
  # to bound memory on this laboratory host.
  sw=bm.load(sender.pid);mark('sender_model_loaded',True)
  if a.fault:
   source=a.out/'sample.txt';source.write_text('Disposable diagnostic input.\n');action(sender.pid,'New Chat');bm.send_chat(sender.pid,'Read file '+str(source));dx.base.review(sender.pid,'Allow file access?','Cancel',[str(source),'for this request only','No files will be changed.'])
   refused=wait(lambda:next((t for t in dx.traces(tracepath) if t['state']=='sealed' and [20,3,3] in t['events']),None),25,'native refusal trace');require(refused['dropped']==0 and not dx.audit(profiles[sender.pid]),'refusal evidence mismatch');require(len(dx.traces(tracepath))==2,'unexpected source records');mark('fault_witness',{'actual_permission_refusal':True,'no_native_tool_dispatch':True,'confirmed':1,'observed_failure':1})
  original=dx.traces(tracepath)
  if not a.fault:
   receiver=start('receiver');focus(receiver.pid);rw=bm.load(receiver.pid);require(sw!=rw and bm.model_worker(sender.pid)==sw and bm.model_worker(receiver.pid)==rw,'two distinct model workers');mark('receiver_model_loaded',True);settings(receiver.pid)
  else:receiver=None;rw=None
  settings(sender.pid);require(sockets()==before,'listener without consent');mark('default_off')
  st=bm.ticks(sw);invite(sender.pid,False);wait(lambda:named(sender.pid,CREATE,actionable=True),15,'sender refusal idle');require(sockets()==before and bm.ticks(sw)==st,'sender refusal produced execution');mark('sender_refusal')
  st=bm.ticks(sw);rt=bm.ticks(rw) if rw else None;invite(sender.pid,True);wait(lambda:named(sender.pid,'Copy invitation',actionable=True) or (named(sender.pid,CREATE,actionable=True) and named(sender.pid,'Stop review') is None),50,'sender review settled');
  if not named(sender.pid,'Copy invitation',actionable=True):
   scroll(sender.pid);scroll(sender.pid);capture('sender-review-refusal');mark('sender_no_verified_invitation',{'worker_active':bm.ticks(sw)>st,'no_endpoint':sockets()==before});raise RuntimeError('model did not produce verified stage finding')
  require(bm.ticks(sw)>st and (rw is None or bm.ticks(rw)==rt),'sender model execution witness');created=sockets()-before;require(len(created)==1,'exact one review endpoint');ep=Path(next(iter(created)));require(ep.stat().st_mode&0o777==0o700 and (ep/'s').stat().st_mode&0o777==0o600,'private endpoint');action(sender.pid,'Copy invitation');mark('sender_model_review',{'cpu_ticks_delta':bm.ticks(sw)-st,'receiver_idle':True})
  if a.fault:
   action(sender.pid,'Chat');action(sender.pid,'Choose a model');action(sender.pid,'⏏ Unload active model');wait(lambda:bm.model_worker(sender.pid) is None,35,'sender model unloaded');require(ep.exists(),'approved invitation lost during unload');mark('sender_unloaded_for_memory')
   receiver=start('receiver');focus(receiver.pid);rw=bm.load(receiver.pid);require(sw!=rw and bm.model_worker(sender.pid) is None,'independent sequential receiver worker');mark('receiver_model_loaded',True);settings(receiver.pid)
  rt=bm.ticks(rw);receive(receiver.pid,False);wait(lambda:named(receiver.pid,'Invitation from another Lu',actionable=True),15,'receiver refusal idle');require(ep.exists() and bm.ticks(rw)==rt,'receiver refusal consumed or ran');mark('receiver_refusal')
  rt=bm.ticks(rw);st=bm.ticks(sw) if not a.fault else None;receive(receiver.pid,True);wait(lambda:not ep.exists(),15,'one use endpoint retired');wait(lambda:named(receiver.pid,'Stop review') is None,45,'stage review settled');scroll(receiver.pid);scroll(receiver.pid);wait(lambda:named(receiver.pid,'Clear affected-step review',actionable=True),20,'verified two-model result');require(bm.ticks(rw)>rt and (bm.model_worker(sender.pid) is None if a.fault else bm.ticks(sw)==st),'independent receiver model execution witness');mark('receiver_model_review',{'cpu_ticks_delta':bm.ticks(rw)-rt,'sender_idle':True});require((bm.model_worker(sender.pid) is None if a.fault else bm.model_worker(sender.pid)==sw) and bm.model_worker(receiver.pid)==rw,'worker replacement');require(dx.traces(tracepath)==original,'source trace changed');capture('two-model-result');mark('verified_graphical_result','VISUAL_REVIEW_REQUIRED')
  action(receiver.pid,'Clear affected-step review');rt=bm.ticks(rw);receive(receiver.pid,True);time.sleep(1);scroll(receiver.pid);require(named(receiver.pid,'Clear affected-step review',actionable=True) is None and bm.ticks(rw)==rt,'replayed invitation ran model');capture('replay-rejected');mark('replay','VISUAL_REVIEW_REQUIRED');receipt['status']='PASS_SCOPED_VISUAL_REVIEW_REQUIRED'
 except BaseException as e:
  receipt['error']=str(e);print('INCOMPLETE',str(e),flush=True)
  try:capture('failure')
  except Exception:pass
  raise
 finally:
  for app in apps:
   if app.poll() is None:
    app.terminate()
    try:app.wait(15)
    except subprocess.TimeoutExpired:app.kill();app.wait(5)
  for log in logs:log.close()
  record()
if __name__=='__main__':main()
