#!/usr/bin/env python3
"""Installed Linux graphical acceptance: real accessibility clicks and read-only SQLite.
Run on an ordinary disposable desktop with AT-SPI, X11, and a source-pinned installed Lu.
Does not invoke frontend IPC, seed application reports, or substitute test delays.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time

spec = importlib.util.spec_from_file_location('gui_helpers', Path(__file__).with_name('cp06er3-native-web-gui.py'))
gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)
require, wait, action, named, nodes = gui.require, gui.wait, gui.action, gui.named, gui.nodes
TITLE = 'Lu — Check Lu'
KINDS = ['reading', 'saved_history', 'search_speed', 'document_windows', 'search_scale']
COUNTS = [3, 4, 16, 16, 64]
SLOTS = ['lu-check-work', 'lu-check-work-2', 'lu-check-work-3', 'lu-check-work-4']


def journal(path):
    if not path.is_file():
        return None
    try:
        with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, timeout=.05) as conn:
            row = conn.execute('SELECT run_id,state,body FROM active_check WHERE slot=1').fetchone()
            return (row[0], row[1], json.loads(row[2])) if row else None
    except sqlite3.OperationalError:
        return None


def observed(path, test, label, seconds=30):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        row = journal(path)
        if row is not None and test(row):
            return row
        time.sleep(.01)
    raise RuntimeError('missing journal evidence: ' + label)


def snapshot(profile):
    result = {}
    for name in SLOTS:
        slot = profile / name
        if not slot.exists():
            continue
        require(slot.is_dir() and not slot.is_symlink(), 'unsafe fixture slot')
        result[name] = {}
        for child in sorted(slot.iterdir()):
            require(child.is_file() and not child.is_symlink(), 'unexpected fixture entry')
            result[name][child.name] = hashlib.sha256(child.read_bytes()).hexdigest()
    return result


def navigate(pid):
    action(pid, 'Settings', timeout=90)
    action(pid, 'Privacy & Observe')
    wait(lambda: named(pid, 'Run checks…', actionable=True), 30, 'check panel ready')


def show_review(pid):
    action(pid, 'Run checks…')
    return wait(lambda: named(pid, TITLE), 20, 'native approval dialog')


def choose(pid, box, choice):
    output = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Lu — Check Lu$'], capture_output=True, text=True, timeout=5)
    wins = output.stdout.splitlines()
    require(output.returncode == 0 and len(wins) == 1, 'native approval window missing or ambiguous')
    require(int(subprocess.check_output(['xdotool', 'getwindowpid', wins[0]], text=True, timeout=5)) == pid, 'approval owner mismatch')
    # GTK's message-dialog accessibility tree can expose the title as a
    # child label of an alert named "Information". Ascend only through
    # the same owning process to its nearest visible native dialog.
    # The independently checked X11 title and owner above remain mandatory.
    dialog = box
    for _ in range(12):
        require(dialog is not None and dialog.get_process_id() == pid,
                'approval accessibility owner mismatch')
        if dialog.get_role_name() in ('alert', 'dialog', 'window', 'frame'):
            require(dialog.get_state_set().contains(gui.Atspi.StateType.SHOWING),
                    'native approval dialog not showing')
            break
        dialog = dialog.get_parent()
    else:
        raise RuntimeError('native approval dialog ancestor unavailable')
    box = dialog
    text = []
    for node in nodes(box):
        try:
            text.append(node.get_name() or '')
            iface = node.get_text_iface()
            if iface:
                text.append(iface.get_text(0, -1))
        except Exception:
            continue
    disclosure = ' '.join(' '.join(text).split())
    for phrase in ('sample documents', 'Nothing is sent online', 'no model is loaded', 'no repairs are made', 'separate workspace'):
        require(phrase in disclosure, 'approval scope not visible: ' + phrase)
    action(pid, choice, box)


def capture_status(pid, out, phase):
    # Capture actual displayed pixels for independent review. AT-SPI actions
    # and SQLite witnesses still determine the automated lifecycle verdict;
    # WebKit does not expose every static status paragraph via AT-SPI.
    wait(lambda: named(pid, 'Run checks…', actionable=True), 15, 'check returned idle')
    time.sleep(.45)  # ordinary paint settling, not a change to workload timing
    target = out / (phase + '.png')
    subprocess.run(['scrot', str(target)], check=True, timeout=8)
    require(target.is_file() and target.stat().st_size > 0, 'status capture missing')


def check_prefix(row, full=False):
    body = row[2]
    results = body['results']
    require(body['expanded'] is True, 'wrong check suite')
    require([x['kind'] for x in results] == KINDS[:len(results)], 'unordered or unknown results')
    require(all(x['passed'] is True and x['verifiedOperations'] == COUNTS[i] and x['elapsedMicros'] >= 0 for i,x in enumerate(results)), 'unverified check result')
    if full:
        require(row[1] == 'complete' and len(results) == len(KINDS) and body['reason'] is None, 'complete report absent')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--exe', type=Path, required=True)
    p.add_argument('--sha256', required=True)
    p.add_argument('--fresh-data-home', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    require(os.geteuid() != 0, 'ordinary non-root desktop required')
    exe = a.exe.resolve(strict=True)
    require(exe.is_file() and exe.stat().st_uid == 0 and not exe.stat().st_mode & 0o022, 'protected installed binary required')
    actual = hashlib.sha256(exe.read_bytes()).hexdigest()
    require(actual == a.sha256, 'binary checksum mismatch')
    data_home = a.fresh_data_home.resolve()
    data_home.mkdir(parents=True, exist_ok=False)
    profile = data_home / 'org.lu.desktop'
    path = profile / 'lu-dx-traces.sqlite'
    a.out.mkdir(parents=True, exist_ok=False)
    verdict = {'status': 'INCOMPLETE', 'binary_sha256': actual, 'phases': {}, 'scope': 'installed Linux native accessibility; not physical owner acceptance'}
    env = dict(os.environ, XDG_DATA_HOME=str(data_home))
    app = None
    with (a.out / 'native.log').open('wb') as log:
        def launch():
            return subprocess.Popen([str(exe)], env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            app = launch()
            navigate(app.pid)
            require(journal(path) is None and not snapshot(profile), 'startup initiated check')
            box = show_review(app.pid)
            choose(app.pid, box, 'Cancel')
            wait(lambda: named(app.pid, 'Run checks…', actionable=True), 15, 'refusal returns idle')
            require(journal(path) is None and not snapshot(profile), 'refusal produced effects')
            capture_status(app.pid, a.out, 'refused')
            verdict['phases']['refusal'] = 'PASS'
            box = show_review(app.pid)
            choose(app.pid, box, 'Run checks')
            first = observed(path, lambda r: r[1] == 'complete', 'initial completed suite')
            check_prefix(first, full=True)
            wait(lambda: named(app.pid, 'Run checks…', actionable=True), 15, 'check returned idle')
            capture_status(app.pid, a.out, 'approved')
            verdict['phases']['approved'] = {'status': 'PASS', 'measurements': first[2]['results']}
            box = show_review(app.pid)
            choose(app.pid, box, 'Run checks')
            running = observed(path, lambda r: r[0] != first[0] and r[1] == 'running', 'live check')
            start = time.monotonic()
            action(app.pid, 'Stop', timeout=3)
            stopped = observed(path, lambda r: r[0] == running[0] and r[1] == 'stopped', 'durable GUI stop')
            check_prefix(stopped)
            require(stopped[2]['reason'] == 'user_stopped' and len(stopped[2]['results']) < len(KINDS), 'stop did not cancel unfinished work')
            verdict['phases']['stop'] = {'status': 'PASS', 'elapsed_seconds': time.monotonic()-start, 'verified_prefix': len(stopped[2]['results'])}
            wait(lambda: named(app.pid, 'Run checks…', actionable=True), 15, 'stopped check returned idle')
            capture_status(app.pid, a.out, 'stopped')
            box = show_review(app.pid)
            choose(app.pid, box, 'Run checks')
            in_flight = observed(path, lambda r: r[0] != stopped[0] and r[1] == 'running' and 0 < len(r[2]['results']) < len(KINDS), 'unfinished durable prefix')
            # Abruptly terminate only this disposable launched app, not any unrelated process.
            app.kill(); app.wait(timeout=10)
            before = journal(path)
            require(before and before[0] == in_flight[0] and before[1] == 'running', 'interruption did not occur in flight')
            saved = snapshot(profile)
            require(bool(saved), 'missing abandoned sample workspace')
            app = launch()
            navigate(app.pid)
            interrupted = observed(path, lambda r: r[0] == before[0] and r[1] == 'interrupted', 'cold-start interruption recovery')
            require(interrupted[2] == before[2] and snapshot(profile) == saved, 'restart changed verified prefix or discarded sample files')
            capture_status(app.pid, a.out, 'interrupted')
            time.sleep(1)
            require(journal(path) == interrupted, 'automatic replay')
            box = show_review(app.pid)
            choose(app.pid, box, 'Cancel')
            wait(lambda: named(app.pid, 'Run checks…', actionable=True), 15, 'recovery refusal returns idle')
            require(journal(path) == interrupted and snapshot(profile) == saved, 'recovery refusal changed prior state')
            box = show_review(app.pid)
            choose(app.pid, box, 'Run checks')
            fresh = observed(path, lambda r: r[0] != interrupted[0] and r[1] == 'complete', 'fresh separately approved check')
            check_prefix(fresh, full=True)
            require(snapshot(profile) == saved, 'fresh check changed retained interrupted workspace')
            capture_status(app.pid, a.out, 'fresh')
            verdict['phases']['interruption_recovery'] = 'PASS'
            verdict['status_captures'] = 'CAPTURED_FOR_INDEPENDENT_VISUAL_REVIEW'
            verdict['status'] = 'PASS'
        except Exception as exc:
            verdict['failure'] = str(exc)[:300]
            raise
        finally:
            if app is not None and app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    app.kill(); app.wait(timeout=5)
            (a.out / 'receipt.json').write_text(json.dumps(verdict, indent=2) + '\n')


if __name__ == '__main__':
    main()
