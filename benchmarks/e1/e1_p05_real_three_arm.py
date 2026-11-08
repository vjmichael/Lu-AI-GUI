#!/usr/bin/env python3
"""E1-P05 same-task direct-model / Lu utility / reviewed app evidence pilot.

Native arms launch actual compiled Rust executables in new disposable roots.
The model arm runs a genuine local SHA-pinned Qwen GGUF through a locally
started SHA-pinned llama.cpp build. No output is filled in from estimates.
The text-only model can answer a specification but cannot be credited with a
real DOCX/XLSX artifact unless one independently exists (none is created).
"""
import argparse
import hashlib
import json
import pathlib
import random
import statistics
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from e1_p05_xml_oracle import check_ooxml_exact

MODEL_SHA="7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d"
SEED=20261107
KINDS=("docx","xlsx")
ARMS=("direct_model","native_utility","reviewed_app")
EXPECTED={
    "docx":{
        "title":"Create a genuine .docx with the paragraph 'Lu E1 artifact custody verified'",
        "required":["Lu E1 artifact custody verified"],
    },
    "xlsx":{
        "title":"Create a genuine Excel .xlsx sheet E1, A1 Status, B1 Reviewed, A2 Frames, B2 48",
        "required":["E1","Status","Reviewed","Frames","48"],
    },
}
SCHEMA="lu-e1-p05-real-three-arm-v1"
def digest(path):
    h=hashlib.sha256()
    with open(path,"rb") as f:
        for part in iter(lambda:f.read(1024*1024),b""):
            h.update(part)
    return h.hexdigest()
def check_binary(path,label):
    if not path or not path.is_file():
        raise RuntimeError("Missing actual binary: "+label)
    return digest(path)
