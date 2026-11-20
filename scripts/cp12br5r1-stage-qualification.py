#!/usr/bin/env python3
"""Offline, synthetic diagnostic classification comparison; not installed acceptance.

Runs one actual hash-pinned local model per process. Never uses application data,
network services, or a peer model answer. A match is the exact visible label.
"""
import argparse, hashlib, json, os, random, secrets, socket, subprocess, time, urllib.request
from pathlib import Path
ENGINE_PINS={'libggml-base.so.0': {'bytes': 882888, 'sha256': '9c5339ee6d143145373a3e48c97d0fbbe74f2abeaf8acbe1c3f955a60eb3920e'}, 'libggml-cpu.so.0': {'bytes': 1234136, 'sha256': 'eda24f96ba1643abb41e4901dc564f2406fc61ff39e4923c6d8f6b8897ce1f2d'}, 'libggml.so.0': {'bytes': 55920, 'sha256': '3bf335264fee6eea0a0fdcd91eb488e47b0779d2595641accf3a1cb33ad3c14e'}, 'libllama-common.so.0': {'bytes': 6022544, 'sha256': '8d78df7c9c7e420151c07043f3cd61c157b3f26e412f0d044b05c6e8f80537d3'}, 'libllama-server-impl.so': {'bytes': 3868192, 'sha256': '1ff0bcb236c9f5473ad9bd40e8e0f86a93c8acd6ce1a067a1fd5be119424ebfc'}, 'libllama.so.0': {'bytes': 4618664, 'sha256': '034ab82df7a2e1e33f262bb391ceb87641706f31501e8c16947f88a69e9d0fec'}, 'libmtmd.so.0': {'bytes': 1920480, 'sha256': '895ac37bce53f9c90810f6858a00f340ff7c2804485be2b3ee387eca4c7f5072'}, 'llama-server': {'bytes': 17912, 'sha256': '04cc7c03a1d558ba7f71228b95104880a182178d6d79e3968475268e497b08eb'}}
LABELS='UNKNOWN SUBMIT USER_SAVE SEND RECEIVE SAVED_CHECK PERMISSION WORKER_START MODEL_START MODEL_RETURN REPLY STREAM_CLOSE RESPONSE_SAVE DISPLAY TOOL_START TOOL_RETURN STOP WORKER_EXIT RECOVERY MODEL_READY REFUSED NATIVE_FAILURE LOCAL_ROUTE LOCAL_START LOCAL_RETURN APPROVED_ROUTE UNKNOWN_STAGE'.split()
# Fixture labels are independently specified; native dx_stage tests verify every case.
CASES=json.loads(Path(__file__).with_name('cp12br5r1-stage-cases.json').read_text())
def cases_for(phase):
 return [(row['name'],row['summary']['complete'],row['summary']['incomplete']+row['summary']['unavailable'],[(s['stage'],s['observed'],s['bounded']) for s in row['stages']],row['expected']) for row in CASES if row['phase']==phase]
DEVELOPMENT=cases_for('development')
HOLDOUT=cases_for('holdout')
def prompt(case,kind):
 _,complete,missing,rows,_=case
 if kind=='baseline':
  r='\n'.join(f'{LABELS[i]}: {obs} observed stops/failures, {b} unconfirmed' for i,obs,b in rows)
  return f'Read the diagnostic facts. Answer with one label only.\nMissing or uncertain records: {missing}\nSuccessful deliveries: {complete}\nNumber of affected steps: {len(rows)}\nAffected steps:\n{r}\n\nRules in order:\n1. If missing or uncertain records is greater than zero, or a step is UNKNOWN_STAGE, answer UNCERTAIN.\n2. If number of affected steps is greater than one, answer MULTIPLE.\n3. If number of affected steps is one, copy the label before the colon on its line.\n4. If number of affected steps is zero and successful deliveries is greater than zero, answer DELIVERED.\n5. Otherwise answer EMPTY.\nWrite only the label. /no_think'
 if kind=='choice':
  names=', '.join(LABELS[i] for i,_,_ in rows) or 'none'
  options=', '.join(['UNCERTAIN','MULTIPLE','DELIVERED','EMPTY']+[LABELS[i] for i,_,_ in rows if i!=26])
  details='; '.join(f'{LABELS[i]} observed={o} unconfirmed={b}' for i,o,b in rows) or 'none'
  return f"Classify a diagnostic summary. Use the first applicable rule: missing records or UNKNOWN_STAGE means UNCERTAIN; multiple affected names means MULTIPLE; one affected name means that name; no affected names means DELIVERED if deliveries are positive, otherwise EMPTY.\nExamples:\nMissing=0, deliveries=1, affected=[DISPLAY] -> DISPLAY\nMissing=2, deliveries=1, affected=[DISPLAY] -> UNCERTAIN\nMissing=0, deliveries=2, affected=[] -> DELIVERED\nMissing=0, deliveries=0, affected=[] -> EMPTY\nMissing=0, deliveries=0, affected=[USER_SAVE, RECEIVE] -> MULTIPLE\n\nClassify this summary:\nMissing={missing}; deliveries={complete}; affected=[{names}]\nDetails: {details}\nChoose exactly one of: {options}.\nReply with only the chosen label. /no_think"
 names=', '.join(LABELS[i] for i,_,_ in rows) or '(none)'
 details='; '.join(f'{LABELS[i]} observed={o} unconfirmed={b}' for i,o,b in rows) or '(none)'
 return f'Choose one exact answer using the FIRST matching rule.\nMissing > 0 or UNKNOWN_STAGE listed: UNCERTAIN\nTwo or more affected names: MULTIPLE\nExactly one affected name: copy that name\nNo affected names and delivered > 0: DELIVERED\nNo affected names and delivered = 0: EMPTY\n\nMissing: {missing}\nDelivered: {complete}\nAffected names: {names}\nAffected count: {len(rows)}\nCounts: {details}\n\nAnswer with just the label, no explanation. /no_think'
