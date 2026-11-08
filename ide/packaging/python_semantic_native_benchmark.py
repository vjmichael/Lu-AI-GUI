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
import re
import statistics
import urllib.parse
import urllib.request
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

    def request(self,method:str,params:dict|None,timeout:float=TIMEOUT):
        self.next_id+=1
        wanted=self.next_id
        request={"jsonrpc":"2.0","id":wanted,"method":method}
        # The LSP shutdown request has no params. Ty strictly returns
        # InvalidParams (-32602) if a client sends an empty {} object.
        if params is not None:
            request["params"]=params
        self.send(request)
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

    def close(self) -> dict:
        # Record individual OS lifecycle evidence. A reaped process is NOT
        # necessarily a clean shutdown, and exit code 1 is not a process leak.
        outcome = {
            "shutdownAcknowledged": False, "exitNotificationSent": False,
            "forcedTermination": False, "reaped": False, "cleanExit": False,
            "exitCode": None, "shutdownError": None,
            "stderrClass": [],
        }
        if self.proc.poll() is None:
            try:
                self.request("shutdown", None, timeout=5)
                outcome["shutdownAcknowledged"] = True
            except (OSError, RuntimeError, TimeoutError) as error:
                outcome["shutdownError"] = type(error).__name__
                # No raw paths/project diagnostics from provider stderr.
                # Keep a bounded machine-readable class and numeric LSP code.
                cause = str(error)
                if cause.startswith("LSP rejected shutdown"):
                    outcome["shutdownFailureKind"] = "lsp_shutdown_rejected"
                    code = re.search(r"['\"]code['\"]:\s*(-?\d+)",cause)
                    outcome["shutdownLspCode"] = (int(code.group(1)) if code else None)
                elif "packet reader terminated" in cause:
                    outcome["shutdownFailureKind"] = "server_stdout_closed"
                elif "exited mid-request" in cause:
                    outcome["shutdownFailureKind"] = "server_exited_early"
                elif isinstance(error,TimeoutError):
                    outcome["shutdownFailureKind"] = "shutdown_response_timeout"
                else:
                    outcome["shutdownFailureKind"] = "other_protocol_error"
            if outcome["shutdownAcknowledged"]:
                try:
                    self.notify("exit", {})
                    outcome["exitNotificationSent"] = True
                except (OSError, RuntimeError) as error:
                    outcome["shutdownError"] = type(error).__name__
            if outcome["exitNotificationSent"]:
                try:
                    # ty's upstream actual-LSP benchmark closes client stdin
                    # after shutdown + exit before waiting for terminal status.
                    # EOF is not a waiver for nonzero exit or failed reaping.
                    self.proc.stdin.close()
                    self.proc.wait(timeout=5)
                except (OSError, BrokenPipeError) as error:
                    outcome["shutdownError"] = type(error).__name__
                except subprocess.TimeoutExpired:
                    outcome["shutdownError"] = "ExitWaitTimeout"
        if self.proc.poll() is None:
            outcome["forcedTermination"] = True
            if os.name == "nt":
                subprocess.run(
                    ["taskkill","/PID",str(self.proc.pid),"/T","/F"],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                    timeout=12, check=False,
                )
            else:
                import signal
                os.killpg(self.proc.pid, signal.SIGKILL)
            self.proc.wait(timeout=10)
        if self.proc.poll() is None:
            raise RuntimeError("candidate LSP process still alive after OS cleanup")
        outcome["exitCode"] = self.proc.returncode
        outcome["reaped"] = True
        outcome["cleanExit"] = (
            outcome["shutdownAcknowledged"] and
            outcome["exitNotificationSent"] and
            not outcome["forcedTermination"] and self.proc.returncode == 0
        )
        # Only bounded error classes; do not embed paths, source text, PIDs
        # or arbitrary provider stderr into public benchmark receipts.
        for line in self.stderr_lines[-8:]:
            lower = line.lower()
            for term in ("panic", "error", "warning", "shutdown", "exit", "closed"):
                if term in lower and term not in outcome["stderrClass"]:
                    outcome["stderrClass"].append(term)
        return outcome


def completions(result:list|dict|None,expected:str)->bool:
    items=result if isinstance(result,list) else result.get("items",[]) if isinstance(result,dict) else []
    return any(isinstance(x,dict) and x.get("label")==expected for x in items)


