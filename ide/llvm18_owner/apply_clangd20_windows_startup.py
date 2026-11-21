#!/usr/bin/env python3
"""Exact LLVM 20.1.8 clangd CI source repair; not release authorization."""
import hashlib
from pathlib import Path
import subprocess
import sys

PIN = '87f0227cb60147a26a1eeb4fb06e3b505e9c7261'
SOURCE = 'llvm/lib/Support/Windows/Process.inc'
BLOB = 'd525f5b16e862eb79f695daf5897c6a79bcc4ee9'
# Reuse only the reviewed startup algorithm, not the LLDB owner hook.
from apply_llvm18_owner_hook import WINDOWS_PROCESS_ORIGINAL, WINDOWS_PROCESS_REPAIRED


def patch_source(source: str) -> str:
    if source.count(WINDOWS_PROCESS_ORIGINAL) != 1:
        raise RuntimeError('DENIED: clangd upstream startup anchor changed')
    replacement = WINDOWS_PROCESS_REPAIRED
    start = replacement.index('    // A restricted token')
    end = replacement.index('\n  }\n  if (LongLength', start)
    replacement = replacement[:start] + r'''    // GetModuleFileNameW identifies the actually loaded image. An AppContainer
    // may execute it without permission to enumerate its ancestors. Never
    // recover a shortened/other executable name or a different OS error.
    const wchar_t *Leaf = ModuleName;
    for (const wchar_t *P = ModuleName; *P; ++P)
      if (*P == L'\\' || *P == L'/')
        Leaf = P + 1;
    const bool Clangd = ::CompareStringOrdinal(Leaf, -1, L"clangd.exe", -1,
                                                TRUE) == CSTR_EQUAL;
    if (Error != ERROR_ACCESS_DENIED || !Clangd)
      return mapWindowsError(Error);
    ResolvedName = ModuleName;
    LongLength = Length;
    static constexpr char Marker[] = "LU_LLVM20_FIXED_CLANGD_IMAGE_LEAF_USED\n";
    DWORD Written = 0;
    ::WriteFile(::GetStdHandle(STD_ERROR_HANDLE), Marker,
                static_cast<DWORD>(sizeof(Marker) - 1), &Written, nullptr);''' + replacement[end:]
    return source.replace(WINDOWS_PROCESS_ORIGINAL, replacement, 1)


def apply(root: Path) -> None:
    root = root.resolve(strict=True)
    actual = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != PIN:
        raise RuntimeError('DENIED: clangd upstream commit changed')
    if subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True).strip():
        raise RuntimeError('DENIED: clangd upstream checkout not clean')
    path = root / SOURCE
    if path.is_symlink() or not path.is_file():
        raise RuntimeError('DENIED: clangd upstream startup source missing')
    raw = path.read_bytes()
    if hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() != BLOB:
        raise RuntimeError('DENIED: clangd upstream startup blob changed')
    patched = patch_source(raw.decode('utf-8'))
    path.write_bytes(patched.encode('utf-8'))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('DENIED: exact LLVM checkout required')
    apply(Path(sys.argv[1]))
