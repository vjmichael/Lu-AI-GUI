#!/usr/bin/env python3
"""WO7-B static PE imports closure gate for staged Windows native binaries.

Checks real import tables including delay imports. A static import pass is
NOT a complete audit of dynamic LoadLibrary calls, licenses or installed OS.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

from verify_wo7_nsis_release import Denied, safe_asset

SYSTEM_DLLS = {
    "advapi32.dll", "bcrypt.dll", "cabinet.dll", "combase.dll", "comctl32.dll",
    "comdlg32.dll", "crypt32.dll", "d3d11.dll", "dwmapi.dll", "gdi32.dll",
    "gdi32full.dll", "imm32.dll", "kernel32.dll", "kernelbase.dll",
    "mpr.dll", "mswsock.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll",
    "psapi.dll", "rpcrt4.dll", "secur32.dll", "setupapi.dll", "shell32.dll",
    "shcore.dll", "shlwapi.dll", "user32.dll", "userenv.dll", "uxtheme.dll",
    "version.dll", "winhttp.dll", "wininet.dll", "ws2_32.dll",
}
MAX_FILE = 1024 * 1024 * 1024
MAX_IMPORTS = 1024


def pe_imports(path: Path) -> set[str]:
    if not path.is_file() or path.stat().st_size == 0 or path.stat().st_size > MAX_FILE:
        raise Denied("pe_missing_or_oversized")
    data = path.read_bytes()
    def check(offset: int, size: int):
        if offset < 0 or offset + size > len(data):
            raise Denied("pe_truncated")
    def u16(offset: int) -> int:
        check(offset, 2)
        return struct.unpack_from("<H", data, offset)[0]
    def u32(offset: int) -> int:
        check(offset, 4)
        return struct.unpack_from("<I", data, offset)[0]
    check(0, 64)
    if data[:2] != b"MZ":
        raise Denied("not_pe")
    pe = u32(0x3c)
    check(pe, 24)
    if data[pe:pe+4] != b"PE\0\0":
        raise Denied("invalid_pe_signature")
    section_count, opt_len = u16(pe+6), u16(pe+20)
    if not 1 <= section_count <= 96:
        raise Denied("unreasonable_sections")
    opt = pe + 24
    check(opt, opt_len)
    magic = u16(opt)
    if magic == 0x20b:
        directory_at, count_at, image_base = opt + 112, opt + 108, struct.unpack_from("<Q", data, opt + 24)[0]
    elif magic == 0x10b:
        directory_at, count_at, image_base = opt + 96, opt + 92, u32(opt + 28)
    else:
        raise Denied("unsupported_pe_magic")
    if u32(count_at) < 2 or directory_at + 16 > opt + opt_len:
        raise Denied("pe_import_directory_missing")
    sections = []
    for i in range(section_count):
        start = opt + opt_len + i * 40
        check(start, 40)
        sections.append((u32(start+12), max(u32(start+8), u32(start+16)),
                         u32(start+20), u32(start+16)))
    def rva_at(rva: int, size: int) -> int:
        for va, extent, raw, raw_len in sections:
            if va <= rva and rva + size <= va + extent:
                off = rva - va
                if off + size > raw_len:
                    raise Denied("pe_import_in_unbacked_virtual_memory")
                check(raw+off, size)
                return raw + off
        raise Denied("pe_import_unmapped_rva")
    def read_dll_name(rva: int) -> str:
        offset = rva_at(rva, 1)
        end = data.find(b"\0", offset, min(len(data), offset+256))
        if end <= offset:
            raise Denied("missing_or_oversized_import_name")
        try:
            name = data[offset:end].decode("ascii").lower()
        except UnicodeDecodeError as exc:
            raise Denied("non_ascii_import_name") from exc
        if "/" in name or "\\" in name or not name.endswith(".dll"):
            raise Denied("unsafe_import_library_name")
        return name
    found = set()
    def directory(index: int) -> tuple[int, int]:
        if u32(count_at) <= index or directory_at + (index+1)*8 > opt+opt_len:
            return 0, 0
        return u32(directory_at+index*8), u32(directory_at+index*8+4)
    imp_rva, imp_size = directory(1)
    if imp_rva and imp_size:
        if imp_size > 65536:
            raise Denied("import_directory_too_large")
        for i in range(min(MAX_IMPORTS, imp_size // 20)):
            off = rva_at(imp_rva + i*20, 20)
            if data[off:off+20] == b"\0" * 20:
                break
            found.add(read_dll_name(u32(off+12)))
        else:
            raise Denied("unterminated_import_directory")
    delay_rva, delay_size = directory(13)
    if delay_rva and delay_size:
        if delay_size > 65536:
            raise Denied("delay_directory_too_large")
        for i in range(min(MAX_IMPORTS, delay_size // 32)):
            off = rva_at(delay_rva+i*32, 32)
            if data[off:off+32] == b"\0" * 32:
                break
            attrs, name_addr = u32(off), u32(off+4)
            name_rva = name_addr if attrs & 1 else name_addr - image_base
            found.add(read_dll_name(name_rva))
        else:
            raise Denied("unterminated_delay_directory")
    return found


def _digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _reviewed_inventory(root: Path, rows: object) -> dict[str, tuple[Path, dict]]:
    """Prevent a private DLL from bypassing the reviewed hash/license inventory."""
    if not isinstance(rows, list) or not 4 <= len(rows) <= 100:
        raise Denied("staged_binary_inventory_missing")
    reviewed = {}
    for row in rows:
        if not isinstance(row, dict):
            raise Denied("unreviewed_package_record")
        name = row.get("destination")
        if not isinstance(name, str):
            raise Denied("unreviewed_package_destination")
        key = name.casefold()
        if key in reviewed:
            raise Denied("duplicate_windows_binary_destination")
        if not name.lower().endswith((".exe", ".dll")):
            raise Denied("unreviewed_binary_type")
        binary = safe_asset(root, name)
        pin = row.get("sha256")
        if not isinstance(pin, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", pin):
            raise Denied("unreviewed_package_digest")
        if _digest(binary).lower() != pin.lower():
            raise Denied("staged_binary_digest_drift")
        if not isinstance(row.get("license"), str) or not row["license"].strip():
            raise Denied("staged_binary_license_not_reviewed")
        notices = row.get("notices")
        if not isinstance(notices, list) or not 1 <= len(notices) <= 16:
            raise Denied("staged_binary_notice_not_reviewed")
        for notice in notices:
            if not isinstance(notice, dict):
                raise Denied("invalid_notice_record")
            notice_path, notice_sha = notice.get("path"), notice.get("sha256")
            if (not isinstance(notice_path, str)
                    or not notice_path.startswith("resources/third-party-notices/")
                    or not isinstance(notice_sha, str)
                    or not re.fullmatch(r"[0-9a-fA-F]{64}", notice_sha)):
                raise Denied("unreviewed_notice_path_or_digest")
            notice_bytes = safe_asset(root, notice_path)
            if notice_bytes.stat().st_size > 1024 * 1024:
                raise Denied("oversized_license_notice")
            if _digest(notice_bytes).lower() != notice_sha.lower():
                raise Denied("license_notice_digest_drift")
        reviewed[key] = (binary, row)
    return reviewed


def audit_staged_pe(root: Path) -> dict:
    root = root.resolve(strict=True)
    receipt = json.loads(safe_asset(root, "resources/wo7-staged-tree.json").read_text(encoding="utf-8"))
    if receipt.get("schema") != "lu.ide.wo7.staged-tree.v1":
        raise Denied("staged_tree_schema_wrong")
    reviewed = _reviewed_inventory(root, receipt.get("files"))
    inspected = {}
    for name, (binary, row) in reviewed.items():
        imports = pe_imports(binary)
        for dll in imports:
            if dll.startswith(("api-ms-win-", "ext-ms-win-")) or dll in SYSTEM_DLLS:
                continue
            # The real Windows loader needs the private module immediately
            # beside this importer, not merely in another bundle directory.
            sibling = binary.parent / dll
            if sibling.is_symlink() or not sibling.is_file():
                raise Denied("unresolved_private_import:" + dll)
            if sibling.name.lower() != dll:
                raise Denied("private_import_case_mismatch")
            rel = sibling.relative_to(root).as_posix().casefold()
            if rel not in reviewed:
                raise Denied("unreviewed_private_import:" + dll)
            reviewed_binary, _ = reviewed[rel]
            if reviewed_binary != sibling or _digest(sibling) != reviewed[rel][1]["sha256"].lower():
                raise Denied("private_import_reviewed_digest_mismatch")
        inspected[row["destination"]] = sorted(imports)
    return {"schema": "lu.ide.wo7.pe-static-closure.v1",
            "staticImportClosure": True, "fullReviewedPrivateDllChain": True,
            "dynamicImportAuditStillRequired": True,
            "inspected": inspected, "releaseAccepted": False}

def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--staged-root", type=Path, required=True)
    args = p.parse_args()
    try:
        print(json.dumps(audit_staged_pe(args.staged_root), sort_keys=True))
        return 0
    except (OSError, ValueError, TypeError, KeyError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "malformed_pe_or_package"), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
