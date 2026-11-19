#!/usr/bin/env python3
"""Technical-only resource verification. No provider is executed or authorized.

The independently staged, SHA-pinned original archive tree is the reference,
not a receipt read from the installation being verified.
"""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

from collect_original_lsp_notices import validate, check_original
from review_upstream_lsp_archives import checked_manifest, sha256
from stage_complete_native_lsp_candidate import SCHEMA, safe_parts, candidate
from verify_p04_license_bundle import audit

RECEIPT = 'candidate-full-resource-receipt.json'
MARKER = 'DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION.txt'
WARNING = b'Lu IDE technical verification only. No provider execution, redistribution, signing or release approval.\n'
FLAGS = ('releaseApproved', 'redistributionApproved', 'legalReviewComplete',
         'protectedLuProtocolAccepted', 'realInstalledNsisDebAccepted',
         'dynamicDependencyClosureApproved')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def plain(path, directory=False, root_owned=False):
    info = path.lstat()
    require((stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode))
            and not (getattr(info, 'st_file_attributes', 0) & 0x400), 'linked or special entry')
    if root_owned:
        require(info.st_uid == 0 and not info.st_mode & 0o022, 'installed bytes not root owned/read-only to others')
    return info


def file_inventory(root, root_owned=False):
    plain(root, True, root_owned)
    files = set()
    names = set()
    visited = 0
    dirs = [root]
    while dirs:
        folder = dirs.pop()
        for path in folder.iterdir():
            visited += 1
            require(visited <= 10000, 'resource inventory exceeds bound')
            relative = path.relative_to(root).as_posix()
            safe_parts(relative)
            require(relative.casefold() not in names, 'case-aliased resource')
            names.add(relative.casefold())
            info = path.lstat()
            if stat.S_ISDIR(info.st_mode):
                plain(path, True, root_owned)
                dirs.append(path)
            else:
                plain(path, root_owned=root_owned)
                name = path.relative_to(root).as_posix()
                safe_parts(name)
                files.add(name)
    return files


def strict_json(path):
    plain(path)
    require(path.stat().st_size <= 4_000_000, 'oversized receipt')
    def unique(pairs):
        result = {}
        for k, v in pairs:
            require(k not in result, 'duplicate receipt field')
            result[k] = v
        return result
    return json.loads(path.read_bytes(), object_pairs_hook=unique)


def verify_resources(reference, observed, manifest, notices, platform, *, root_owned=False):
    reference_files = file_inventory(reference)
    observed_files = file_inventory(observed, root_owned)
    r = strict_json(reference / RECEIPT)
    require(r['schema'] == SCHEMA and r['platform'] == platform and
            all(r[k] is False for k in FLAGS) and r['allArchiveFileBytesRetained'] is True
            and r['originalNoticesVerified'] == 4, 'invalid or authorized source receipt')
    rows = {name: item for name, osname, item in checked_manifest(manifest) if osname == platform}
    require(set(r['providers']) == set(rows) == {'clangd', 'rust-analyzer', 'ruff'}, 'wrong provider set')
    originals = validate(notices)
    expected = {RECEIPT, MARKER}
    for name, row in r['providers'].items():
        require(row['archiveSha256'] == rows[name]['sha256'] and
                row['stagedOriginalEntries'] == row['originalArchiveEntries'], 'archive pin/entry mismatch')
        binary = 'bin/' + rows[name]['binary']
        require(row['files'][binary]['sha256'] == row['originalExecutableSha256'], 'binary digest mismatch')
        notice_rows = [n for n in originals if n['provider'] == name]
        for index, notice in enumerate(notice_rows):
            relative = 'NOTICES/' + f'{index:03d}-' + Path(notice['destination']).name
            check_original(notice, (reference / name / relative).read_bytes())
            require(relative in row['files'], 'missing pinned notice')
        require(len(row['files']) == row['originalArchiveEntries'] + len(notice_rows), 'incomplete resource set')
        for relative, meta in row['files'].items():
            safe_parts(relative)
            path = name + '/' + relative
            require(path not in expected and re.fullmatch('[0-9a-f]{64}', meta['sha256']) is not None
                    and type(meta['bytes']) is int and 0 <= meta['bytes'] <= 512*1024*1024, 'invalid file seal')
            expected.add(path)
            for root, owned in ((reference, False), (observed, root_owned)):
                p = root / path
                require(plain(p, root_owned=owned).st_size == meta['bytes'] and sha256(p) == meta['sha256'], 'resource bytes differ: '+path)
                if relative == binary and owned:
                    require(bool(p.stat().st_mode & 0o111), 'installed Linux provider not executable')
    # Full closed inventory, not just the three executables or notice counts.
    require(reference_files == expected and observed_files == expected, 'unexpected/missing resource file')
    require((observed / RECEIPT).read_bytes() == (reference / RECEIPT).read_bytes(), 'installed receipt differs from original staging')
    require((reference / MARKER).read_bytes() == WARNING and (observed / MARKER).read_bytes() == WARNING, 'technical restriction sentinel mismatch')
    return len(expected)


