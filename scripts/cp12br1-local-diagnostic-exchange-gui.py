#!/usr/bin/env python3
"""Two real installed Lu processes, separate disposable profiles, same non-root OS account.
Native review, normal clipboard and UI actions only; independent trace readback.
Invitation is never printed, captured as text, or persisted in evidence.
"""
import argparse,importlib.util,json,os,re,subprocess,time
from pathlib import Path
spec=importlib.util.spec_from_file_location('dx',Path(__file__).with_name('cp12a-native-diagnostics-gui.py'));dx=importlib.util.module_from_spec(spec);spec.loader.exec_module(dx)
bm=dx.bm;gui=dx.gui;require,wait,action,named=dx.require,dx.wait,dx.action,dx.named
SHARE='Lu — Share troubleshooting summary';RECEIVE='Lu — Receive troubleshooting summary'
CREATE='Create sharing invitation…';ACCEPT='Receive summary…'
def main():
 import tkinter
 clipboard=tkinter.Tk();clipboard.withdraw()
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--exe',type=Path,required=True);p.add_argument('--sha256',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
 require(os.geteuid()!=0 and a.exe.stat().st_uid==0 and not a.exe.stat().st_mode&0o022 and bm.sha(a.exe)==a.sha256,'installed binary identity');a.out.mkdir(parents=True,exist_ok=False)
 receipt={'status':'INCOMPLETE','binary_sha256':a.sha256,'phases':{}};apps=[];logs=[];profile={};clip_holders=[]
 def record(): (a.out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 def mark(k,v='PASS'):receipt['phases'][k]=v;record();print(k,v,flush=True)
 def start(name):
  root=a.out/name;root.mkdir(exist_ok=True);log=(a.out/(name+'.log')).open('ab');logs.append(log)
  app=subprocess.Popen([str(a.exe)],env=dict(os.environ,XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache')),stdin=subprocess.DEVNULL,stdout=log,stderr=log);apps.append(app);profile[app.pid]=root/'data'/'org.lu.desktop';action(app.pid,'New Chat',timeout=90);return app
 def focus(pid):
  wins=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(pid)],text=True,timeout=5).splitlines()
  require(wins,'no app window');subprocess.run(['xdotool','windowactivate','--sync',wins[-1]],check=True,timeout=5)
 def settings(pid):focus(pid);action(pid,'Settings');action(pid,'Privacy & Observe');wait(lambda:named(pid,CREATE,actionable=True),25,'sharing off control')
 def sockets():return {str(p) for p in Path('/tmp').glob('lu-dx-*') if p.is_dir() and p.stat().st_uid==os.geteuid() and (p/'s').is_socket()}
 def review(pid,which,choice):
  phrases=['Only five counts are shared','No conversations, model names, files, or tool access','two minutes or one use'] if which==SHARE else ['Only count totals','neither session access to your conversations, files, tools or model']
  dx.base.review(pid,which,choice,phrases)
 def invite(pid):
  focus(pid);action(pid,CREATE);review(pid,SHARE,'Create invitation');wait(lambda:named(pid,'Copy invitation',actionable=True),15,'copy invitation');action(pid,'Copy invitation')
 def receive(pid,approve):
  focus(pid);node=wait(lambda:named(pid,'Invitation from another Lu',actionable=True),20,'invitation entry');require(node.get_component_iface().grab_focus(),'invitation focus');wait(lambda:node.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'invitation focus');subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a','ctrl+v'],check=True,timeout=8);action(pid,ACCEPT,timeout=20);review(pid,RECEIVE,'Receive summary' if approve else 'Cancel')
 def scroll_result(pid):
  focus(pid);subprocess.run(['xdotool','mousemove','1050','850','click','--repeat','5','5'],check=True,timeout=8);time.sleep(.5)
 def visible_text(pid,text):
  for n in dx.base.nodes(gui.Atspi.get_desktop(0)):
   try:
    if n.get_process_id()!=pid or not n.get_state_set().contains(gui.Atspi.StateType.SHOWING):continue
    if text in (n.get_name() or ""):return True
    iface=n.get_text_iface()
    if iface and text in gui.Atspi.Text.get_text(iface,0,-1):return True
   except Exception:pass
  return False
 def rejected(pid,label):
  scroll_result(pid);time.sleep(1);capture(label);require(named(pid,"Clear summary",actionable=True) is None,"unexpected received summary")
  mark(label,"PASS_ACCESSIBLE" if visible_text(pid,"The invitation is invalid, expired, incompatible or unavailable.") else "VISUAL_REVIEW_REQUIRED")
 def capture(name):subprocess.run(['scrot',str(a.out/(name+'.png'))],check=True,timeout=8)
 try:
  before=sockets();sender=start('sender');focus(sender.pid);bm.send_chat(sender.pid,'Show local model status');path=profile[sender.pid]/'lu-dx-traces.sqlite'
  trace=wait(lambda:next((t for t in dx.traces(path) if t['state']=='sealed' and [13,2,0] in t['events']),None),40,'saved and renderer-correlated CPU status trace');require(len(dx.traces(path))==1 and trace['dropped']==0 and bm.model_worker(sender.pid) is None,'source trace mismatch');mark('source_witness',{'count':1,'complete':1,'native_cpu':True})
  receiver=start('receiver');settings(sender.pid);settings(receiver.pid);require(sockets()==before,'listener started without consent');mark('off_by_default')
  focus(sender.pid);action(sender.pid,CREATE);review(sender.pid,SHARE,'Cancel');require(sockets()==before,'refusal started listener');mark('sender_refusal')
  invite(sender.pid);created=wait(lambda:sockets()-before,5,'one private listener');require(len(created)==1,'unexpected listener count');endpoint=Path(next(iter(created)));require(endpoint.stat().st_mode&0o777==0o700 and (endpoint/'s').stat().st_mode&0o777==0o600,'endpoint permissions');mark('sender_approval')
  receive(receiver.pid,False);require(endpoint.exists(),'refusal consumed invitation');mark('receiver_refusal')
  receive(receiver.pid,True);wait(lambda:not endpoint.exists(),15,'single-use listener retired');scroll_result(receiver.pid);wait(lambda:named(receiver.pid,'Clear summary',actionable=True),15,'received summary displayed');focus(receiver.pid);capture('received-summary');mark('two_process_exchange','GRAPHICAL_RESULT_AND_INDEPENDENT_SOURCE_TRACE')
  receive(receiver.pid,True);rejected(receiver.pid,'replay_rejected')
  invite(sender.pid);second=wait(lambda:sockets()-before,5,'fresh listener');require(len(second)==1 and second!=created,'invitation not rotated');action(sender.pid,'Stop sharing');wait(lambda:sockets()==before,15,'listener revoked');wait(lambda:named(sender.pid,CREATE,actionable=True),10,'sharing off after revocation');mark('stop_sharing');receive(receiver.pid,True);rejected(receiver.pid,'revoked_invitation_rejected')
  invite(sender.pid);third=wait(lambda:sockets()-before,5,'fresh pre-crash listener');require(len(third)==1 and third!=second,'fresh invitation after revocation');old_code=clipboard.clipboard_get();require(isinstance(old_code,str) and len(old_code)==101,'clipboard not copied');sender.kill();sender.wait(10);mark('abrupt_sender_exit');sender=start('sender');settings(sender.pid);require(sockets()-before==third,'unexpected new listener after cold restart');require(len(dx.traces(path))==1,'source trace not preserved');mark('cold_restart_default_off');holder=subprocess.Popen(['/usr/bin/python3','-c','import sys,tkinter; c=tkinter.Tk();c.withdraw();c.clipboard_clear();c.clipboard_append(sys.stdin.read());c.update();c.mainloop()'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);holder.stdin.write(old_code.encode());holder.stdin.close();clip_holders.append(holder);time.sleep(1);require(clipboard.clipboard_get()==old_code,'test clipboard re-offer');old_code=None;receive(receiver.pid,True);rejected(receiver.pid,'pre_crash_invitation_rejected')
  invite(sender.pid);fresh=wait(lambda:(sockets()-before)-third,5,'fresh post-crash listener');require(len(fresh)==1 and fresh!=third,'post-restart invitation not rotated');receive(receiver.pid,True);wait(lambda:not Path(next(iter(fresh))).exists(),15,'fresh post-crash invitation retired');scroll_result(receiver.pid);wait(lambda:named(receiver.pid,'Clear summary',actionable=True),15,'fresh result displayed');capture('fresh-after-restart');mark('fresh_exchange_after_restart');require(bm.model_worker(sender.pid) is None and bm.model_worker(receiver.pid) is None,'model loaded unexpectedly');mark('no_model_loaded');receipt['status']='PASS_SCOPED_VISUAL_REVIEW_REQUIRED'
  # Clean only disposable endpoint verified to have been created by killed test process.
  stale=Path(next(iter(third)));(stale/'s').unlink();stale.rmdir()

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
  for holder in clip_holders:
   if holder.poll() is None:holder.terminate();holder.wait(5)
  clipboard.destroy()
  for log in logs:log.close()
  record()
if __name__=='__main__':main()
