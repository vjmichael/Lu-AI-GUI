#!/usr/bin/env python3
"""Deterministic test-only Windows ICON resource for G04D cargo check.

Only CI may materialize this asset. It is NOT a reviewed application icon,
packaging input, or installed acceptance result. No network/Pillow required.
The real release must supply its own approved artwork and asset provenance.
"""
import hashlib
import os
from pathlib import Path
import struct

if os.environ.get("GITHUB_ACTIONS") != "true":
    raise SystemExit("DENIED: icon fixture is allowed only in CI")
app = Path(__file__).resolve().parents[1]
dest = app / "src-tauri" / "icons" / "icon.ico"
if dest.exists():
    raise SystemExit("DENIED: refusing to overwrite an existing real icon")
n = 32
pixels = bytearray()
for y in range(n - 1, -1, -1):
    for x in range(n):
        ink = (7 <= x <= 12 and 6 <= y <= 25) or (7 <= x <= 24 and 20 <= y <= 25)
        # BGRA, opaque gray-background / pale 'L' diagnostic fixture.
        pixels += bytes((228, 228, 228, 255) if ink else (62, 62, 62, 255))
mask = bytes(4 * n)  # 1-bit transparent AND mask, all opaque
bitmap = struct.pack("<IIIHHIIIIII", 40, n, n * 2, 1, 32, 0,
                     len(pixels), 0, 0, 0, 0) + pixels + mask
entry = struct.pack("<BBBBHHII", n, n, 0, 0, 1, 32, len(bitmap), 22)
ico = struct.pack("<HHH", 0, 1, 1) + entry + bitmap
assert len(ico) == 22 + 40 + n * n * 4 + 4 * n
dest.parent.mkdir(parents=True, exist_ok=True)
dest.write_bytes(ico)
print("IDE_G04D_WINDOWS_RESOURCE=CI_ONLY_NOT_RELEASE_SHA256=" +
      hashlib.sha256(ico).hexdigest())
