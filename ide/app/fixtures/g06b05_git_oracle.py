#!/usr/bin/env python3
"""Independent, read-only Git oracle for IDE 6B-05 installed commit/amend acceptance.

The IDE must not call this helper. This helper deliberately uses an independent
Git executable, not Lu's libgit2/SQLite/journals. It refuses non-fixture repos.
"""
import argparse
import hashlib
import json
import os
import stat
from pathlib import Path
import subprocess
import sys

MARKER = '.ide-g06b05-disposable-fixture'
TARGET = 'reviewed.txt'
OTHER = 'unrelated.txt'
BASE_LINES = [f'line {n:02d}: unchanged\n' for n in range(1, 51)]
BASE_LINES[3] = 'line 04: alpha original\n'
BASE_LINES[43] = 'line 44: omega original\n'


def fail(reason):
    raise ValueError('G06B05_ORACLE_DENIED:' + reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(repo, *args, input=None, check=True):
    p = subprocess.run(['git', '-C', str(repo), *args], input=input,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                       check=False)
    if check and p.returncode:
        fail('git_command_failed:' + ':'.join(args[:2]) + ':' + p.stderr.decode('utf8', 'replace')[:160])
    return p.stdout


def original(mode):
    lines = BASE_LINES.copy()
    if mode == 'amend':
        lines[3] = 'line 04: previously committed\n'
    return ''.join(lines).encode()


def working(mode):
    lines = original(mode).decode().splitlines(keepends=True)
    lines[3] = 'line 04: reviewed new value\n'
    lines[43] = 'line 44: UNSELECTED worktree change\n'
    return ''.join(lines).encode()


def selected(mode):
    lines = original(mode).decode().splitlines(keepends=True)
    lines[3] = 'line 04: reviewed new value\n'
    return ''.join(lines).encode()


def safe_repo(repo):
    repo = Path(repo).resolve(strict=True)
    if not repo.is_dir() or not (repo / MARKER).is_file():
        fail('missing_disposable_marker')
    if (repo / MARKER).read_text() != 'IDE_G06B05_DISPOSABLE_ONLY\n':
        fail('invalid_disposable_marker')
    actual = Path(os.fsdecode(git(repo, 'rev-parse', '--show-toplevel').strip())).resolve()
    if actual != repo:
        fail('not_exact_repo_root')
    if (repo / '.git').is_symlink() or not (repo / '.git').is_dir():
        fail('linked_or_nonstandard_gitdir')
    if git(repo, 'remote').strip():
        fail('remotes_are_forbidden')
    if git(repo, 'symbolic-ref', '--quiet', 'HEAD').strip() != b'refs/heads/main':
        fail('unexpected_branch')
    return repo


def init_fixture(root, mode):
    root = Path(root).absolute()
    if root.exists():
        fail('fixture_directory_already_exists')
    root.mkdir(parents=True, mode=0o700)
    git(root, 'init', '--initial-branch=main', '--quiet')
    git(root, 'config', 'user.name', 'IDE G06B05 fixture')
    git(root, 'config', 'user.email', 'g06b05@example.invalid')
    (root / MARKER).write_text('IDE_G06B05_DISPOSABLE_ONLY\n')
    (root / OTHER).write_bytes(b'unrelated object, must never change\n')
    (root / TARGET).write_bytes(original('create'))
    git(root, 'add', TARGET, OTHER)
    git(root, '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture parent')
    if mode == 'amend':
        (root / TARGET).write_bytes(original('amend'))
        git(root, 'add', TARGET)
        git(root, '-c', 'commit.gpgsign=false', 'commit', '-qm', 'existing amend target')
    (root / TARGET).write_bytes(working(mode))
    safe_repo(root)
    print(json.dumps({'fixture': str(root), 'mode': mode,
                      'head': git(root, 'rev-parse', 'HEAD').decode().strip(),
                      'human_instruction': 'Open this disposable project in the INSTALLED IDE; select ONLY the first of two diff hunks.'}, indent=2))


def parse_index(repo):
    records = git(repo, 'ls-files', '--stage', '-z').split(b'\0')
    entries = {}
    for item in records:
        if not item:
            continue
        left, sep, path = item.partition(b'\t')
        if sep != b'\t' or b' ' not in left:
            fail('invalid_index_entry')
        mode, oid, stage = left.decode('ascii').split(' ')
        p = os.fsdecode(path)
        if p in entries or stage != '0' or mode != '100644':
            fail('nonregular_duplicate_or_unmerged_index')
        entries[p] = {'mode': mode, 'oid': oid}
    return entries


def parse_tree(repo, revision):
    records = git(repo, 'ls-tree', '-rz', revision).split(b'\0')
    entries = {}
    for item in records:
        if not item:
            continue
        left, _, path = item.partition(b'\t')
        mode, typ, oid = left.decode('ascii').split(' ')
        if typ != 'blob':
            fail('unreviewed_nonblob_tree_entry')
        p = os.fsdecode(path)
        entries[p] = {'mode': mode, 'oid': oid}
    return entries


def refs(repo):
    result = {}
    for line in git(repo, 'for-each-ref', '--format=%(refname) %(objectname)').decode().splitlines():
        name, oid = line.split(' ', 1)
        result[name] = oid
    return result


def ancestry(repo, head):
    return git(repo, 'rev-list', '--parents', '-n', '1', head).decode().strip().split()


def worktree_files(repo):
    entries = {}
    for base, dirs, files in os.walk(repo, followlinks=False):
        dirs[:] = [d for d in dirs if d != '.git']
        for name in files:
            path = Path(base) / name
            if not stat.S_ISREG(path.lstat().st_mode):
                fail('nonregular_fixture_worktree')
            entries[str(path.relative_to(repo))] = sha(path.read_bytes())
    return entries


def state(repo):
    return {'head': git(repo, 'rev-parse', 'HEAD').decode().strip(),
            'refs': refs(repo), 'index': parse_index(repo),
            'index_sha256': sha((repo / '.git' / 'index').read_bytes()),
            'worktree': worktree_files(repo),
            'index_lock': ((repo / '.git' / 'index.lock').read_bytes().hex()
                           if (repo / '.git' / 'index.lock').exists() else None)}


def snapshot(repo, manifest, mode, expect_index, expect_foreign_lock=False):
    repo = safe_repo(repo)
    manifest = Path(manifest).absolute()
    if manifest.exists() or manifest.is_relative_to(repo):
        fail('manifest_must_be_new_and_outside_repo')
    st = state(repo)
    if st['worktree'][TARGET] != sha(working(mode)):
        fail('fixture_worktree_not_expected')
    if (st['index_lock'] is not None) != expect_foreign_lock:
        fail('unexpected_foreign_index_lock_at_snapshot')
    entries = st['index']
    if set(entries) != {TARGET, OTHER}:
        fail('unexpected_fixture_index_paths')
    target = original(mode) if expect_index == 'initial' else selected(mode)
    if git(repo, 'show', ':' + TARGET) != target:
        fail('index_not_at_requested_checkpoint')
    if parse_tree(repo, st['head']) != parse_index(repo) and expect_index == 'initial':
        fail('initial_index_not_clean')
    st['history'] = git(repo, 'rev-list', '--all').decode().splitlines()
    payload = {'schema': 'ide-g06b05-independent-git-v1', 'repo': str(repo),
               'mode': mode, 'expect_index': expect_index,
               'foreign_lock_expected': expect_foreign_lock, 'state': st}
    manifest.parent.mkdir(parents=True, exist_ok=True)
    with manifest.open('x', encoding='utf8') as f:
        json.dump(payload, f, sort_keys=True, indent=2)
        f.write('\n')
        f.flush()
        os.fsync(f.fileno())
    print('G06B05_SNAPSHOT=' + str(manifest))


def verify(manifest, outcome):
    with Path(manifest).open(encoding='utf8') as f:
        record = json.load(f)
    if record.get('schema') != 'ide-g06b05-independent-git-v1':
        fail('unknown_snapshot_schema')
    repo = safe_repo(record['repo'])
    before = record['state']
    after = state(repo)
    if after['worktree'] != before['worktree']:
        fail('tracked_or_unrelated_worktree_modified')
    if after['index'] != before['index']:
        fail('selected_index_or_unrelated_index_modified')
    if after['index_lock'] != before['index_lock']:
        fail('foreign_lock_created_removed_or_modified')
    for oid in before['history']:
        if git(repo, 'cat-file', '-t', oid).strip() != b'commit':
            fail('original_history_object_lost')
    other_refs = {k: v for k, v in before['refs'].items() if k != 'refs/heads/main'}
    if {k: v for k, v in after['refs'].items() if k != 'refs/heads/main'} != other_refs:
        fail('unrelated_git_refs_changed')
    if outcome in ('unchanged', 'denied', 'cancelled', 'interrupted_prepublication'):
        if after['refs'] != before['refs'] or after['head'] != before['head']:
            fail('denied_cancelled_or_interrupted_operation_changed_head')
        if after['index_sha256'] != before['index_sha256']:
            fail('denied_cancelled_or_interrupted_operation_changed_index_bytes')
    elif outcome in ('created', 'amended'):
        expected_mode = record['mode']
        if expected_mode == 'create' and outcome == 'amended':
            fail('expected_create_not_amend')
        if expected_mode == 'amend' and outcome == 'created':
            fail('expected_amend_not_create')
        if record['expect_index'] != 'selected':
            fail('commit_requires_preverified_selected_index')
        if after['head'] == before['head'] or after['refs'].get('refs/heads/main') != after['head']:
            fail('reviewed_branch_not_updated_exactly_once')
        parent = ancestry(repo, before['head'])
        expected_parent = before['head'] if expected_mode == 'create' else (parent[1] if len(parent) == 2 else None)
        published = ancestry(repo, after['head'])
        if len(published) != 2 or published[1] != expected_parent:
            fail('published_commit_parent_or_merge_changed')
        if parse_tree(repo, after['head']) != before['index']:
            fail('published_tree_differs_from_selected_index')
        if after['refs'].get('refs/heads/main') == before['head']:
            fail('branch_update_not_observed')
    else:
        fail('unknown_expected_outcome')
    print('G06B05_INDEPENDENT_GIT_' + outcome.upper() + '=PASS')
    print('G06B05_HISTORY_ORIGINAL_COMMITS_RETAINED=' + str(len(before['history'])))
    print('G06B05_UNRELATED_WORKTREE_INDEX_REFS=UNMODIFIED')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest='command', required=True)
    fixture = sub.add_parser('fixture')
    fixture.add_argument('--directory', required=True)
    fixture.add_argument('--mode', choices=['create', 'amend'], required=True)
    snap = sub.add_parser('snapshot')
    snap.add_argument('--repo', required=True)
    snap.add_argument('--manifest', required=True)
    snap.add_argument('--mode', choices=['create', 'amend'], required=True)
    snap.add_argument('--expect-index', choices=['initial', 'selected'], required=True)
    snap.add_argument('--expect-foreign-lock', action='store_true')
    check = sub.add_parser('check')
    check.add_argument('--manifest', required=True)
    check.add_argument('--outcome', required=True, choices=['unchanged', 'denied', 'cancelled',
                       'interrupted_prepublication', 'created', 'amended'])
    args = p.parse_args()
    if args.command == 'fixture':
        init_fixture(args.directory, args.mode)
    elif args.command == 'snapshot':
        snapshot(args.repo, args.manifest, args.mode, args.expect_index, args.expect_foreign_lock)
    else:
        verify(args.manifest, args.outcome)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, json.JSONDecodeError) as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(2)
