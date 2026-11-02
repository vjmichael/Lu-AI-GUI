#!/usr/bin/env bash
# R14 development-only cross-link. Assumes target MSVC toolchain/Cargo supplied
# by caller; does not download or install software, and cannot approve alpha.
# The Tauri RC is still a limited development route for VERSION/ICON, but lld
# now receives a genuine Windows RT_MANIFEST .res for Common Controls v6.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${CARGO_TARGET_DIR:?Cargo Windows target output directory required}"
: "${LU_R14_RESOURCE_DIR:?Directory to write the genuine manifest resource}"
mkdir -p "$LU_R14_RESOURCE_DIR"
resource="$LU_R14_RESOURCE_DIR/lu-desktop.manifest.res"
python3 "$root/scripts/build_win_manifest_res.py" "$root/scripts/lu-desktop.manifest" "$resource"
(cd "$root/src-tauri" && cargo rustc --offline --target x86_64-pc-windows-msvc --bin lu-desktop \
   -- -C link-arg=/manifest:no -C "link-arg=$resource")
python3 "$root/scripts/verify_win_v6_manifest.py" "$CARGO_TARGET_DIR/x86_64-pc-windows-msvc/debug/lu-desktop.exe"
