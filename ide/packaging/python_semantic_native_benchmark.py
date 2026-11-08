#!/usr/bin/env python3
"""CI-only real Python LSP protocol benchmark: BasedPyright v1.40.2 vs ty 0.0.99.

This is research evidence, not a Lu Core/IDE executable permission or an
installed-app acceptance. Launch is restricted to named CI-provisioned
candidates in a disposable GitHub Actions runner. The frozen synthetic
fixtures have no project scripts, arbitrary imports or user-supplied code.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import os
from pathlib import Path
import queue
import statistics
import subprocess
import sys
import tempfile
import threading
import time

FIXTURE_COUNT=20
MAX_PACKET=2_000_000
TIMEOUT=18.0
SCHEMA="lu.ide.dev02.python-real-lsp-comparison.v1"


def fixtures(root:Path):
    root.mkdir(parents=True,exist_ok=True)
    sources=[]
    for index in range(FIXTURE_COUNT):
        name=f"calculate_{index:02d}"
        text=(
            f"def {name}(x: int) -> int:\n"
            "    return x + 1\n\n"
            f"answer = {name}(2)\n"
            "suggestion = calcula\n"
            f"missing = completely_unknown_{index:02d}\n"
        )
        path=root/f"case_{index:02d}.py"
        path.write_text(text,encoding="utf-8")
        sources.append((name,path,text))
    return sources


def framing(payload:dict)->bytes:
    raw=json.dumps(payload,separators=(",",":"),ensure_ascii=False).encode("utf-8")
    if len(raw)>MAX_PACKET:
        raise ValueError("LSP request exceeds bounded fixture")
    return b"Content-Length: "+str(len(raw)).encode("ascii")+b"\r\n\r\n"+raw


def read_packet(pipe):
    length=None
    count=0
    while count<16:
        line=pipe.readline(4096)
        if not line:
            raise EOFError("LSP provider stdout closed")
        count+=1
        if line==b"\r\n":
            break
        head=line.decode("ascii",errors="strict").strip()
        if head.lower().startswith("content-length:"):
            length=int(head.split(":",1)[1].strip())
    if length is None or length<1 or length>MAX_PACKET:
        raise ValueError("invalid LSP Content-Length")
    raw=pipe.read(length)
    if len(raw)!=length:
        raise EOFError("incomplete LSP response")
    message=json.loads(raw)
    if not isinstance(message,dict):
        raise ValueError("LSP response must be a JSON object")
    return message


class LspClient:
    def __init__(self, argv:list[str],cwd:Path):
        if not argv or any(not x for x in argv):
            raise ValueError("missing fixed CI candidate command")
        kwargs={"stdin":subprocess.PIPE,"stdout":subprocess.PIPE,
                "stderr":subprocess.PIPE,"cwd":str(cwd),"shell":False}
        if os.name=="nt":
            kwargs["creationflags"]=subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            kwargs["start_new_session"]=True
        self.proc=subprocess.Popen(argv,**kwargs)
        self.pending=queue.Queue(maxsize=1000)
        self.notifications=[]
        self.next_id=0
        self.stderr_lines=[]
        self.worker=threading.Thread(target=self._reader,daemon=True)
        self.worker.start()
        self.errors=threading.Thread(target=self._stderr,daemon=True)
        self.errors.start()

    def _reader(self):
        try:
            while True:
                item=read_packet(self.proc.stdout)
                self.pending.put(item,timeout=5)
        except (EOFError,ValueError,OSError,queue.Full) as error:
            try:
                self.pending.put({"__reader_error":type(error).__name__},timeout=1)
            except queue.Full:
                pass

    def _stderr(self):
        try:
            for line in iter(self.proc.stderr.readline,b""):
                if len(self.stderr_lines)<100:
                    self.stderr_lines.append(
                        line.decode("utf-8",errors="replace")[:300]
                    )
        except OSError:
            pass

    def send(self,message:dict):
        try:
            self.proc.stdin.write(framing(message))
            self.proc.stdin.flush()
        except (BrokenPipeError,OSError) as error:
            raise RuntimeError("real LSP provider stdin closed") from error

    def notify(self,method:str,params:dict):
        self.send({"jsonrpc":"2.0","method":method,"params":params})

    def _service_server_request(self,message):
        method=message.get("method")
        if method=="workspace/configuration":
            result=[{} for _ in message.get("params",{}).get("items",[])]
        elif method in ("client/registerCapability","client/unregisterCapability",
                        "window/workDoneProgress/create"):
            result=None
        else:
            self.send({"jsonrpc":"2.0","id":message["id"],
                       "error":{"code":-32601,"message":"Client method not supported"}})
            return
        self.send({"jsonrpc":"2.0","id":message["id"],"result":result})

    def request(self,method:str,params:dict,timeout:float=TIMEOUT):
        self.next_id+=1
        wanted=self.next_id
        self.send({"jsonrpc":"2.0","id":wanted,"method":method,"params":params})
        began=time.monotonic()
        deadline=began+timeout
        while time.monotonic()<deadline:
            try:
                message=self.pending.get(timeout=min(0.5,max(0.01,deadline-time.monotonic())))
            except queue.Empty:
                if self.proc.poll() is not None:
                    raise RuntimeError("real LSP provider exited mid-request")
                continue
            if "__reader_error" in message:
                raise RuntimeError("LSP packet reader terminated: "+message["__reader_error"])
            if "id" in message and "method" in message:
                self._service_server_request(message)
                continue
            if "method" in message and "id" not in message:
                if message["method"]=="textDocument/publishDiagnostics":
                    self.notifications.append(message)
                continue
            if message.get("id")==wanted:
                if "error" in message:
                    raise RuntimeError("LSP rejected "+method+" "+str(message["error"])[:240])
                return message.get("result"),(time.monotonic()-began)*1000
        raise TimeoutError("LSP real provider request timed out: "+method)

    def close(self):
        try:
            if self.proc.poll() is None:
                self.request("shutdown",{},timeout=5)
                self.notify("exit",{})
                self.proc.wait(timeout=5)
        except (OSError,RuntimeError,TimeoutError,subprocess.TimeoutExpired):
            pass
        if self.proc.poll() is None:
            if os.name=="nt":
                subprocess.run(["taskkill","/PID",str(self.proc.pid),"/T","/F"],
                    stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
                    timeout=12,check=False)
            else:
                import signal
                os.killpg(self.proc.pid,signal.SIGKILL)
            self.proc.wait(timeout=10)
        if self.proc.poll() is None:
            raise RuntimeError("candidate LSP was not reaped")
        if self.proc.returncode not in (0,None):
            # Some servers exit 1 after the LSP shutdown handshake; do not
            # misrepresent a partial or failed shutdown as production recovery.
            return False
        return True


def completions(result:list|dict|None,expected:str)->bool:
    items=result if isinstance(result,list) else result.get("items",[]) if isinstance(result,dict) else []
    return any(isinstance(x,dict) and x.get("label")==expected for x in items)


def definition_is_inside_source(result,uri:str)->bool:
    items=result if isinstance(result,list) else [result] if isinstance(result,dict) else []
    return any(isinstance(x,dict) and (
        x.get("uri")==uri or x.get("targetUri")==uri
    ) for x in items)


def hover_is_real(result)->bool:
    if not isinstance(result,dict):
        return False
    value=result.get("contents")
    if isinstance(value,str): return bool(value.strip())
    if isinstance(value,dict): return bool(str(value.get("value","")).strip())
    if isinstance(value,list): return bool(value)
    return False


def score(label:str,argv:list[str],project:Path,sources)->dict:
    run=LspClient(argv,project)
    data={"engine":label,"cases":FIXTURE_COUNT,"hover":0,"definition":0,
          "completion":0,"diagnosticFileCount":0,"nativeInitialized":False,
          "terminalReaped":False,"failures":[],"latencyMs":defaultdict(list)}
    try:
        rooturi=project.as_uri()
        initialized,_=run.request("initialize",{
            "processId":os.getpid(),"rootUri":rooturi,
            "rootPath":str(project),"clientInfo":{"name":"Lu IDE CI provider evaluation"},
            "workspaceFolders":[{"uri":rooturi,"name":"fixed-python-ci-fixtures"}],
            "capabilities":{
                "workspace":{"configuration":True,"workspaceFolders":True},
                "textDocument":{
                    "hover":{"contentFormat":["plaintext","markdown"]},
                    "completion":{"completionItem":{"snippetSupport":False}},
                    "definition":{"linkSupport":True},
                    "publishDiagnostics":{"relatedInformation":False}
                }
            },"initializationOptions":{}
        },timeout=30)
        if not isinstance(initialized,dict) or "capabilities" not in initialized:
            raise RuntimeError("provider did not initialize with capabilities")
        data["nativeInitialized"]=True
        run.notify("initialized",{})
        for name,path,text in sources:
            uri=path.as_uri()
            run.notify("textDocument/didOpen",{
                "textDocument":{"uri":uri,"languageId":"python",
                                "version":1,"text":text}
            })
            # Genuine source-specific requests in an actual provider, never
            # synthesized completions or language-server responses.
            for kind,method,position,verify in [
                ("hover","textDocument/hover",{"line":3,"character":11},
                 hover_is_real),
                ("definition","textDocument/definition",{"line":3,"character":11},
                 lambda v,uri=uri:definition_is_inside_source(v,uri)),
                ("completion","textDocument/completion",{"line":4,"character":20},
                 lambda v,name=name:completions(v,name)),
            ]:
                try:
                    value,ms=run.request(method,{
                        "textDocument":{"uri":uri},"position":position
                    })
                    data["latencyMs"][kind].append(round(ms,3))
                    if verify(value):
                        data[kind]+=1
                    else:
                        data["failures"].append(
                            f"{path.name}:{kind}:empty_or_mismatched_response"
                        )
                except (TimeoutError,RuntimeError,OSError) as error:
                    data["failures"].append(
                        f"{path.name}:{kind}:{type(error).__name__}"
                    )
            run.notify("textDocument/didClose",{
                "textDocument":{"uri":uri}
            })
        observed=set()
        for item in run.notifications:
            params=item.get("params",{})
            if isinstance(params,dict) and params.get("diagnostics"):
                observed.add(params.get("uri",""))
        data["diagnosticFileCount"]=len(observed)
    finally:
        data["terminalReaped"]=run.close()
    records={}
    for kind,values in data.pop("latencyMs").items():
        ordered=sorted(values)
        records[kind]={
            "n":len(ordered),
            "p50":round(statistics.median(ordered),3),
            "p95":ordered[min(len(ordered)-1,int(len(ordered)*0.95))],
        }
    data["latencyMs"]=records
    data["failures"]=data["failures"][:100]
    return data


def main()->int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--node",type=Path,required=True)
    parser.add_argument("--basedpyright",type=Path,required=True)
    parser.add_argument("--ty",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True)
    args=parser.parse_args()
    root=Path(os.environ.get("RUNNER_TEMP",""))
    if os.environ.get("GITHUB_ACTIONS")!="true" or not root.is_dir():
        print("DENIED: CI-only LSP candidate evaluation",file=sys.stderr)
        return 2
    if args.out.exists() or args.out.is_symlink():
        print("DENIED: immutable candidate result already exists",file=sys.stderr)
        return 2
    if not all(path.is_file() and not path.is_symlink() for path in
               [args.node,args.basedpyright,args.ty]):
        print("DENIED: reviewed candidate executable absent or linked",file=sys.stderr)
        return 2
    if not args.basedpyright.resolve().is_relative_to(root.resolve()) or not (
        args.ty.resolve().is_relative_to(root.resolve())
    ):
        print("DENIED: candidates must be isolated to disposable runner",file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory(prefix="lu-ide-python-eval-",dir=root) as folder:
        source=Path(folder)/"project"
        cases=fixtures(source)
        reports=[]
        for label,argv in [
            ("basedpyright-1.40.2",[str(args.node),str(args.basedpyright),"--stdio"]),
            ("ty-0.0.99",[str(args.ty),"server"]),
        ]:
            reports.append(score(label,argv,source,cases))
    result={"schema":SCHEMA,"releaseApproved":False,
            "testedInInstalledLu":False,"osProtectionAccepted":False,
            "externalSemanticsEnabled":False,
            "reports":reports}
    args.out.parent.mkdir(parents=True,exist_ok=True)
    with args.out.open("x",encoding="utf-8") as writer:
        json.dump(result,writer,indent=2,sort_keys=True)
        writer.write("\n")
    for record in reports:
        print(record["engine"]+": "+str({
            k:record[k] for k in ("nativeInitialized","hover","definition",
                                  "completion","diagnosticFileCount","terminalReaped")
        }),flush=True)
    if any((not rec["nativeInitialized"] or not rec["terminalReaped"] or
            rec["hover"]<18 or rec["definition"]<18 or rec["completion"]<10)
           for rec in reports):
        print("DENIED: real Python candidate acceptance thresholds not met",file=sys.stderr)
        return 1
    print("DEV02_REAL_PYTHON_PYRIGHT_COMPATIBLE_VS_TY_LSP_20_CASES=PASS")
    print("DEV02_INSTALLED_PROTECTED_PYTHON_SEMANTICS=NOT_APPROVED")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
