#!/usr/bin/env python3
"""Verify exact, minimal reviewed IDE IPC lock successors WITHOUT changing old approvals.

Original lock receipt lives in MERGE02C and remains immutable. This checker
accepts only bytes reproduced from the immutable Git base by exact named local
package dependency insertions. This is source-test approval, not release approval.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

BASE_HOST_BLOB = '90e52a812a834786d828fefb86d244732a882dd7'
BASE_DESKTOP_BLOB = 'ec7461e0a64a8fa121cffda7ecb74333c6f6686c'
BASE_HOST_SHA256 = '00de403d7aeb63cd476f7694d52af1db1ff25a87fd2460a782ad2301edf893ef'
BASE_DESKTOP_SHA256 = 'b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5a'
HOST_SUCCESSOR_SHA256 = 'f9fc6178178bf2d56c6ca308472b9ba2e35a27910e3a30f6b72e9339bb4d10b2'
DESKTOP_SUCCESSOR_SHA256 = '77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255'
REVIEW_FILE = 'ide/packaging/ide-precore-r2-ipc-lock-successor.json'

class Denied(ValueError):
    pass


def sha(data):
    return hashlib.sha256(data).hexdigest()


def original(repo, path, blob, digest):
    got = subprocess.check_output(['git','rev-parse','f1f0ed4bc1e740d2a9f85615b988b779a46651db:'+path], cwd=repo, text=True).strip()
    if got != blob:
        raise Denied('original_git_lock_blob_mismatch')
    data = subprocess.check_output(['git','show','f1f0ed4bc1e740d2a9f85615b988b779a46651db:'+path], cwd=repo)
    if sha(data) != digest:
        raise Denied('original_git_lock_bytes_mismatch')
    return data


def reproduce(base, pkg_name, additions):
    # Permits only adding explicitly reviewed, previously locked dependencies
    # to one local package; every unrelated byte stays identical.
    section = (b'[[package]]\nname = "' + pkg_name.encode() + b'"\n')
    if base.count(section) != 1:
        raise Denied('unreviewed_local_package_count')
    head, tail = base.split(section, 1)
    original_block, following = tail.split(b'\n[[package]]', 1)
    anchor = b'dependencies = [\n'
    if original_block.count(anchor) != 1:
        raise Denied('unreviewed_local_dependencies')
    first, other = original_block.split(anchor,1)
    dep_bytes, end = other.split(b']\n', 1)
    dep_list = dep_bytes.strip().splitlines()
    values = [x.strip().strip(b',').strip(b'"') for x in dep_list]
    if sorted(values) != values or len(set(values)) != len(values):
        raise Denied('unreviewed_dependency_order')
    for dep in additions:
        bdep = dep.encode()
        if bdep in values:
            raise Denied('dependency_already_present')
        values.append(bdep)
    values.sort()
    # Preserve the original formatting, including its leading single space.
    new_deps = b''.join(b' "' + v + b'",\n' for v in values)
    return head + section + first + anchor + new_deps + b']\n' + end + b'\n[[package]]' + following


def validate_review(review):
    expected_review = {
      'schema':'lu.ide.precore-r2-ipc-native-lock-successor.v1',
      'baseIdeCommit':'f1f0ed4bc1e740d2a9f85615b988b779a46651db',
      'changedLocalPackage':'lu-ide-workspace@0.1.0',
      'hostAddedExistingDependencies':['rustix'],
      'desktopAddedExistingDependencies':['rustix','zeroize'],
      'hostSha256':HOST_SUCCESSOR_SHA256,
      'desktopSha256':DESKTOP_SUCCESSOR_SHA256,
      'originalMERGE02CUnchanged':True,
      'externalPackageChanges':0,
      'releaseApproved':False,
      'legalReviewed':False,
      'physicalOsAccepted':False,
      'mainMergeAuthorized':False
    }
    if review != expected_review:
        raise Denied('successor_review_not_independently_pinned')
    return True


def check(repo, overrides=None, review_override=None):
    repo = Path(repo)
    review = review_override if review_override is not None else json.loads((repo/REVIEW_FILE).read_text())
    validate_review(review)
    if subprocess.run(['git','merge-base','--is-ancestor',review['baseIdeCommit'],'HEAD'],cwd=repo,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=False).returncode:
        raise Denied('ide_source_not_descended_from_reviewed_pr_base')
    for path,blob,base_sha,current_sha,additions,count in (
        ('ide/host/Cargo.lock',BASE_HOST_BLOB,BASE_HOST_SHA256,HOST_SUCCESSOR_SHA256,['rustix'],166),
        ('ide/app/src-tauri/Cargo.lock',BASE_DESKTOP_BLOB,BASE_DESKTOP_SHA256,DESKTOP_SUCCESSOR_SHA256,['rustix','zeroize'],500),
    ):
        base = original(repo,path,blob,base_sha)
        target = reproduce(base,'lu-ide-workspace',additions)
        data = (overrides or {}).get(path)
        if data is None:
            file=repo/path
            if not file.is_file() or file.is_symlink():
                raise Denied('successor_lock_file_missing_or_linked')
            data=file.read_bytes()
        if sha(target) != current_sha or data != target:
            raise Denied('unreviewed_lock_successor_bytes')
        graph=tomllib.loads(data.decode('utf-8'))
        if graph.get('version') != 4 or len(graph.get('package',[])) != count:
            raise Denied('unreviewed_lock_successor_graph')
    return True

if __name__ == '__main__':
    import sys
    try:
        check(Path(__file__).resolve().parents[2])
    except (Denied,OSError,ValueError,subprocess.CalledProcessError) as exc:
        print('DENIED:',exc,file=sys.stderr);sys.exit(2)
    print('IDE_PRECORE_R2_IPC_LOCK_SUCCESSOR=VERIFIED_SOURCE_ONLY_NOT_RELEASE')
