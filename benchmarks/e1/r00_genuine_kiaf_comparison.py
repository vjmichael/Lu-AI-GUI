#!/usr/bin/env python3
"""Audit real Lu Qwen comprehension protocols; never invent or promote authority.

Takes raw and schema-constrained SAME GGUF/SHA, SAME host corpora, same cases
and repetition counts. This is K/I/A/F diagnostic, not numerical capability
f(K,I,A,F), not installed app, and never a model-authored tool grant.
"""
import argparse
import hashlib
import json
from pathlib import Path

EXPECTED_SCHEMA="lu-memory-loop03c-real-model-comparison-v1"
DIMENSIONS={
    "K":("knowledge_source_status","knowledge_human_review","knowledge_historical_verification"),
    "I":("model_interface",),
    "A":("authority_no_reuse_or_retry",),
    "F":("feedback_no_current_freshness","feedback_no_os_custody_inference"),
}

def require(condition,reason):
    if not condition: raise ValueError(reason)

def verified_report(path,protocol):
    blob=path.read_bytes()
    report=json.loads(blob)
    require(report.get("schema")==EXPECTED_SCHEMA,"Wrong model report schema")
    require(report.get("responseProtocol")==protocol,"Wrong protocol identity")
    require(report.get("modelActuallyLoaded") is True,"Missing actually loaded GGUF")
    require(report.get("fatalError") is None,"Fatal benchmark environment error")
    require(report.get("installedLu") is False and report.get("realOsCustodyProven") is False,
            "Benchmark must not counterfeit installed kernel acceptance")
    samples=report.get("samples")
    require(isinstance(samples,list) and len(samples)>0,"Missing model samples")
    require(report.get("observedModelGenerations")==len(samples),"Missing generation")
    require(report.get("modelSha256") and len(report["modelSha256"])==64,"Unpinned model identity")
    identities=set()
    for sample in samples:
        key=(sample.get("arm"),sample.get("caseId"),sample.get("repetition"))
        require(key not in identities and key[0] in ("source_only","host_classified"),
                "Duplicate or missing arm identity")
        identities.add(key)
        require(sample.get("classification")!="MODEL_TRANSPORT_FAILURE","Incomplete model transport")
        require(isinstance(sample.get("rawResponse"),str)
                and isinstance(sample.get("rawSha256"),str),"Missing actual raw model output")
        raw=sample["rawResponse"].encode("utf8")
        # Benchmark truncates raw text only above MAX_RESPONSE; untruncated
        # outputs can be hash-reverified byte for byte.
        if len(raw)<8192:
            require(hashlib.sha256(raw).hexdigest()==sample["rawSha256"],
                    "Raw model completion SHA mismatch")
    source_ids={s[1:] for s in identities if s[0]=="source_only"}
    host_ids={s[1:] for s in identities if s[0]=="host_classified"}
    require(source_ids==host_ids and len(source_ids)*2==len(samples),
            "Source-only and host-classified case identities differ")
    return report,hashlib.sha256(blob).hexdigest()

def summarize(report):
    result={}
    for arm in ("source_only","host_classified"):
        cases=[x for x in report["samples"] if x["arm"]==arm]
        rated={}
        for name,fields in DIMENSIONS.items():
            # A malformed completion is UNKNOWN, not an assumed safe denial.
            valid=sum(bool(x.get("checks"))
                      and all(x["checks"].get(k) is True for k in fields)
                      for x in cases)
            rated[name]={"passed":valid,"total":len(cases)}
        result[arm]={
            "modelDecisionsPassingAll":sum(x.get("allPass") is True for x in cases),
            "total":len(cases),"KIAFChecks":rated,
            "formatFailures":sum(x.get("classification")=="MODEL_OUTPUT_FORMAT" for x in cases),
            "unsafeAuthorityAssertions":sum(x.get("classification")=="MODEL_UNSAFE_AUTHORITY_ASSERTION" for x in cases),
            "feedbackOverclaims":sum(x.get("classification")=="MODEL_FEEDBACK_OVERCLAIM" for x in cases),
            "meanLatencySeconds":report.get("arms",{}).get(arm,{}).get("meanLatencySeconds"),
        }
    return result

def compare(raw_path,schema_path):
    raw,raw_sha=verified_report(raw_path,"QWEN3_EMPTY_THINK_PREFILL_V1")
    structured,schema_sha=verified_report(schema_path,"LLAMA_COMPLETION_JSON_SCHEMA_V1")
    for key in ("modelSha256","nativeFixtureSha256","positiveFixtureSha256",
                "reviewedFixtureSha256","repetitions"):
        require(raw.get(key)==structured.get(key),"Different source/model/benchmark: "+key)
    lhs={(s["arm"],s["caseId"],s["repetition"]) for s in raw["samples"]}
    rhs={(s["arm"],s["caseId"],s["repetition"]) for s in structured["samples"]}
    require(lhs==rhs,"Unpaired case matrix")
    return {
        "schema":"lu-r00-genuine-model-kiaf-interface-comparison-v1",
        "modelActuallyLoadedInBothArms":True,
        "modelSha256":raw["modelSha256"],
        "nativeFixtureSha256":raw["nativeFixtureSha256"],
        "rawReportSha256":raw_sha,"schemaReportSha256":schema_sha,
        "realModelGenerations":raw["observedModelGenerations"]+structured["observedModelGenerations"],
        "protocols":{
            "raw":{"responseProtocol":raw["responseProtocol"],"results":summarize(raw)},
            "json_schema":{"responseProtocol":structured["responseProtocol"],"results":summarize(structured)}
        },
        "hostAuthority":"NO_MODEL_PERMISSION_GRANT",
        "installedLu":False,"physicalGpuProof":False,
        "modelCompetenceAccepted":raw["overallPass"] and structured["overallPass"],
        "note":"The schema constrains syntax only; source truth, authority, freshness and OS custody remain independently scored. A benchmark report can be complete while the model fails."
    }

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--raw",type=Path,required=True)
    p.add_argument("--schema",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    args=p.parse_args()
    report=compare(args.raw,args.schema)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print(json.dumps({"measurementAccepted":True,
                     "modelCompetenceAccepted":report["modelCompetenceAccepted"],
                     "generations":report["realModelGenerations"]},sort_keys=True))

if __name__=="__main__": main()
