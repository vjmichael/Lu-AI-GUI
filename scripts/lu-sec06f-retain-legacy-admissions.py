#!/usr/bin/python3
"""Root-only, fixed-path legacy admission retention before quiescent reboot.

Do not register this maintenance action in Lu or its sudo policy. Preserve
old shared GO entries and spent records; never infer receipt or delete data.
"""
import os
import pathlib
import runpy
import stat
import sys


def main():
    if os.geteuid()!=0 or len(sys.argv)!=1:
        raise RuntimeError("root-only fixed maintenance action; no arguments")
    image=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
    for item in (image.parent.parent,image.parent,image):
        st=item.lstat()
        if st.st_uid!=0 or st.st_mode&0o022 or not (stat.S_ISREG(st.st_mode) if item==image else stat.S_ISDIR(st.st_mode)):
            raise RuntimeError("untrusted installed maintenance dependency")
    k=runpy.run_path(str(image),run_name="lu_legacy_denial_maintenance")
    count=k["retain_legacy_markers"]()
    print("SEC06F_LEGACY_DENIALS_RETAINED="+str(count),flush=True)


if __name__=="__main__":
    try:main()
    except Exception as error:
        print("SEC06F_LEGACY_RETENTION_BLOCKED: "+str(error),file=sys.stderr)
        sys.exit(1)
