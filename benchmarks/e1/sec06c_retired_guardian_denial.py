#!/usr/bin/python3
"""Original SEC-02/05 retired-guardian admission denial, independent native test.

DOES NOT replace or mark the original hostile fixtures PASS. Invokes the same
real compiled first-party binary in every old mode and demands the precise
kernel-custody fail-closed reason, *before* a worker can be admitted. Success
is only a negative admission witness; OS-owned positive cgroup and browser
evidence is separately mandatory for SEC-06F. No sudo, fake receipt, stub,
cgroup chmod, fallback process-group custody, or model-selected binary.
"""
import argparse
import os
import pathlib
import stat
import subprocess
import sys
import tempfile

MODES=("guardian-death","guardian-stall","worker-stall","ancestor-migration")
EXPECTED="SEC06C_BLOCKED_UNSAFE_WRITABLE_ANCESTOR"

def fail(reason):
    raise RuntimeError("SEC06C_RETIRED_GUARDIAN_DENIAL_BLOCKED: "+reason)

def main():
    if sys.platform!="linux" or os.geteuid()==0:
        fail("native unprivileged Linux required")
    p=argparse.ArgumentParser(allow_abbrev=False)
    p.add_argument("--fixture",required=True)
    p.add_argument("--guard",required=True)
    p.add_argument("--expected-sha",required=True)
    a=p.parse_args()
    repo=pathlib.Path(__file__).resolve().parents[2]
    source=subprocess.check_output(
        ["/usr/bin/git","-C",str(repo),"rev-parse","HEAD"],
        text=True,timeout=15).strip()
    if source!=a.expected_sha or len(source)!=40:
        fail("original exact-head identity changed")
    originals=[pathlib.Path(a.fixture),pathlib.Path(a.guard)]
    for exe in originals:
        if not exe.is_absolute() or exe.is_symlink() or not exe.is_file():
            fail("original executable unavailable or linked")
        st=exe.stat()
        if not stat.S_ISREG(st.st_mode) or not st.st_mode&0o111 or st.st_mode&0o022:
            fail("original compiled executable not independently reviewed")
    for mode in MODES:
        with tempfile.TemporaryDirectory(prefix="lu-sec06c-retired-") as temp:
            root=pathlib.Path(temp)
            evidence=root/"original-host-identity"
            outcome=root/"original-result"
            # This is the exact historical first-party fixture, NOT a
            # simulated denial or substitute security process.
            child=subprocess.run(
                [str(originals[0]),mode,str(root),str(evidence),str(outcome)],
                env=dict(os.environ,LU_AGENT01L_TEST_CGROUP_GUARD=str(originals[1])),
                stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,timeout=15,check=False)
            if child.returncode!=101:
                fail(mode+": expected explicit old-guardian refusal, exit="+str(child.returncode))
            stderr=child.stderr.decode("utf8",errors="replace")
            if EXPECTED not in stderr or "No original guardian admission" not in stderr:
                fail(mode+": wrong historical refusal or missing kernel-custody reason")
            if evidence.exists() or outcome.exists():
                fail(mode+": old guardian minted original process evidence or output")
    print("SEC06C_RETIRED_GUARDIAN_FAIL_CLOSED_NATIVE_PASS=1 "
          "historical_cases_preserved=4 original_worker_not_admitted=1 "
          "positive_root_owned_cgroup_still_required=1",flush=True)

if __name__=="__main__":
    try:main()
    except Exception as error:
        print("SEC06C_RETIRED_GUARDIAN_NATIVE_FAILED: "+str(error),file=sys.stderr)
        sys.exit(93)