def native_trial(binary,worker,kind,mode,root):
    started=time.perf_counter()
    p=subprocess.run(
        [str(binary),mode,kind,str(root),str(worker)],
        text=True,capture_output=True,timeout=75,
    )
    elapsed=(time.perf_counter()-started)*1000
    if p.returncode:
        raise RuntimeError(f"Native {mode}/{kind} failed, code={p.returncode}, "
                           f"stderr={p.stderr[-2000:]!r}")
    try:
        row=json.loads(p.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("Native runner emitted no trustworthy JSON") from error
    if row.get("schema")!="lu-e1-p05-native-real-arm-v1":
        raise RuntimeError("Wrong actual native benchmark schema")
    if row.get("mode")!=mode or row.get("kind")!=kind:
        raise RuntimeError("Task/arm identity mismatch")
    if row.get("modelUsed") is not False or row.get("actualThirdPartyApp") is not False:
        raise RuntimeError("Native reporter invented model/third-party application")
    target=root/("result.docx" if kind=="docx" else "result.xlsx")
    if bool(target.is_file())!=row.get("artifactCreated"):
        raise RuntimeError("No independently observed native artifact")
    if target.exists():
        if digest(target)!=row.get("artifactSha256") or (
            target.stat().st_size!=row.get("artifactBytes")
        ):
            raise RuntimeError("Actual published bytes disagree with native result")
        # Cross-language independent semantic witness for exact task values,
        # in addition to the native Rust document/worksheet reader.
        check_ooxml_exact(target,kind)
    supported=(mode=="reviewed_app" or kind=="xlsx")
    expected_attempts=1 if supported else 0
    if row.get("supported") is not supported or row.get("journalBacked") is not supported:
        raise RuntimeError("Native capability/receipt coverage differs from sealed task")
    if (row.get("unauthorizedAttempts")!=expected_attempts or
        row.get("unauthorizedAttemptsBlocked")!=expected_attempts):
        raise RuntimeError("Native effect-veto attempt missing or unblocked")
    row["harnessWallMs"]=round(elapsed,2)
    row["source"]="real-compiled-lu-core-native"
    row["taskSuccess"]=bool(row["artifactCreated"] and row["semanticAccuracy"])
    row["independentPythonZipWitness"]=target.is_file()
    row["independentPythonSemanticWitness"]=target.is_file()
    row["rootDisposition"]="disposable-workspace"
    return row
def server(engine,model,port,log):
    return subprocess.Popen([
        str(engine),"-m",str(model),"--alias","lu-e1-p05",
        "--host","127.0.0.1","--port",str(port),
        "--ctx-size","2048","--threads","2","--parallel","1",
        "--n-gpu-layers","0",
    ],stdout=log,stderr=subprocess.STDOUT)
def ready(proc,base):
    for _ in range(120):
        if proc.poll() is not None:
            raise RuntimeError("Actual local model process exited")
        try:
            with urllib.request.urlopen(base+"/health",timeout=2) as response:
                if response.status==200:return
        except (TimeoutError,OSError,urllib.error.URLError):
            pass
        time.sleep(.5)
    raise RuntimeError("Real Qwen server never became healthy")
def model_trial(base,kind,trial):
    prompt=(
        "You cannot access tools or the filesystem. The user wants this "
        "exact document task. Return one plain text line describing the "
        "required contents, no fabricated file links or success claims. "
        "Do not claim you created a file.\nTask: "+EXPECTED[kind]["title"]
    )
    request=urllib.request.Request(
        base+"/v1/chat/completions",method="POST",
        headers={"Content-Type":"application/json"},
        data=json.dumps({
            "model":"lu-e1-p05",
            "messages":[
                {"role":"system","content":"No tools. Answer the task text only."},
                {"role":"user","content":prompt},
            ],
            "temperature":0.0,"top_p":1.0,
            "seed":SEED+trial,"max_tokens":120,"stream":False,
        }).encode(),
    )
    started=time.perf_counter()
    with urllib.request.urlopen(request,timeout=180) as response:
        result=json.loads(response.read(1024*1024))
    elapsed=(time.perf_counter()-started)*1000
    text=result["choices"][0]["message"].get("content") or ""
    knowledge=all(token.casefold() in text.casefold()
                  for token in EXPECTED[kind]["required"])
    return {
        "schema":"lu-e1-p05-native-real-arm-v1",
        "mode":"direct_model","kind":kind,
        "source":"actual-local-qwen-gguf-chat",
        "latencyMs":round(elapsed,2),
        "setupMs":None,
        "modelRawText":text,
        "modelUsage":result.get("usage"),
        "knowledgeSpecAccurate":knowledge,
        "supported":True,
        "artifactCreated":False,
        "artifactBytes":0,
        "artifactSha256":None,
        "semanticAccuracy":False,
        "taskSuccess":False,
        "independentPythonZipWitness":False,
        "unauthorizedAttempts":0,
        "unauthorizedAttemptsBlocked":0,
        "modelUsed":True,
        "actualThirdPartyApp":False,
    }
def aggregate(rows,model_present):
    out={}
    for arm in ARMS:
        subset=[r for r in rows if r["mode"]==arm]
        if not subset:
            out[arm]={"state":"NOT_EXECUTED","trials":0,"taskSuccessRate":None,
                      "p50Ms":None,"p95Ms":None}
            continue
        observed=[float(r["latencyMs"]) for r in subset]
        out[arm]={
            "state":"GENUINE_NATIVE" if arm!="direct_model" else "GENUINE_LOCAL_GGUF",
            "trials":len(subset),
            "taskSuccessRate":sum(r["taskSuccess"] for r in subset)/len(subset),
            "byFormat":{kind:{
                "success":sum(r["taskSuccess"] for r in subset if r["kind"]==kind),
                "total":sum(r["kind"]==kind for r in subset),
            }for kind in KINDS},
            "p50Ms":round(statistics.median(observed),2),
            "p95Ms":round(sorted(observed)[int((len(observed)-1)*.95)],2),
            "unauthorizedAttempts":sum(r["unauthorizedAttempts"] for r in subset),
            "unauthorizedAttemptsBlocked":sum(
                r["unauthorizedAttemptsBlocked"] for r in subset
            ),
            "modelSpecAccuracy":(
                sum(r["knowledgeSpecAccurate"] for r in subset)/len(subset)
                if arm=="direct_model" else None
            ),
        }
    return out
def run(args):
    if args.repeats not in (1,2,3):raise RuntimeError("Only 1–3 trials allowed")
    if args.output_dir.exists():raise RuntimeError("Evidence output exists")
    native_hash=check_binary(args.native,"real Lu Core harness")
    worker_hash=check_binary(args.worker,"real Lu reviewed artifact worker")
    real_model=(args.model is not None or args.engine is not None)
    if real_model and (args.model is None or args.engine is None):
        raise RuntimeError("Both genuine model and local llama server required")
    model_hash=engine_hash=None
    if real_model:
        model_hash=check_binary(args.model,"genuine GGUF")
        engine_hash=check_binary(args.engine,"genuine llama server")
        if model_hash!=MODEL_SHA:raise RuntimeError("Pinned Qwen GGUF SHA mismatch")
    args.output_dir.mkdir(parents=True)
    rows=[]
    proc=None
    try:
        if real_model:
            log=(args.output_dir/"llama-server.log").open("wb")
            proc=server(args.engine,args.model,args.port,log)
            ready(proc,f"http://127.0.0.1:{args.port}")
        order=[(trial,kind,arm) for trial in range(args.repeats)
               for kind in KINDS
               for arm in (ARMS if real_model else ARMS[1:])]
        random.Random(SEED).shuffle(order)
        for trial,kind,arm in order:
            if arm=="direct_model":
                row=model_trial(f"http://127.0.0.1:{args.port}",kind,trial)
            else:
                target=args.output_dir/f"trial-{trial}-{kind}-{arm}"
                row=native_trial(args.native,args.worker,kind,arm,target)
            row["trial"]=trial
            rows.append(row)
    finally:
        if proc is not None:
            proc.terminate()
            try:proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                proc.kill();proc.wait()
            log.close()
    out={
        "schema":SCHEMA,
        "sourceCommit":args.source_commit,
        "modelSha256":model_hash,
        "engineSha256":engine_hash,
        "nativeSha256":native_hash,
        "workerSha256":worker_hash,
        "trialCount":len(rows),
        "results":rows,
        "summary":aggregate(rows,real_model),
        "status":("GENUINE_THREE_ARM_HEADLESS" if real_model
                  else "NATIVE_ARMS_ONLY_MODEL_NOT_EXECUTED"),
        "limitations":{
            "directModelCannotWriteArtifacts":True,
            "nativeUtilityDocxCreationNotImplemented":True,
            "appArmIsReviewedFirstPartyFixtureNotThirdParty":True,
            "modelAndNativeTasksDifferInOutputMedium":True,
            "runtimeArtifactProducerAttributionOnlyForFirstPartyPipe":True,
            "installedPhysicalWindowsAccepted":False,
            "autonomousToolSelectionTested":False,
            "untrustedThirdPartyAppSandboxAccepted":False,
            "modelInferenceExecuted":real_model,
        }
    }
    (args.output_dir/"e1_p05_raw.json").write_text(
        json.dumps(out,indent=2)+"\n",encoding="utf-8")
    summary=["# E1-P05 Genuine OS/runtime measured arms",
             f"**Status:** {out['status']}",
             "| Arm | Real trials | Task artifact success | p50 ms | Host-veto attempts blocked |",
             "|---|---:|---:|---:|---:|"]
    for arm,stats in out["summary"].items():
        success=stats["taskSuccessRate"]
        summary.append(
            f"| {arm} | {stats['trials']} | "
            f"{'NOT MEASURED' if success is None else f'{100*success:.1f}%'} | "
            f"{stats['p50Ms'] if stats['p50Ms'] is not None else 'N/A'} | "
            f"{stats.get('unauthorizedAttemptsBlocked','N/A')} |"
        )
    summary+=["","These are fixed, independently graded disposable tasks. The "
              "model arm has no file access, the Lu utility lacks DOCX output, "
              "and the reviewed app is a first-party fixture. Do not treat "
              "these as general-app superiority or installed v1.0 proof."]
    (args.output_dir/"e1_p05_summary.md").write_text(
        "\n".join(summary)+"\n",encoding="utf-8")
    print(json.dumps({"status":out["status"],"summary":out["summary"]}))
def main():
    p=argparse.ArgumentParser()
    p.add_argument("--native",required=True,type=pathlib.Path)
    p.add_argument("--worker",required=True,type=pathlib.Path)
    p.add_argument("--model",type=pathlib.Path)
    p.add_argument("--engine",type=pathlib.Path)
    p.add_argument("--output-dir",required=True,type=pathlib.Path)
    p.add_argument("--source-commit",default="")
    p.add_argument("--repeats",type=int,default=2)
    p.add_argument("--port",type=int,default=18137)
    run(p.parse_args())
if __name__=="__main__":main()
