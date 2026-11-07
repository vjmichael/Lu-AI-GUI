"""Lu source-only interoperability evidence for NVP.

Runs on a separate branch from the functional 0.1.0-alpha.2 baseline.
No model is downloaded, loaded or unloaded; no listener is opened.
"""
from __future__ import annotations
import hashlib
import json
from pathlib import Path

EXPECTED={
 "src-tauri/lu-core/proto/lu/v1/core.proto":
   "6495ad09b4403dd8f6c714d3358d31da2a43e63a",
 "src-tauri/lu-core/src/persistent_llama_backend.rs":
   "ea28e818b1a62538ab3f1afc10c7b588f9ff0e1a",
 "src-tauri/lu-core/examples/persistent_llama_smoke.rs":
   "79a31fed778c6feeb1a05d717f3f942c3949af69",
 "lu-desktop/src-tauri/src/lib.rs":
   "fb93dc2be3778cddbd9b502dec3bf5bd45656095",
}
def verify(root):
    got={}
    for path,expected in EXPECTED.items():
        source=(root/path)
        if not source.is_file() or source.is_symlink():
            raise RuntimeError("Lu pinned source missing or unsafe: "+path)
        raw=source.read_bytes()
        # actions/checkout may convert checked-out text files to CRLF
        # on native Windows; check the exact canonical Git blob bytes.
        canonical=raw.replace(b"\r\n",b"\n")
        sha=hashlib.sha1(b"blob "+str(len(canonical)).encode()+b"\0"+canonical).hexdigest()
        if sha!=expected:raise RuntimeError("Pinned Lu source drift: "+path)
        got[path]=sha
    # The separate jan_alt AF_UNIX crate is not part of independent desktop.
    if (root/"src-tauri/lu-ipc-common").exists():
        raise RuntimeError("Experimental socket carrier unexpectedly entered alpha source")
    return {"format":"lu.nvp.source.contract.v1",
        "pinned_source_commit":"ac015280e4594b7cb5fb730a240b8d2a8a819975",
        "blobs":got,"source_verified":True,
        "alpha_binary_executed":False,"real_gguf_inference_executed":False,
        "external_inference_socket_shipped":False,
        "host_approval_still_required":True}

if __name__=="__main__":
    data=verify(Path(__file__).resolve().parents[1])
    Path("lu_nvp_source_contract.json").write_text(
        json.dumps(data,indent=2,sort_keys=True),encoding="utf8")
    print(json.dumps(data,sort_keys=True))
