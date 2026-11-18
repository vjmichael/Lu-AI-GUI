#!/usr/bin/env python3
"""M5-05 evidence gate: report ONLY observed headless/installed measurements.

Evidence files are untrusted until independently correlated to immutable
GitHub job/artifact or reviewed Windows hardware. This program never declares
a release accepted solely because a JSON file contains PASS strings.
"""
import argparse
import hashlib
import json
import pathlib
import statistics
import re

SCHEMA="lu-m5-comparative-pilot-v1"
REQUIRED=["m5-01-native-linux","m5-01-native-windows",
          "m5-02-real-local-model","m5-03-installed-Windows-GGUF",
          "m5-04-native-authority","m5-05-installed-NSIS-Vulkan"]
KNOWN_ARMS=("no_memory","original_ansf")
class InvalidEvidence(ValueError):pass
def sha256(path):
    digest=hashlib.sha256()
    with path.open("rb") as fh:
        for data in iter(lambda:fh.read(1024*1024),b""):
            digest.update(data)
    return digest.hexdigest()
def load_json(path):
    path=pathlib.Path(path)
    if not path.is_file():raise InvalidEvidence("Missing evidence file")
    if path.stat().st_size>4*1024*1024:raise InvalidEvidence("Oversized evidence")
    try:
        return json.loads(path.read_text(encoding="utf-8-sig"))
    except (UnicodeError,json.JSONDecodeError) as error:
        raise InvalidEvidence("Invalid JSON bytes: "+str(error)) from error
def safe_number(value):
    return isinstance(value,(float,int)) and not isinstance(value,bool) and 0<=value<=1
def normalized_text(value):
    if not isinstance(value,str):raise InvalidEvidence("Missing raw text")
    # Match the actual M5 model runner's strict single-line ANSWER contract.
    plain=re.sub(r"<think>.*?</think>","",value,flags=re.S|re.I).strip()
    if "\n" in plain:return "__MULTILINE__"
    m=re.fullmatch(r"(?i)ANSWER:\s*(.{1,100})",plain)
    if not m:return "__INVALID__"
    answer=m.group(1).strip().strip(chr(34)+chr(39)+" .!")
    if not answer or "<" in answer or ">" in answer or ";" in answer:
        return "__INVALID__"
    return " ".join(answer.lower().split())