def same_local_uri(candidate: str, expected: str) -> bool:
    """Only the identical native fixture file, without query/fragment/remote host.

    Python's original URL text comparison is too strict on Windows, where
    LSP providers may normalize drive-letter casing and percent encoding.
    """
    if not isinstance(candidate, str) or not isinstance(expected, str):
        return False
    try:
        left, right = urllib.parse.urlsplit(candidate), urllib.parse.urlsplit(expected)
        if any(uri.scheme.lower() != "file" or uri.netloc not in ("", "localhost")
               or uri.query or uri.fragment for uri in (left, right)):
            return False
        a = urllib.request.url2pathname(left.path)
        b = urllib.request.url2pathname(right.path)
        if not os.path.isabs(a) or not os.path.isabs(b):
            return False
        # On Windows, distinct valid file: spellings (drive casing, short
        # names) can resolve to the SAME NTFS file while string realpath
        # comparisons disagree. An OS inode/file-ID test is stricter than
        # casefolding untrusted paths: both files must actually exist.
        return os.path.samefile(a,b)
    except (OSError, ValueError):
        return False


def definition_is_inside_source(result,uri:str)->bool:
    items=result if isinstance(result,list) else [result] if isinstance(result,dict) else []
    return any(isinstance(x,dict) and (
        same_local_uri(x.get("uri"),uri) or
        same_local_uri(x.get("targetUri"),uri)
    ) for x in items)


def definition_response_class(result, expected_uri: str|None=None) -> str:
    """Non-sensitive shape and OS existence facts on a failed fixture URI.

    Never print a raw provider path or user-provided URI. A correct native
    same-file decision still requires the actual file identity check.
    """
    items=result if isinstance(result,list) else [result] if isinstance(result,dict) else []
    if not items: return "null_or_empty"
    obj=items[0]
    if not isinstance(obj,dict): return "non_object"
    uri=obj.get("targetUri") or obj.get("uri")
    if not isinstance(uri,str): return "no_location_uri"
    try:
        parsed=urllib.parse.urlsplit(uri)
        facts=""
        if isinstance(expected_uri,str):
            other=urllib.parse.urlsplit(expected_uri)
            local=urllib.request.url2pathname(parsed.path)
            registered=urllib.request.url2pathname(other.path)
            actual=os.path.exists(local)
            expected_exists=os.path.exists(registered)
            # Distinguish a genuinely different target from a Windows
            # file-URL spelling/conversion bug. These are booleans only.
            facts=("_candidate_exists_"+str(int(actual))+
                   "_registered_exists_"+str(int(expected_exists))+
                   "_same_basename_"+str(int(
                       os.path.basename(local).casefold()==
                       os.path.basename(registered).casefold()))+
                   "_same_text_casefold_"+str(int(
                       os.path.normcase(local)==os.path.normcase(registered)))+
                   "_candidate_absolute_"+str(int(os.path.isabs(local)))+
                   "_expected_absolute_"+str(int(os.path.isabs(registered))))
        return ("scheme_"+parsed.scheme[:12]+
            ("_remote" if parsed.netloc not in ("","localhost") else "_local")+
            ("_file_extension_py" if parsed.path.lower().endswith(".py") else "_other_extension")+
            facts)
    except ValueError:
        return "malformed_location_uri"


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
          "terminalReaped":False,"terminalClean":False,"termination":None,
          "failures":[],"latencyMs":defaultdict(list)}
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
                        reason=(definition_response_class(value,uri)
                                if kind=="definition" else "empty_or_mismatched_response")
                        data["failures"].append(f"{path.name}:{kind}:{reason}")
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
        lifecycle=run.close()
        data["termination"]=lifecycle
        data["terminalReaped"]=lifecycle["reaped"]
        data["terminalClean"]=lifecycle["cleanExit"]
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
                                  "completion","diagnosticFileCount",
                                  "terminalReaped","terminalClean","termination")
        }),flush=True)
    if any((not rec["nativeInitialized"] or not rec["terminalReaped"] or
            not rec["terminalClean"] or rec["hover"]<18 or
            rec["definition"]<18 or rec["completion"]<10)
           for rec in reports):
        print("DENIED: real Python candidate acceptance thresholds not met",file=sys.stderr)
        return 1
    print("DEV02_REAL_PYTHON_PYRIGHT_COMPATIBLE_VS_TY_LSP_20_CASES=PASS")
    print("DEV02_INSTALLED_PROTECTED_PYTHON_SEMANTICS=NOT_APPROVED")
    return 0


if __name__=="__main__":
    raise SystemExit(main())
