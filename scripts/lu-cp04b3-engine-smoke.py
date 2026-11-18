"""Independent real CPU GGUF smoke, not a public-search or native Lu acceptance."""
import argparse, hashlib, json, pathlib, secrets, socket, subprocess, tempfile, time, urllib.request

def check(path, expected):
    path = pathlib.Path(path).resolve(strict=True)
    with path.open('rb') as f:
        actual = hashlib.file_digest(f,'sha256').hexdigest()
    if actual != expected: raise ValueError(f'checksum mismatch: {path.name}')
    return path

def main():
    p=argparse.ArgumentParser()
    for name in ('engine','model','engine-sha256','model-sha256','evidence'): p.add_argument('--'+name, required=True)
    a=p.parse_args(); engine=check(a.engine,a.engine_sha256); model=check(a.model,a.model_sha256)
    with model.open('rb') as f:
        if f.read(4)!=b'GGUF': raise ValueError('not GGUF')
    with socket.socket() as s: s.bind(('127.0.0.1',0)); port=s.getsockname()[1]
    key=secrets.token_hex(24)
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def fetch(path, data=None, timeout=5):
        r=urllib.request.Request(f'http://127.0.0.1:{port}'+path, data=None if data is None else json.dumps(data).encode(), headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
        with opener.open(r,timeout=timeout) as reply: return json.load(reply)
    result={'schema':'lu-cp04b3-standalone-gguf-smoke-v1','engineSha256':a.engine_sha256,'modelSha256':a.model_sha256,'providerVerified':False,'protectedBrowserVerified':False,'generated':False}
    evidence=pathlib.Path(a.evidence)
    with tempfile.TemporaryFile() as log:
        proc=subprocess.Popen([str(engine),'--model',str(model),'--host','127.0.0.1','--port',str(port),'--api-key',key,'--ctx-size','8192','--threads','2','--n-gpu-layers','0','--no-warmup'],cwd=engine.parent,stdout=log,stderr=log)
        try:
            for _ in range(180):
                if proc.poll() is not None: raise RuntimeError('engine exited before readiness')
                try:
                    if fetch('/health').get('status')=='ok': break
                except (OSError,ValueError): pass
                time.sleep(1)
            else: raise RuntimeError('model readiness timeout')
            body=fetch('/v1/chat/completions',{'messages':[{'role':'user','content':'Briefly explain what Rust is, in one sentence. /no_think'}],'max_tokens':256,'temperature':0,'stream':False},180)
            answer=body['choices'][0]['message'].get('content') or ''
            if proc.poll() is not None or len(answer.split())<3 or body.get('usage',{}).get('completion_tokens',0)<1: raise RuntimeError('no substantive generated completion')
            result.update(generated=True,enginePid=proc.pid,answer=answer,usage=body['usage'],answerSha256=hashlib.sha256(answer.encode()).hexdigest())
        except Exception as exc:
            result['error']=str(exc); raise
        finally:
            if proc.poll() is None:
                proc.terminate()
                try: proc.wait(10)
                except subprocess.TimeoutExpired: proc.kill(); proc.wait(10)
            result['engineExited']=proc.poll() is not None
            log.seek(0); evidence.with_suffix('.engine.log').write_bytes(log.read(2_000_000).replace(key.encode(),b'[REDACTED]'))
            evidence.write_text(json.dumps(result,indent=2)+'\n',encoding='utf8')
    print('CP04B3_GENUINE_STANDALONE_GGUF_PASS',result['answerSha256'])
if __name__=='__main__': main()
