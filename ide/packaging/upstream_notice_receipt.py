#!/usr/bin/env python3
"""Retrieve every canonical Lu IDE verification notice from its exact upstream tag.

A successful fetch proves that the vendored full license text equals the named
upstream source; it is NOT legal approval for all transitive components.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request

SOURCES = {
    "LLVM-18.1.8-LICENSE.txt": ("llvmorg-18.1.8", "https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-18.1.8/LICENSE.TXT"),
    "Python-3.10.11-LICENSE.txt": ("v3.10.11", "https://raw.githubusercontent.com/python/cpython/v3.10.11/LICENSE"),
    "Tauri-LICENSE-MIT.txt": ("tauri-v2.12.5", "https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.12.5/LICENSE-MIT"),
    "Tauri-LICENSE-APACHE-2.0.txt": ("tauri-v2.12.5", "https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.12.5/LICENSE-APACHE-2.0"),
    "CodeMirror-6-LICENSE-MIT.txt": ("6.0.0", "https://raw.githubusercontent.com/codemirror/view/6.0.0/LICENSE"),
    "esbuild-0.25.9-LICENSE-MIT.txt": ("v0.25.9", "https://raw.githubusercontent.com/evanw/esbuild/v0.25.9/LICENSE.md"),
}
MAX_LICENSE = 2_000_000

class NoticeDenied(ValueError):
    pass

def download(url, opener=None, sleeper=None):
    """Bounded exact-origin retry for temporary transport failures only.

    Retry does not change pinned URLs, accepted response size, origin checks
    or exact vendored-byte verification. Redirects, size violations and
    permanent HTTP rejection remain immediate hard denials.
    """
    opener = opener or urllib.request.urlopen
    sleeper = sleeper or time.sleep
    request = urllib.request.Request(
        url, headers={"User-Agent": "Lu-IDE-license-provenance/1",
                      "Accept": "text/plain"})
    transient_http = {408, 429, 500, 502, 503, 504}
    for attempt in range(3):
        try:
            with opener(request, timeout=20) as response:
                if response.geturl() != url:
                    raise NoticeDenied("upstream_unexpected_redirect")
                data = response.read(MAX_LICENSE + 1)
            if len(data) > MAX_LICENSE:
                raise NoticeDenied("upstream_notice_oversize")
            return data
        except urllib.error.HTTPError as error:
            if attempt == 2 or error.code not in transient_http:
                raise
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            if attempt == 2:
                raise
        sleeper(attempt + 1)
    raise NoticeDenied("upstream_notice_fetch_unavailable")

def verify_sources(root, source_sha, downloader=download):
    if len(source_sha) != 40 or any(c not in "0123456789abcdefABCDEF" for c in source_sha):
        raise NoticeDenied("missing_exact_source_sha")
    root = Path(root) / "ide" / "packaging" / "upstream-licenses"
    records = []
    for name, (revision, url) in SOURCES.items():
        local = root / name
        if local.is_symlink() or not local.is_file() or not 100 <= local.stat().st_size <= MAX_LICENSE:
            raise NoticeDenied("upstream_notice_missing:" + name)
        expected = local.read_bytes()
        remote = downloader(url)
        if not isinstance(remote, bytes) or not 100 <= len(remote) <= MAX_LICENSE:
            raise NoticeDenied("invalid_upstream_notice_bytes:" + name)
        # Git checkouts can apply CRLF conversion on Windows. Compare meaningful
        # upstream license text after only that strictly bounded normalization.
        canonical = lambda b: b.replace(b"\r\n", b"\n")
        if canonical(remote) != canonical(expected):
            raise NoticeDenied("source_notice_differs_from_upstream:" + name)
        records.append({
            "name": name, "sourceRevision": revision, "sourceUrl": url,
            "sourceSha256": hashlib.sha256(remote).hexdigest(),
            "bundledSourceSha256": hashlib.sha256(expected).hexdigest(),
            "matchesUpstream": True,
        })
    return {"schema": "lu.ide.upstream-notice-fetch.v1", "sourceSha": source_sha,
            "allSourcesRetrieved": True, "licenseLegalReviewApproved": False,
            "releaseApproved": False, "sources": records}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.output.exists() or args.output.is_symlink():
            raise NoticeDenied("receipt_must_be_new")
        record = verify_sources(args.repo_root, args.source_sha)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("x", encoding="utf-8") as dst:
            json.dump(record, dst, indent=2, sort_keys=True)
            dst.write("\n")
        print(json.dumps({"upstreamLicenseSources": len(record["sources"]), "allSourcesRetrieved": True,
                          "legalReviewApproved": False}))
    except (OSError, ValueError, TypeError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, NoticeDenied) else "upstream_notice_fetch_unavailable"), file=sys.stderr)
        return 2
    return 0
if __name__ == "__main__":
    raise SystemExit(main())
