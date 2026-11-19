#!/usr/bin/env python3
"""Retrieve every canonical Lu IDE verification notice from its exact upstream tag.

A successful fetch proves that the vendored full license text equals the named
upstream source; it is NOT legal approval for all transitive components.
"""
import argparse
import hashlib
import json
import re
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
# Independently checked against the Git blob IDs at the exact named upstream revisions.
# Never derive these pins from an untrusted checkout, receipt, or download.
PINNED_BYTES = {
    'CodeMirror-6-LICENSE-MIT.txt': ('aa8f65819a471c1a25fb3c0816c5a75e45f009b7899d292ee3cc70d9dbdd8f7e', '3af12e62d9b6de4259803e6afabae22ccd35ad7c'),
    'LLVM-18.1.8-LICENSE.txt': ('8d85c1057d742e597985c7d4e6320b015a9139385cff4cbae06ffc0ebe89afee', 'fa6ac540007032cbd0ec772a1c72e6cb5527a4fe'),
    'Python-3.10.11-LICENSE.txt': ('3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf', 'f26bcf4d2de6eb136e31006ca3ab447d5e488adf'),
    'Tauri-LICENSE-APACHE-2.0.txt': ('0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594', 'f433b1a53f5b830a205fd2df78e2b34974656c7b'),
    'Tauri-LICENSE-MIT.txt': ('9dd42ea92cff2ede5cd477cbfcce051b2d0115c0ac7f368ee88cb545055dff1d', 'b08530d59433b3060f4fad8751ac2610c0b0aff3'),
    'esbuild-0.25.9-LICENSE-MIT.txt': ('b40ec5baec7bb34fa5b1c09521fa3cd52d5fad7adafed74932a2010d3612a681', '2027e8dcf3787f5aac9fcd835e2b80cb75a39211'),
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

def read_receipt_json(path):
    def unique(pairs):
        obj = {}
        for key, value in pairs:
            if key in obj:
                raise NoticeDenied("duplicate_receipt_json_key")
            obj[key] = value
        return obj
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique)
    except (UnicodeError, ValueError) as exc:
        raise NoticeDenied("invalid_receipt_json") from exc


def exact_notice_digest(name, data):
    if name not in PINNED_BYTES or not isinstance(data, bytes) or not 100 <= len(data) <= MAX_LICENSE:
        raise NoticeDenied("invalid_upstream_notice_bytes:" + str(name))
    digest = hashlib.sha256(data).hexdigest()
    blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
    if (digest, blob) != PINNED_BYTES[name]:
        raise NoticeDenied("source_notice_differs_from_upstream:" + name)
    return digest


def validate_receipt(proof, source_sha, actual_digests):
    """Bind receipt to immutable upstream pins AND independently read package bytes.

    This validates consistency, not a signed or independently witnessed HTTPS event.
    """
    if (not isinstance(source_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", source_sha)
            or not isinstance(proof, dict)
            or set(proof) != {"schema", "sourceSha", "allSourcesRetrieved", "licenseLegalReviewApproved", "releaseApproved", "sources"}
            or proof.get("schema") != "lu.ide.upstream-notice-fetch.v1"
            or proof.get("sourceSha") != source_sha
            or proof.get("allSourcesRetrieved") is not True
            or proof.get("licenseLegalReviewApproved") is not False
            or proof.get("releaseApproved") is not False
            or not isinstance(proof.get("sources"), list)
            or len(proof["sources"]) != len(SOURCES)
            or set(actual_digests) != set(SOURCES)):
        raise NoticeDenied("unverified_upstream_license_receipt")
    seen = set()
    for item in proof["sources"]:
        if not isinstance(item, dict) or set(item) != {"name", "sourceRevision", "sourceUrl", "sourceSha256", "bundledSourceSha256", "matchesUpstream"}:
            raise NoticeDenied("unverified_upstream_notice_source:malformed")
        name = item.get("name")
        if not isinstance(name, str) or name not in SOURCES or name in seen:
            raise NoticeDenied("unverified_upstream_notice_source:duplicate_or_unknown")
        seen.add(name)
        digest = actual_digests[name]
        revision, url = SOURCES[name]
        if (digest != PINNED_BYTES[name][0] or item.get("sourceRevision") != revision
                or item.get("sourceUrl") != url or item.get("matchesUpstream") is not True
                or item.get("sourceSha256") != digest or item.get("bundledSourceSha256") != digest):
            raise NoticeDenied("unverified_upstream_notice_source:" + name)


def verify_sources(root, source_sha, downloader=download):
    if not isinstance(source_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", source_sha):
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
        # Byte-exact canonical pins; Windows line-ending projections must not
        # substitute for originals. The tracked notices use -text attributes.
        exact_notice_digest(name, expected)
        exact_notice_digest(name, remote)
        if remote != expected:
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