def analyze_model(payload):
    if payload.get("schema")!="lu-m5-repeated-task-real-gguf-v1":
        raise InvalidEvidence("Real M5 Qwen result schema missing")
    if payload.get("installed_windows_acceptance") is not False or (
        payload.get("actual_authorized_tool_effects_tested") is not False
    ):raise InvalidEvidence("Headless result claims unsupported installed/tool effect")
    rows=payload.get("rows")
    if not isinstance(rows,list) or not rows or len(rows)>300:
        raise InvalidEvidence("Real model row count absent or invalid")
    seen=set()
    groups={}
    for row in rows:
        if not isinstance(row,dict) or row.get("arm") not in KNOWN_ARMS:
            raise InvalidEvidence("Unexpected model arm")
        if not isinstance(row.get("trial"),int) or row["trial"]<0 or row["trial"]>5:
            raise InvalidEvidence("Invalid trial number")
        case=row.get("case")
        if not isinstance(case,str) or not case or len(case)>100:
            raise InvalidEvidence("Invalid case identity")
        key=(case,row["trial"],row["arm"])
        if key in seen:raise InvalidEvidence("Duplicated answer/trial/arm")
        seen.add(key)
        if row.get("correct") is not True and row.get("correct") is not False:
            raise InvalidEvidence("Missing actual model correctness")
        if not isinstance(row.get("model_text"),str) or not isinstance(
            row.get("expected"),str
        ):raise InvalidEvidence("Missing raw model generation or expected answer")
        actual_correct=(normalized_text(row["model_text"])==
                        normalized_text("ANSWER: "+row["expected"]))
        if actual_correct!=row["correct"]:
            raise InvalidEvidence("Model row correctness disagrees with actual raw text")
        for number in ("latency_ms","retrieval_ms"):
            value=row.get(number)
            if not isinstance(value,(float,int)) or isinstance(value,bool) or value<0:
                raise InvalidEvidence("Missing genuine timing")
        if row.get("known") is not True and row.get("known") is not False:
            raise InvalidEvidence("Missing known/unknown ground truth flag")
        if row["arm"]=="no_memory" and (
            row.get("source_id") is not None or row["retrieval_ms"]!=0
        ):raise InvalidEvidence("Control arm received historical source")
        if row["arm"]=="original_ansf" and (
            not isinstance(row.get("source_id"),str) or
            not row["source_id"].startswith("ansf:v1:")
        ):raise InvalidEvidence("Memory arm is missing canonical source ID")
        groups.setdefault((case,row["trial"]),set()).add(row["arm"])
    if any(arms!=set(KNOWN_ARMS) for arms in groups.values()):
        raise InvalidEvidence("Not every task has both paired model arms")
    metrics={}
    for arm in KNOWN_ARMS:
        subset=[r for r in rows if r["arm"]==arm]
        known=[r for r in subset if r["known"]]
        unknown=[r for r in subset if not r["known"]]
        if not known or not unknown:
            raise InvalidEvidence("Must include known and unknown cases")
        latencies=[r["latency_ms"] for r in subset]
        lookup=[r["retrieval_ms"] for r in subset]
        metrics[arm]={
            "total":len(subset),
            "correct":sum(r["correct"] for r in subset),
            "accuracy":sum(r["correct"] for r in subset)/len(subset),
            "known_accuracy":sum(r["correct"] for r in known)/len(known),
            "unknown_safety":sum(r["correct"] for r in unknown)/len(unknown),
            "p50_model_ms":statistics.median(latencies),
            "p95_model_ms":sorted(latencies)[int((len(latencies)-1)*.95)],
            "p50_lookup_ms":statistics.median(lookup),
            "p95_lookup_ms":sorted(lookup)[int((len(lookup)-1)*.95)],
            "avg_prompt_tokens":statistics.mean(
                [r["prompt_tokens"] for r in subset if isinstance(
                    r.get("prompt_tokens"),int)]
            ) if any(isinstance(r.get("prompt_tokens"),int) for r in subset) else None
        }
    old=metrics["no_memory"];new=metrics["original_ansf"]
    if payload.get("model_sha256") is None or not isinstance(
        payload.get("engine_sha256"),str
    ):raise InvalidEvidence("Missing real model/engine fingerprint")
    return {
        "raw_rows":len(rows),
        "model_sha256":payload["model_sha256"],
        "engine_sha256":payload["engine_sha256"],
        "metrics":metrics,
        "known_accuracy_gain":new["known_accuracy"]-old["known_accuracy"],
        "full_accuracy_gain":new["accuracy"]-old["accuracy"],
        "model_p50_latency_delta_ms":new["p50_model_ms"]-old["p50_model_ms"],
        "lookup_p50_latency_ms":new["p50_lookup_ms"],
        "headless_benefit_candidate":(
            new["known_accuracy"]>old["known_accuracy"] and
            new["unknown_safety"]==1
        ),
        "does_not_test":"installed Windows, autonomous source discovery, real external effect or GPU offload"
    }
def analyze_physical(payload):
    if payload.get("schema")!="lu-m5-installed-local-model-acceptance-v1":
        raise InvalidEvidence("Unexpected installed Windows evidence schema")
    models=payload.get("modelSessions")
    if not isinstance(models,list) or len(models)!=3:
        raise InvalidEvidence("Missing primary/restart/replacement model sessions")
    phases=[x.get("phase") for x in models]
    if phases!=["primary-before-restart","same-primary-after-restart",
                "replacement-after-model-swap"]:
        raise InvalidEvidence("Windows model/restart phases mismatched")
    hashes=payload.get("hashes") or {}
    if not all(isinstance(hashes.get(k),str) and len(hashes[k])==64
      for k in ("installedExe","engine","firstModel","replacementModel")):
        raise InvalidEvidence("Missing reviewed installed byte identities")
    os_results=[]
    reported_results=[]
    for sess in models:
        win=sess.get("windowObservation") or {}
        if not isinstance(win.get("pid"),int) or win.get("pid")!=win.get("uiAProcessId"):
            raise InvalidEvidence("Missing actual UIA process ownership")
        os_results.append(sess.get("osEngineObserved")=="OS_REVIEWED_PROCESS_SEEN" and
              sess.get("osEngineReaped")=="OS_PRIOR_WORKERS_NOT_PRESENT")
        scenarios=sess.get("scenarios") or {}
        reported_results.append(bool(scenarios) and all(
            x=="OPERATOR_REPORTED_PASS" for x in scenarios.values()))
    # This is deliberately an evidence-readiness signal, NOT automatic acceptance.
    return {
        "sessions":3,
        "all_os_process_observations":all(os_results),
        "all_operator_reported_pass":all(reported_results),
        "source_identity_reported":all(
            s.get("reportedSourceId","").startswith("ansf:v1:") for s in models),
        "reported_first_response_ms":None,
        "actual_model_accuracy_independently_verified":False,
        "gpu_offload_independently_verified":False,
        "operator_statements_are_not_automatic_machine_proof":True,
    }
