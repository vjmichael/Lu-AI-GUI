#!/usr/bin/python3
"""Native SEC-06C X11 private-cookie and private-profile admission regression.

This is a nonroot *preflight*, not a substitute for the unchanged original
guardian or genuine Chromium/Node kernel matrix. The first-party privileged
launcher/manager must never walk Xvfb's 0700 user-owned directory. The
nonroot mediator must verify the exact original cookie bytes' ownership,
type and confidentiality before using it.
"""
import os
import pathlib
import runpy
import stat
import tempfile
import sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
def module(name):
    return runpy.run_path(str(ROOT/"scripts"/name),run_name="sec06c_original_x11_preflight")
def check():
    if sys.platform!="linux" or os.geteuid()==0:
        raise AssertionError("SEC06C private XAUTHORITY test requires nonroot Linux")
    broker=module("lu-sec06c-owned-browser-launcher.py")
    manager=module("lu-sec06c-owned-browser-manager.py")
    mediator=module("lu-sec06c-owned-browser-worker.py")
    with tempfile.TemporaryDirectory(prefix="xvfb-run.sec06c-") as root:
        top=pathlib.Path(root)
        top.chmod(0o700)
        auth=top/"Xauthority"
        auth.write_bytes(b"\x00SEC06C_TEST_USER_X11_COOKIE\xff")
        auth.chmod(0o600)
        profile=top/"lu-browser-profile"
        profile.mkdir(mode=0o700)
        for module in (broker,manager):
            lexical=module["unprivileged_xauthority_path"]
            assert lexical(str(auth)), "legitimate private X11 file not recognized lexically"
            assert lexical(str(profile)), "legitimate private user browser profile denied"
            assert lexical(""), "absence of optional user X11 cookie is allowed"
            for bad in ("relative/Xauthority", "/tmp/../etc/shadow",
                "/tmp//xvfb-run/auth", "/proc/self/mem",
                "/sys/fs/cgroup/cgroup.procs", "/dev/null",
                "/tmp/xvfb-run/auth\nINJECT"):
                assert not lexical(bad), "untrusted original X11 path admitted: "+bad
        native=mediator["validate_original_user_xauthority"]
        assert native(str(auth))==str(auth), "nonroot owner cookie rejected"
        link=top/"cookie-symlink"
        link.symlink_to(auth)
        try:native(str(link))
        except (RuntimeError,OSError):pass
        else:raise AssertionError("symlink to private cookie accepted")
        auth.chmod(0o644)
        try:native(str(auth))
        except (RuntimeError,OSError):pass
        else:raise AssertionError("public-readable X11 cookie accepted")
        auth.chmod(0o600)
        auth.write_bytes(b"")
        try:native(str(auth))
        except (RuntimeError,OSError):pass
        else:raise AssertionError("empty X11 credential accepted")
    print("SEC06C_BROWSER_PRIVATE_X11_COOKIE_PASS=1 original_uid=nonroot "
          "privileged_path_traversal=none private_x11=1 private_profile=1 "
          "user_cookie_verified=1 symlink_denied=1 public_denied=1",flush=True)
if __name__=="__main__":
    try:check()
    except Exception as e:
        print("SEC06C_BROWSER_PRIVATE_X11_COOKIE_FAILED: "+str(e),file=sys.stderr)
        sys.exit(93)
