#!/usr/bin/env python3
"""CP-05E5 independent original ZIP review; never promotes a candidate."""
import hashlib
import json
import pathlib
import re
import stat
import subprocess
import sys
import zipfile

SOURCE = "99203ea5a014118e27b8677c1e50924da27cffc9"
RUN = 44409968222
ARTIFACT = 14477756536
ZIP_SHA = "ed6b67d11eb5dc464d807284f15e5f5cf87bf33b9a7a764d76051dc96381270b"
CANDIDATE = "0e2f63d71442a4178b517f496b827bd627e85d3c139add68ddb8449be0542319"
WORKER = "src-tauri/lu-core/workers/lu_browser_worker.mjs"

def require(condition, message):
    if not condition:
        raise ValueError(message)

def digest(stream):
    h = hashlib.sha256()
    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
        h.update(chunk)
    return h.hexdigest()

def safe(name):
    return (re.fullmatch(r"[A-Za-z0-9._ -]+(?:/[A-Za-z0-9._ -]+)*", name)
            and len(name) < 512 and all(
                part not in (".", "..") and part == part.strip() for part in name.split("/")))

def main():
    archive, output = map(pathlib.Path, sys.argv[1:])
    require(archive.is_file() and not archive.is_symlink(), "expected native archive")
    require(archive.stat().st_size == 563547593, "original archive size changed")
    with archive.open("rb") as f:
        require(digest(f) == ZIP_SHA, "original archive digest changed")
    canonical = subprocess.check_output(["git", "show", "HEAD:" + WORKER])
    with zipfile.ZipFile(archive) as z:
        members = {}
        folded = set()
        for info in z.infolist():
            name = info.filename.rstrip("/")
            require(safe(name), "unsafe archive path")
            require(not stat.S_ISLNK(info.external_attr >> 16), "archive link")
            if info.is_dir():
                continue
            require(name not in members and name.casefold() not in folded, "duplicate archive identity")
            members[name] = info
            folded.add(name.casefold())
        raw = z.read("manifest.json")
        require(hashlib.sha256(raw).hexdigest() == CANDIDATE, "manifest mismatch")
        m = json.loads(raw)
        require(m["schema"] == 1 and m["runtime"] == "lu-browser" and
                m["status"] == "candidate" and m["platform"] == "win32" and
                m["arch"] == "x64" and m["playwrightVersion"] == "1.64.1",
                "manifest metadata mismatch")
        require(len(m["files"]) == 482, "manifest count mismatch")
        names = set()
        for f in m["files"]:
            n = f["path"]
            require(safe(n) and n not in names and n in members, "invalid component")
            require(re.fullmatch("[0-9a-f]{64}", f["sha256"]) and
                    type(f["bytes"]) is int and 0 <= f["bytes"] <= 2_000_000_000 and
                    members[n].file_size == f["bytes"], "component size/hash invalid")
            with z.open(n) as stream:
                require(digest(stream) == f["sha256"], "component hash mismatch: " + n)
            names.add(n)
        require(set(members) == names | {
            "manifest.json", "manifest.sha256", "candidate-summary.json"}, "extra/missing contents")
        require({"chrome.exe", "node.exe", "lu_browser_worker.mjs",
                 "node_modules/playwright/index.mjs", "node_modules/playwright-core/index.mjs"}
                <= names, "required entrypoints missing")
        require(z.read("manifest.sha256").decode().strip() == CANDIDATE, "companion digest mismatch")
        summary = json.loads(z.read("candidate-summary.json"))
        require(summary["manifestSha256"] == CANDIDATE and summary["files"] == 482 and
                summary["status"] == "candidate" and summary["platform"] == "win32" and
                summary["arch"] == "x64" and summary["playwrightVersion"] == "1.64.1", "summary mismatch")
        require(z.read("lu_browser_worker.mjs") == canonical, "canonical worker mismatch")
        for n in ("playwright", "playwright-core"):
            require(json.loads(z.read("node_modules/" + n + "/package.json"))["version"] == "1.64.1",
                    "dependency version mismatch")
        for name in ("chrome.exe", "node.exe"):
            with z.open(name) as f:
                header = f.read(64)
                require(header[:2] == b"MZ", "invalid PE signature")
                offset = int.from_bytes(header[60:64], "little")
                require(64 <= offset < 1_000_000, "invalid PE offset")
                f.seek(offset)
                pe = f.read(26)
                require(pe[:4] == b"PE\x00\x00" and pe[4:6] == b"\x64\x86" and
                        pe[24:26] == b"\x0b\x02", "not x64 PE32+")
        # Node serialization exactly matches the separate promotion utility.
        proposed = subprocess.check_output(
            ["node", "-e", "const fs=require('fs');const m=JSON.parse(fs.readFileSync(0,'utf8'));m.status='reviewed';process.stdout.write(JSON.stringify(m,null,2)+'\\n')"],
            input=raw)
    output.mkdir(parents=True, exist_ok=False)
    (output / "original-manifest.json").write_bytes(raw)
    # This is an evidence-only digest; never stage, execute or write an approval.
    report = {
        "schema": "lu-cp05e5-browser-review-proposal-v1",
        "status": "EXACT_BYTES_REVIEWED_APPROVAL_PENDING",
        "approved": False, "sourceSha": SOURCE, "runId": RUN, "artifactId": ARTIFACT,
        "archiveSha256": ZIP_SHA, "candidateManifestSha256": CANDIDATE,
        "proposedReviewedManifestSha256": hashlib.sha256(proposed).hexdigest(),
        "fileCount": len(names), "canonicalWorkerSha256": hashlib.sha256(canonical).hexdigest(),
        "allFileHashes": True, "archiveCrc": True, "unexpectedFiles": 0,
        "playwrightVersions": "1.64.1", "x64PeEntrypoints": True,
        "publisherSignatures": "NOT_ESTABLISHED",
        "protectedNativeExecution": "NOT_TESTED_BY_THIS_REVIEW",
        "approvalScope": "none; requires separate exact-byte owner/source approval",
    }
    (output / "review-proposal.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))

if __name__ == "__main__":
    main()