def sha(p):
 with open(p,'rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--model',required=True,type=Path);p.add_argument('--model-sha256',required=True);p.add_argument('--out',required=True,type=Path);p.add_argument('--phase',choices=['development','holdout'],required=True);p.add_argument('--variants',nargs='+',choices=['baseline','lean','lean-no-thinking','choice'],default=['baseline','lean']);p.add_argument('--engine',type=Path,default=Path('/usr/lib/Lu/lu-inference/llama-server'));a=p.parse_args();assert os.geteuid()!=0;assert sha(a.model)==a.model_sha256;assert a.engine.name=='llama-server'
 for filename,pin in ENGINE_PINS.items():
  f=a.engine.parent/filename;assert f.is_file() and f.stat().st_size==pin['bytes'] and sha(f)==pin['sha256'],f'CPU runtime pin mismatch: {filename}'
 a.out.mkdir(parents=True,exist_ok=False)
 opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();key=secrets.token_hex(32);url=f'http://127.0.0.1:{port}';log=a.out/'engine.log';out=a.out/'results.json';rows=[]
 def request(path,data=None,timeout=45):return opener.open(urllib.request.Request(url+path,data=None if data is None else json.dumps(data).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'}),timeout=timeout)
 def save():out.write_text(json.dumps({'model_sha256':a.model_sha256,'model':a.model.name,'phase':a.phase,'engine_sha256':sha(a.engine),'fixture_sha256':sha(Path(__file__).with_name('cp12br5r1-stage-cases.json')),'scope':'standalone synthetic model comparison, not installed acceptance','rows':rows},indent=2)+'\n')
 with log.open('wb') as l:
  child=subprocess.Popen([str(a.engine),'--model',str(a.model),'--host','127.0.0.1','--port',str(port),'--api-key',key,'--threads','4','--ctx-size','2048','--parallel','1','--n-gpu-layers','0','--no-warmup'],stdout=l,stderr=l,env={**os.environ,'LD_LIBRARY_PATH':str(a.engine.parent)})
  try:
   deadline=time.monotonic()+90
   while True:
    if child.poll() is not None:raise RuntimeError('worker exited')
    try:
     with request('/health',timeout=2) as r:
      if r.status==200:break
    except Exception:
     if time.monotonic()>deadline:raise
     time.sleep(.3)
   cases=DEVELOPMENT if a.phase=='development' else HOLDOUT
   for case in cases:
    for variant in a.variants:
     start=time.monotonic();answer='';finish=None;error=None
     body={'messages':[{'role':'user','content':prompt(case,variant)}],'stream':True,'max_tokens':64,'temperature':0.0,'top_p':0.95,'top_k':20,'min_p':0.0,'cache_prompt':False}
     if variant=='lean-no-thinking':body.update(chat_template_kwargs={'enable_thinking':False},reasoning_effort='none')
     try:
      with request('/v1/chat/completions',body) as r:
       for line in r:
        if not line.startswith(b'data: '):continue
        payload=line[6:].strip()
        if payload==b'[DONE]':break
        v=json.loads(payload);choice=v['choices'][0];answer+=choice.get('delta',{}).get('content') or '';finish=choice.get('finish_reason') or finish
     except Exception as e:error=type(e).__name__
     # MATCH is exact trimmed visible content, not a keyword search or fuzzy match.
     match=not error and finish is not None and answer.strip()==case[-1]
     rows.append({'case':case[0],'variant':variant,'expected':case[-1],'answer':answer[:256].strip(),'match':match,'finish':finish,'error':error,'seconds':round(time.monotonic()-start,3)})
     save();print(case[0],variant,case[-1],repr(answer[:80].strip()),match,flush=True)
  finally:
   child.terminate()
   try:child.wait(10)
   except subprocess.TimeoutExpired:child.kill();child.wait()
   save()
if __name__=='__main__':main()
