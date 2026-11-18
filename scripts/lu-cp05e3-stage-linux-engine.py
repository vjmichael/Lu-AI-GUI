#!/usr/bin/env python3
"""Stage an already reviewed, source-built x86_64 Linux CPU engine.
No downloads, installation, package approval or runtime permission. The build
operator supplies the generated pins JSON to rustc via LU_LINUX_CPU_ENGINE_PINS.
The exact eight ELF files must be installed in /usr/lib/Lu/lu-inference by the
trusted package installer, root-owned and not group/world writable.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

NAMES = ("llama-server", "libllama-server-impl.so", "libggml-base.so.0",
         "libggml-cpu.so.0", "libggml.so.0", "libllama-common.so.0",
         "libllama.so.0", "libmtmd.so.0")
SYSTEM = {"libc.so.6", "libm.so.6", "libstdc++.so.6", "libgcc_s.so.1",
          "libssl.so.3", "libcrypto.so.3", "ld-linux-x86-64.so.2"}

def stage(source, output):
    source = source.resolve(strict=True)
    if output.exists() or output.is_symlink():
        raise ValueError("refuse replacing an existing stage")
    with tempfile.TemporaryDirectory(prefix="lu-linux-engine-", dir=output.parent) as work:
        work = Path(work)
        package = work / "lu-inference"
        package.mkdir()
        script = work / "remove-rpath.cmake"
        script.write_text('file(RPATH_REMOVE FILE "${COMPONENT}")\n')
        pins = {}
        for name in NAMES:
            origin = (source / name).resolve(strict=True)
            if origin.parent != source or not origin.is_file() or origin.stat().st_size > 250_000_000:
                raise ValueError(f"invalid source component: {name}")
            target = package / name
            shutil.copyfile(origin, target)
            target.chmod(0o755 if name == "llama-server" else 0o644)
            hdr = subprocess.check_output(["readelf", "-h", str(target)], text=True, env={"PATH":"/usr/bin:/bin","LC_ALL":"C"})
            if "ELF64" not in hdr or "Advanced Micro Devices X86-64" not in hdr:
                raise ValueError(f"not an x86_64 ELF: {name}")
            subprocess.run(["cmake", f"-DCOMPONENT={target}", "-P", str(script)], check=True)
            dyn = subprocess.check_output(["readelf", "-d", str(target)], text=True, env={"PATH":"/usr/bin:/bin","LC_ALL":"C"})
            if re.search(r"\((?:RPATH|RUNPATH)\)", dyn):
                raise ValueError(f"build-tree runpath survived: {name}")
            deps = set(re.findall(r"\(NEEDED\).*?\[([^]]+)\]", dyn))
            if deps - (set(NAMES) | SYSTEM):
                raise ValueError(f"unreviewed library dependency: {name}: {deps - (set(NAMES) | SYSTEM)}")
            with target.open("rb") as f:
                digest=hashlib.file_digest(f,"sha256").hexdigest()
            pins[name] = {"bytes": target.stat().st_size, "sha256": digest}
        (work / "pins.json").write_text(json.dumps(pins, sort_keys=True, separators=(",", ":"))+"\n")
        script.unlink()
        output.mkdir()
        package.rename(output / "lu-inference")
        (work / "pins.json").rename(output / "pins.json")
    return output / "pins.json"

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reviewed-build-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args=parser.parse_args()
    print(stage(args.reviewed_build_bin,args.output.absolute()))

if __name__=="__main__":
    main()