def build(inputs):
    gates={name:{"state":"NOT_SUBMITTED","source_sha256":None,
                 "finding":None} for name in REQUIRED}
    measured={}
    errors=[]
    names={
        "native_linux":"m5-01-native-linux",
        "native_windows":"m5-01-native-windows",
        "model":"m5-02-real-local-model",
        "physical":"m5-03-installed-Windows-GGUF",
        "authority":"m5-04-native-authority",
        "install":"m5-05-installed-NSIS-Vulkan",
    }
    for key,gate in names.items():
        value=inputs.get(key)
        if not value:continue
        path=pathlib.Path(value)
        gates[gate]["source_sha256"]=sha256(path) if path.is_file() else None
        try:
            content=load_json(path)
            if key=="model":
                measured["paired_real_GGUF"]=analyze_model(content)
            elif key=="physical":
                measured["physical_Windows"]=analyze_physical(content)
            elif key in ("native_linux","native_windows","authority","install"):
                if content.get("schema")!="lu-m5-ci-native-evidence-v1" or (
                    content.get("status") not in ("passed","failed")
                ):
                    raise InvalidEvidence("Only actual CI native evidence schema accepted")
                if content.get("source")!="github-actions-native" or not isinstance(
                    content.get("run_id"),int
                ) or not isinstance(content.get("commit_sha"),str):
                    raise InvalidEvidence("Missing genuine external run identifiers")
                measured[gate]={
                    "observed_status":content["status"],"run_id":content["run_id"],
                    "commit_sha":content["commit_sha"],
                    "note":"Self-reported job metadata still requires independent GitHub verification"
                }
            gates[gate]["state"]="SUBMITTED_NOT_INDEPENDENTLY_ATTESTED"
        except (InvalidEvidence,OSError,TypeError,AttributeError) as error:
            gates[gate]["state"]="INVALID_OR_INCOMPLETE"
            gates[gate]["finding"]=str(error)
            errors.append({"gate":gate,"error":str(error)})
    return {
        "schema":SCHEMA,
        "disposition":"OPEN_NEEDS_INDEPENDENT_NATIVE_MODEL_AND_INSTALLED_WITNESSES",
        "source_provenance":"user-provided-input-bytes-never-auto-release-authority",
        "gates":gates,
        "measured":measured,
        "missing":[k for k,v in gates.items() if v["state"]=="NOT_SUBMITTED"],
        "errors":errors,
        "not_measured":{
            "actual_authorized_external_effect_calls":None,
            "human_interruption_minutes":None,
            "new_question_interruptions":None,
            "rediscovery_tool_calls":None,
            "installed_first_token_latency_ms":None,
            "installed_model_swap_time_ms":None,
            "installed_native_worker_GPU_offload":None
        },
        "release_accepted":False
    }
def run():
    parser=argparse.ArgumentParser()
    parser.add_argument("--model",type=pathlib.Path)
    parser.add_argument("--physical",type=pathlib.Path)
    parser.add_argument("--native-linux",type=pathlib.Path)
    parser.add_argument("--native-windows",type=pathlib.Path)
    parser.add_argument("--authority",type=pathlib.Path)
    parser.add_argument("--install",type=pathlib.Path)
    parser.add_argument("--output",type=pathlib.Path,required=True)
    args=parser.parse_args()
    out=args.output
    if out.exists():raise SystemExit("Refusing to overwrite previous acceptance report")
    report=build({
        "model":args.model,"physical":args.physical,
        "native_linux":args.native_linux,"native_windows":args.native_windows,
        "authority":args.authority,"install":args.install
    })
    out.parent.mkdir(parents=True,exist_ok=True)
    out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"disposition":report["disposition"],"missing":report["missing"],
                      "errors":report["errors"]},indent=2))
    return 0 if not report["errors"] else 2
if __name__=="__main__":
    raise SystemExit(run())
