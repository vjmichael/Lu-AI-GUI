#!/usr/bin/env python3
"""Fail closed if a Win64 Lu executable lacks a valid embedded RT_MANIFEST v6.

Requires only Python 3; checks PE section/RVA bounds, resource tree and import
bindings. This does NOT execute Windows binaries or certify compatibility.
"""
from __future__ import annotations
import argparse
from pathlib import Path
import struct
import sys
import xml.etree.ElementTree as ET


class ManifestError(ValueError):
    pass


class PE:
    def __init__(self, path: Path):
        self.data = path.read_bytes()
        d = self.data
        if len(d) < 256 or d[:2] != b'MZ':
            raise ManifestError('not a DOS MZ executable')
        pe_offset, = struct.unpack_from('<I', d, 0x3c)
        if pe_offset < 64 or pe_offset + 24 > len(d) or d[pe_offset:pe_offset+4] != b'PE\0\0':
            raise ManifestError('invalid PE signature')
        machine, nsections, _, _, _, opt_size, _ = struct.unpack_from('<HHIIIHH', d, pe_offset+4)
        if machine != 0x8664:
            raise ManifestError('not Windows AMD64 PE')
        opt = pe_offset+24
        if opt+opt_size > len(d) or struct.unpack_from('<H', d, opt)[0] != 0x20b or opt_size < 144:
            raise ManifestError('not valid PE32+ optional header')
        subsystem, = struct.unpack_from('<H', d, opt+68)
        if subsystem != 2:
            raise ManifestError('not Windows GUI subsystem')
        self.dirs = [struct.unpack_from('<II', d, opt+112+8*i) for i in range(4)]
        self.sections = []
        for i in range(nsections):
            p = opt+opt_size+40*i
            if p+40 > len(d):
                raise ManifestError('truncated section table')
            name = d[p:p+8].rstrip(b'\0').decode('ascii', 'replace')
            vsize, va, rawsize, fileoff = struct.unpack_from('<IIII', d, p+8)
            self.sections.append((name, vsize, va, rawsize, fileoff))

    def offset(self, rva: int, size: int = 1) -> int:
        if rva == 0 or size < 0:
            raise ManifestError('zero RVA or negative size')
        for _, vsize, va, rawsize, fileoff in self.sections:
            if va <= rva and rva+size <= va+min(vsize or rawsize, rawsize):
                out = fileoff+rva-va
                if out+size <= len(self.data):
                    return out
        raise ManifestError('RVA outside mapped file bounds')

    def manifest(self) -> bytes:
        base, length = self.dirs[2]
        if base == 0 or length == 0:
            raise ManifestError('missing PE resource directory (R13 regression)')
        start = self.offset(base, min(length,16))
        if length < 16:
            raise ManifestError('invalid .rsrc directory size')
        for wanted in (24, 1):  # RT_MANIFEST, executable ID 1
            start = self._descend(start, wanted)
        # Resolve first language child (expected 1033, but not mandatory).
        entries = self._dir_entries(start)
        if len(entries) != 1 or entries[0][1] & 0x80000000:
            raise ManifestError('manifest language leaf invalid')
        leaf_rel = entries[0][1]
        filepos = self.offset(base+leaf_rel,16)
        data_rva, size, _, _ = struct.unpack_from('<IIII',self.data,filepos)
        if size > 65536 or size < 80:
            raise ManifestError('manifest unreasonably large or small')
        offset = self.offset(data_rva,size)
        return self.data[offset:offset+size]

    def _dir_entries(self, filepos: int) -> list[tuple[int,int]]:
        if filepos+16 > len(self.data):
            raise ManifestError('resource directory truncated')
        named, ids = struct.unpack_from('<HH',self.data,filepos+12)
        if named+ids > 2048:
            raise ManifestError('suspicious resource count')
        end=filepos+16+8*(named+ids)
        if end > len(self.data):
            raise ManifestError('resource entries truncated')
        return [struct.unpack_from('<II',self.data,filepos+16+8*i) for i in range(named+ids)]

    def _descend(self, filepos:int, wanted:int) -> int:
        base=self.dirs[2][0]
        matches=[data for ident,data in self._dir_entries(filepos) if ident == wanted]
        if len(matches)!=1 or not matches[0]&0x80000000:
            raise ManifestError(f'resource type/id {wanted} missing or malformed')
        return self.offset(base+(matches[0]&0x7fffffff),16)

    def import_names(self) -> set[tuple[str,str]]:
        base, length=self.dirs[1]
        if not base or length < 20:
            return set()
        names=set()
        for i in range(min(length//20,4096)):
            p=self.offset(base+i*20,20)
            lookup, _, _, name_rva, iat = struct.unpack_from('<IIIII', self.data,p)
            if not any((lookup, name_rva, iat)):
                break
            p=self.offset(name_rva)
            end=self.data.find(b'\0',p,min(p+256,len(self.data)))
            if end<0: raise ManifestError('invalid import DLL name')
            dll=self.data[p:end].decode('ascii').lower()
            ilt=lookup or iat
            for j in range(4000):
                name_entry,=struct.unpack_from('<Q',self.data,self.offset(ilt+j*8,8))
                if not name_entry: break
                if name_entry & (1 << 63): continue
                np=self.offset(name_entry,3)+2
                end=self.data.find(b'\0',np,min(np+256,len(self.data)))
                if end<0:raise ManifestError('invalid imported symbol')
                names.add((dll,self.data[np:end].decode('ascii')))
            else:raise ManifestError('imported symbol count unbounded')
        return names


def verify(path: Path, *, require_taskdialog: bool=True) -> str:
    exe=PE(path)
    xml=exe.manifest()
    try:
        root=ET.fromstring(xml)
    except ET.ParseError as err:
        raise ManifestError('embedded manifest not valid XML') from err
    ns='{urn:schemas-microsoft-com:asm.v1}'
    if root.tag != ns+'assembly':
        raise ManifestError('not a Win32 assembly manifest')
    identities=[x.attrib for x in root.iter(ns+'assemblyIdentity')]
    ok=any(x.get('name')=='Microsoft.Windows.Common-Controls'
        and x.get('version')=='6.0.0.0'
        and x.get('publicKeyToken')=='6595b64144ccf1df'
        for x in identities)
    if not ok:
        raise ManifestError('no Common Controls v6 SxS activation')
    imports=exe.import_names()
    if require_taskdialog and ('comctl32.dll','TaskDialogIndirect') not in imports:
        raise ManifestError('expected TaskDialogIndirect import missing')
    return f'PASS: {path.name} PE x64 GUI, RT_MANIFEST ID 1, Common Controls v6, '+('TaskDialogIndirect imported' if require_taskdialog else 'manifest validated')

if __name__=='__main__':
    cli=argparse.ArgumentParser()
    cli.add_argument('exe',type=Path)
    cli.add_argument('--manifest-only',action='store_true')
    args=cli.parse_args()
    try:
        print(verify(args.exe, require_taskdialog=not args.manifest_only))
    except (ManifestError, OSError, struct.error) as e:
        print('FAIL:',e,file=sys.stderr)
        raise SystemExit(1)
