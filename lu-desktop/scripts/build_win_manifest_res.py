#!/usr/bin/env python3
"""Build a Windows .res containing a real RT_MANIFEST resource for PE linking.

This is intentionally restricted to one Win32 resource (type 24, ID 1,
US English) rather than pretending to implement a generic rc.exe compiler.
"""
import argparse
from pathlib import Path
import struct
import xml.etree.ElementTree as ET


def compiled_manifest(xml: bytes) -> bytes:
    # Validate XML before assembling Windows resources.
    root = ET.fromstring(xml)
    ns = '{urn:schemas-microsoft-com:asm.v1}'
    if root.tag != ns + 'assembly':
        raise ValueError('expected Win32 assembly manifest')
    identities = [v.attrib for v in root.iter(ns + 'assemblyIdentity')]
    if not any(i.get('name') == 'Microsoft.Windows.Common-Controls'
               and i.get('version') == '6.0.0.0'
               and i.get('publicKeyToken') == '6595b64144ccf1df'
               for i in identities):
        raise ValueError('Common Controls v6 dependency not declared')
    # A Win32 .res starts with an empty canonical 32-byte header.
    def entry(data: bytes, kind: int, resource_id: int, lang: int) -> bytes:
        head = struct.pack('<IIHHHHIHHII', len(data), 32,
            0xFFFF, kind, 0xFFFF, resource_id, 0, 0x0030, lang, 0, 0)
        assert len(head) == 32
        return head + data + b'\x00' * (-len(data) % 4)
    return entry(b'', 0, 0, 0) + entry(xml, 24, 1, 0x0409)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('manifest', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_bytes(compiled_manifest(args.manifest.read_bytes()))
    print('Windows RT_MANIFEST .res:', args.output, args.output.stat().st_size, 'bytes')
