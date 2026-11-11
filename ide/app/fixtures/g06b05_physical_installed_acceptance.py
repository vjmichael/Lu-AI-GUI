#!/usr/bin/env python3
"""Interactive IDE G06B05 physical Windows NSIS / Linux Debian acceptance witness.

Run on a human-controlled graphical machine, never in CI. This program never
presses a native OS approval button. Operator acts inside the INSTALLED IDE;
an independent system git executable verifies the resulting history and bytes.
Only newly created throwaway local fixtures are permitted.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from g06b05_git_oracle import (
    TARGET, OTHER, fail, git, init_fixture, snapshot, state, verify,
)

SCENARIOS = {
    'stage-refuse': ('create', 'initial', 'denied'),
    'stage-cancel': ('create', 'initial', 'cancelled'),
    'commit-refuse': ('create', 'selected', 'denied'),
    'commit-cancel': ('create', 'selected', 'cancelled'),
    'commit-approve': ('create', 'selected', 'created'),
    'amend-first-refuse': ('amend', 'selected', 'denied'),
    'amend-second-refuse': ('amend', 'selected', 'denied'),
    'amend-approve': ('amend', 'selected', 'amended'),
    'kill-before-publication': ('create', 'selected', 'interrupted_prepublication'),
    'restart-after-publication': ('create', 'selected', 'created'),
    'foreign-lock-refuse': ('create', 'selected', 'denied'),
}


def installed_process_path(pid):
    if pid <= 0:
        fail('process_id_required')
    if os.name == 'nt':
        command = ['powershell.exe', '-NoProfile', '-NonInteractive', '-Command',
                   f'(Get-Process -Id {pid} -ErrorAction Stop).Path']
        process = subprocess.run(command, capture_output=True, text=True, timeout=20)
        if process.returncode:
            fail('installed_process_not_found')
        name = process.stdout.strip()
        return Path(name).resolve(strict=True)
    return Path(f'/proc/{pid}/exe').resolve(strict=True)


def require_installed(executable, pid):
    candidate = Path(executable).resolve(strict=True)
    expected = 'lu-ide-desktop.exe' if os.name == 'nt' else 'lu-ide-desktop'
    if candidate.name.lower() != expected:
        fail('wrong_desktop_executable')
    if not candidate.is_file() or candidate.is_symlink():
        fail('installed_executable_not_regular_file')
    if os.name != 'nt':
        if candidate != Path('/usr/bin/lu-ide-desktop') and not str(candidate).startswith('/opt/'):
            fail('debian_executable_path_not_system_installed')
    if installed_process_path(pid) != candidate:
        fail('desktop_pid_not_matching_installed_executable')
    return candidate, hashlib.sha256(candidate.read_bytes()).hexdigest()


def acknowledge(prompt):
    print('\n' + prompt)
    print('Type OBSERVED only after physically checking the installed OS UI.')
    if input('Operator witness: ').strip() != 'OBSERVED':
        fail('human_observation_not_acknowledged')


def ask_pid(prompt):
    try:
        return int(input(prompt + ' (PID): ').strip())
    except ValueError:
        fail('invalid_pid')


def source_identity(expected):
    repo = Path(__file__).resolve().parents[3]
    recorded = git(repo, 'rev-parse', 'HEAD').decode().strip()
    if recorded != expected:
        fail('current_test_source_not_exact_expected_sha')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--installed-executable', required=True)
    p.add_argument('--source-sha', required=True)
    p.add_argument('--evidence-directory', required=True)
    p.add_argument('--scenario', choices=sorted(SCENARIOS), required=True)
    args = p.parse_args()
    if os.environ.get('GITHUB_ACTIONS') or not sys.stdin.isatty() or not sys.stdout.isatty():
        fail('physical_interactive_operator_only')
    if os.name not in ('nt', 'posix'):
        fail('windows_or_linux_only')
    if os.name == 'posix' and (sys.platform != 'linux' or os.geteuid() == 0 or
                                not (os.environ.get('DISPLAY') or os.environ.get('WAYLAND_DISPLAY'))):
        fail('nonroot_linux_desktop_required')
    if len(args.source_sha) != 40 or any(x not in '0123456789abcdef' for x in args.source_sha):
        fail('invalid_source_sha')
    source_identity(args.source_sha)
    evidence = Path(args.evidence_directory).absolute()
    if evidence.exists():
        fail('evidence_destination_must_be_new')
    evidence.mkdir(parents=True, mode=0o700)
    scenario = args.scenario
    mode, checkpoint, outcome = SCENARIOS[scenario]
    original_pid = ask_pid('Enter PID of your already running installed Lu IDE')
    exe, exe_hash = require_installed(args.installed_executable, original_pid)
    root = evidence / 'disposable-git-project'
    manifest = evidence / 'independent-before.json'
    init_fixture(root, mode)
    print('DISPOSABLE FIXTURE:', root)
    print('Do not point the installed IDE at a real project during this run.')
    acknowledge('Use the INSTALLED app native OS folder picker to register this NEW disposable fixture. '
                'Confirm Source Control shows two separate hunks in reviewed.txt and the unrelated file.')
    if checkpoint == 'selected':
        acknowledge('Select the FIRST reviewed hunk only, reverify the selection and approve '
                    'the separate native OS stage dialog. DO NOT create a commit or amend yet.')
        # The oracle reads the real Git index bytes; a fake UI success cannot pass.
        if git(root, 'show', ':' + TARGET) == git(root, 'show', 'HEAD:' + TARGET):
            fail('installed_selected_hunk_stage_not_observed')
    if scenario == 'foreign-lock-refuse':
        lock = root / '.git' / 'index.lock'
        with lock.open('xb') as f:
            f.write(b'foreign test writer; do not delete\n')
            f.flush()
            os.fsync(f.fileno())
    snapshot(root, manifest, mode, checkpoint, scenario == 'foreign-lock-refuse')
    action = {
        'stage-refuse': 'Request stage selected hunks, then REFUSE in the real native OS dialog.',
        'stage-cancel': 'Request stage selected hunks, then use Cancel before index publication; do not approve.',
        'commit-refuse': 'Review a new local commit, request commit, then REFUSE its separate OS dialog.',
        'commit-cancel': 'Review a new local commit, request it, then cancel before branch update.',
        'commit-approve': 'Review a new local commit and APPROVE its separate OS dialog exactly once.',
        'amend-first-refuse': 'Review amendment then REFUSE the first native OS commit authorization.',
        'amend-second-refuse': 'Review amendment, APPROVE first dialog, REFUSE FINAL HISTORY-REWRITE CONSENT.',
        'amend-approve': 'Review amendment and APPROVE both distinct native OS dialogs, exactly once each.',
        'kill-before-publication': 'Request commit and leave the native OS approval dialog OPEN. Do not approve it.',
        'restart-after-publication': 'Review new commit and APPROVE its separate OS dialog once. Verify displayed commit receipt.',
        'foreign-lock-refuse': 'With foreign index.lock present, attempt reviewed local commit, then inspect recovery. Do not delete lock.',
    }[scenario]
    acknowledge(action)
    restart_pid = None
    if scenario in ('kill-before-publication', 'restart-after-publication'):
        # This intentionally does not pretend to target the CAS instruction.
        # For kill-before, OS approval remains pending and no publish can occur.
        if installed_process_path(original_pid) != exe:
            fail('original_installed_process_disappeared_unexpectedly')
        print('The test will terminate only the above exact installed desktop PID.')
        acknowledge('Confirm this is the disposable test window and it is safe to forcefully stop.')
        if os.name == 'nt':
            subprocess.run(['taskkill', '/PID', str(original_pid), '/F'], check=True)
        else:
            os.kill(original_pid, 9)
        acknowledge('Restart EXACT same installed executable using the OS, inspect the journal and '
                    'confirm no automatic ref retry, grant restoration or silent lock deletion.')
        restart_pid = ask_pid('Enter new installed IDE process')
        if restart_pid == original_pid:
            fail('restart_pid_not_changed')
        restarted, hash_after_restart = require_installed(exe, restart_pid)
        if restarted != exe or hash_after_restart != exe_hash:
            fail('installed_executable_changed_on_restart')
    verify(manifest, outcome)
    if hashlib.sha256(exe.read_bytes()).hexdigest() != exe_hash:
        fail('installed_executable_mutated')
    ending = state(root)
    record = {
        'schema': 'ide-g06b05-physical-operator-witness-v1',
        'created_utc': datetime.now(timezone.utc).isoformat(),
        'source_sha': args.source_sha,
        'scenario': scenario,
        'installed_executable': str(exe),
        'installed_executable_sha256': exe_hash,
        'original_pid': original_pid,
        'restart_pid': restart_pid,
        'operator_acknowledged_real_native_interaction': True,
        'independent_git_oracle_pass': True,
        'before_manifest': manifest.name,
        'head_after': ending['head'],
        'index_sha256_after': ending['index_sha256'],
        'physical_gui_proof': 'operator-acknowledged; attach independent OS screenshot/accessibility artifact',
        'signed_release_approved': False,
        'group06b05_full_matrix_closed': False,
    }
    with (evidence / 'operator-witness.json').open('x', encoding='utf8') as f:
        json.dump(record, f, indent=2)
        f.write('\n')
        f.flush()
        os.fsync(f.fileno())
    print('G06B05_REAL_INSTALLED_SCENARIO_AND_INDEPENDENT_GIT=PASS')
    print('G06B05_OPERATOR_WITNESS=' + str(evidence / 'operator-witness.json'))
    print('Reminder: this result is only ONE physical matrix scenario on ONE OS.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, EOFError) as exc:
        print(str(exc), file=sys.stderr)
        sys.exit(2)