def verify_from_archives(staged, observed, manifest, notices, archives, original_notices, platform, *, root_owned=False):
    # Independently reconstruct from the original archive pins and Git notice
    # blobs. Two mutually consistent forged receipt copies must not suffice.
    with tempfile.TemporaryDirectory(prefix='dev02-independent-', dir=archives.parent) as temp:
        canonical = Path(temp) / 'original'
        candidate(manifest, notices, archives, original_notices, canonical, platform, fetch_locked=False)
        (canonical / MARKER).write_bytes(WARNING)
        verify_resources(canonical, staged, manifest, notices, platform)
        return verify_resources(canonical, observed, manifest, notices, platform, root_owned=root_owned)


def installed_root(package, windows_root):
    if package:
        require(os.name != 'nt' and package in ('lu-ide', 'lu-ide-desktop'), 'unexpected Debian package')
        status = subprocess.check_output(['dpkg-query','-W','-f=${Status}',package], text=True)
        require(status == 'install ok installed', 'package not installed')
        paths = subprocess.check_output(['dpkg-query','-L',package], text=True).splitlines()
        entries = [Path(p).parent for p in paths if p.endswith('/technical-lsp/'+MARKER)]
        require(len(entries) == 1, 'technical resource directory not unique in installed package')
        require(all(str(entries[0]/item) in paths for item in file_inventory(entries[0], True)), 'resource not owned by installed package')
        return entries[0], entries[0].parent / 'THIRD_PARTY_LICENSES', True
    require(os.name == 'nt' and windows_root is not None, 'not native Windows install')
    plain(windows_root, True)
    return windows_root / 'technical-lsp', windows_root / 'THIRD_PARTY_LICENSES', False


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--archives', type=Path, required=True)
    p.add_argument('--original-notices', type=Path, required=True)
    p.add_argument('--platform', choices=('linux-x86_64','windows-x86_64'), required=True)
    p.add_argument('--source-sha', required=True)
    group = p.add_mutually_exclusive_group(required=True)
    group.add_argument('--deb-package')
    group.add_argument('--windows-root', type=Path)
    a = p.parse_args()
    try:
        require(os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('RUNNER_TEMP'), 'disposable hosted CI only')
        require(re.fullmatch('[0-9a-f]{40}', a.source_sha) and subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip() == a.source_sha, 'source identity changed')
        runner = Path(os.environ['RUNNER_TEMP']).resolve()
        for path in (a.reference, a.archives, a.original_notices):
            require(path.resolve().is_relative_to(runner) and not path.is_symlink(), 'foreign staging reference')
        root, legal, owned = installed_root(a.deb_package, a.windows_root)
        require(owned == a.platform.startswith('linux'), 'wrong native platform')
        manifests = Path(__file__).parent
        count = verify_from_archives(a.reference, root, strict_json(manifests/'lsp_upstream_archive_candidates_20261108.json'),
                                 strict_json(manifests/'lsp_original_upstream_notices_20261108.json'),
                                 a.archives, a.original_notices, a.platform, root_owned=owned)
        plain(legal, True, owned)
        audit(legal, a.source_sha, require_upstream=True)
        print(f'DEV02_TECHNICAL_INSTALLED_ORIGINAL_RESOURCES_VERIFIED={count}')
        print('DEV02_TECHNICAL_INSTALLED_PROTECTED_EXECUTION=NOT_TESTED')
    except (OSError,ValueError,KeyError,TypeError,subprocess.SubprocessError) as exc:
        print('DENIED: '+str(exc),file=sys.stderr)
        return 2
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
