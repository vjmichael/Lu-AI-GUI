#!/usr/bin/env python3
"""E1-P05 raw evidence contract. Never fabricates measurements or binaries.

Runs AFTER a genuine benchmark has created e1_p05_raw.json. Verifies
presence, source identity, paired tasks, native custody claims, and summary
math. Passing this checker does not independently prove external OS execution:
the actual native CI logs and preserved artifacts remain mandatory.
"""
import argparse
import hashlib
import json
import math
import pathlib
import statistics

ARMS = ("direct_model", "native_utility", "reviewed_app")
KINDS = ("docx", "xlsx")
SCHEMA = "lu-e1-p05-real-three-arm-v2"
NATIVE_SCHEMA = "lu-e1-p05-native-real-arm-v2"

def require(condition, message):
    if not condition:
        raise ValueError("E1_P05_RAW_EVIDENCE_INVALID: " + message)

def sha(value):
    return isinstance(value, str) and len(value) == 64 and all(
        letter in "0123456789abcdef" for letter in value
    )
def commit_sha(value):
    return isinstance(value, str) and len(value) == 40 and all(
        letter in "0123456789abcdef" for letter in value
    )

def verify(data, source_commit, require_model, repeats=2):
    require(isinstance(data, dict), "root must be an object")
    require(data.get("schema") == SCHEMA, "wrong frozen schema")
    require(commit_sha(source_commit) and data.get("sourceCommit") == source_commit,
            "source SHA not pinned to checked-out branch head")
    require(sha(data.get("nativeSha256")) and sha(data.get("workerSha256")),
            "real native executable / worker digests missing")
    require(data.get("status") == (
        "GENUINE_THREE_ARM_HEADLESS" if require_model
        else "NATIVE_ARMS_ONLY_MODEL_NOT_EXECUTED"
    ), "incorrect genuine-measurement status")
    if require_model:
        require(sha(data.get("modelSha256")) and sha(data.get("engineSha256")),
                "genuine model or inference server digest missing")
        require(data.get("limitations", {}).get("modelInferenceExecuted") is True,
                "model run incorrectly disclosed")
    else:
        require(data.get("modelSha256") is None and data.get("engineSha256") is None,
                "model not executed, but model provenance reported")
        require(data.get("limitations", {}).get("modelInferenceExecuted") is False,
                "native-only run is pretending to contain model inference")
    rows = data.get("results")
    require(isinstance(rows, list), "results missing")
    permitted = ARMS if require_model else ARMS[1:]
    require(data.get("trialCount") == repeats * len(KINDS) * len(permitted)
            == len(rows), "task count incomplete or wrong")
    seen = set()
    for row in rows:
        require(isinstance(row, dict) and row.get("schema") == NATIVE_SCHEMA,
                "native row schema invalid")
        arm, kind, trial = row.get("mode"), row.get("kind"), row.get("trial")
        key = (arm, kind, trial)
        require(arm in permitted and kind in KINDS and
                type(trial) is int and 0 <= trial < repeats,
                "unknown task/arm/trial")
        require(key not in seen, "duplicate measured task/arm/trial")
        seen.add(key)
        elapsed = row.get("latencyMs")
        require(type(elapsed) in (float, int) and math.isfinite(elapsed)
                and elapsed >= 0, "unmeasured or invalid latency")
        require(type(row.get("taskSuccess")) is bool, "task success not observed")
        if arm == "direct_model":
            require(row.get("modelUsed") is True
                    and row.get("source") == "actual-local-qwen-gguf-chat"
                    and isinstance(row.get("modelRawText"), str),
                    "raw genuine model transcript missing")
            require(row.get("artifactCreated") is False
                    and row.get("taskSuccess") is False,
                    "text-only model fabricated a document")
        else:
            require(row.get("modelUsed") is False
                    and row.get("source") == "real-compiled-lu-core-native"
                    and row.get("journalBacked") is True,
                    "native path or journal claim absent")
            require(row.get("supported") is True,
                    "both native DOCX and XLSX must be supported")
            require(row.get("artifactCreated") is True
                    and row.get("semanticAccuracy") is True
                    and row.get("taskSuccess") is True
                    and row.get("independentPythonZipWitness") is True
                    and row.get("independentPythonSemanticWitness") is True
                    and sha(row.get("artifactSha256"))
                    and type(row.get("artifactBytes")) is int
                    and row["artifactBytes"] > 0,
                    "real native Office publication/witness incomplete")
            require(row.get("unauthorizedAttempts") == 1
                    and row.get("unauthorizedAttemptsBlocked") == 1,
                    "independent host veto not proven")
    require(len(seen) == repeats * len(KINDS) * len(permitted),
            "task pairing not exhaustive")
    summary = data.get("summary")
    require(isinstance(summary, dict) and set(summary) == set(ARMS),
            "three-arm summary absent")
    for arm in ARMS:
        part = [r for r in rows if r["mode"] == arm]
        stats = summary[arm]
        if not part:
            require(not require_model and arm == "direct_model"
                    and stats.get("state") == "NOT_EXECUTED"
                    and stats.get("trials") == 0
                    and stats.get("p95Ms") is None,
                    "missing arm disguised as real data")
            continue
        require(stats.get("trials") == len(part), "summary count mismatch")
        ordered = sorted(r["latencyMs"] for r in part)
        require(stats.get("p95Estimator") == "nearest_rank_empirical"
                and stats.get("p95SampleCount") == len(ordered),
                "p95 methodology/sample size missing")
        p95 = round(ordered[math.ceil(len(ordered) * .95) - 1], 2)
        p50 = round(statistics.median(ordered), 2)
        require(stats.get("p95Ms") == p95 and stats.get("p50Ms") == p50,
                "p50/p95 do not reproduce measured rows")
        rate = sum(r["taskSuccess"] for r in part) / len(part)
        require(stats.get("taskSuccessRate") == rate, "task success rate mismatch")
        by_format = stats.get("byFormat", {})
        for kind in KINDS:
            subset = [r for r in part if r["kind"] == kind]
            require(by_format.get(kind, {}).get("total") == repeats
                    and by_format[kind].get("success") ==
                        sum(r["taskSuccess"] for r in subset),
                    "paired format count/success mismatch")
    return True

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("file", type=pathlib.Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--mode", choices=("native-only", "three-arm"), required=True)
    parser.add_argument("--repeats", type=int, default=2)
    args = parser.parse_args()
    require(args.repeats in (1, 2, 3), "unsupported repeats")
    data = json.loads(args.file.read_text(encoding="utf-8"))
    verify(data, args.source_commit, args.mode == "three-arm", args.repeats)
    print("E1_P05_RAW_EVIDENCE_CONTRACT_PASS",
          args.mode, args.source_commit, len(data["results"]))

if __name__ == "__main__":
    main()
