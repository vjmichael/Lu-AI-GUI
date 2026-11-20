#!/usr/bin/env python3
"""Opt-in installed model evaluation with native actions and read-only report verification.
Uses a separately pinned GGUF in a new disposable app profile; no IPC or report writes.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

spec = importlib.util.spec_from_file_location('sample_gui', Path(__file__).with_name('cp11a-native-checks-gui.py'))
sample_gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sample_gui)
gui = sample_gui.gui
require, wait, action, named, nodes = gui.require, gui.wait, gui.action, gui.named, gui.nodes
journal, observed = sample_gui.journal, sample_gui.observed
TITLE = 'Lu — Check model'
KINDS = ['model_extract', 'model_arithmetic', 'model_history']


def sha(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def model_worker(parent):
    result = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal():
            continue
        pid = int(entry.name)
        info = gui.proc_identity(pid)
        if info and info[2].name == 'llama-server' and gui.owned(pid, parent):
            result.append((pid, info[1]))
    return result[0] if len(result) == 1 else None


def check_review(pid, choice):
    action(pid, 'Check loaded model…')
    label = wait(lambda: named(pid, TITLE), 20, 'separate native model review')
    wins = subprocess.check_output(['xdotool', 'search', '--onlyvisible', '--name', '^Lu — Check model$'], text=True, timeout=5).splitlines()
    require(len(wins) == 1 and int(subprocess.check_output(['xdotool', 'getwindowpid', wins[0]], text=True, timeout=5)) == pid, 'review owner/identity mismatch')
    dialog = label
    for _ in range(12):
        require(dialog is not None and dialog.get_process_id() == pid, 'incorrect accessible review owner')
        if dialog.get_role_name() in ('alert', 'dialog', 'window', 'frame'):
            break
        dialog = dialog.get_parent()
    else:
        raise RuntimeError('no review ancestor')
    text = []
    for node in nodes(dialog):
        try:
            text.append(node.get_name() or '')
            iface = node.get_text_iface()
            if iface:
                text.append(iface.get_text(0, -1))
        except Exception:
            continue
    disclosure = ' '.join(' '.join(text).split())
    for phrase in ('currently loaded local model', 'three short sample tasks', 'known results', 'No personal conversations', 'new conversation work takes priority'):
        require(phrase in disclosure, 'missing disclosure: ' + phrase)
    action(pid, choice, dialog)


def verify_report(row, full=False):
    body = row[2]
    require(body.get('modelSuite') is True and body['expanded'], 'incorrect suite')
    results = body['results']
    require([r['kind'] for r in results] == KINDS[:len(results)], 'invalid result order')
    for result in results:
        m = result['inference']
        require(result['verifiedOperations'] == int(result['passed']), 'invalid independent score')
        require(0 <= m['firstOutputMicros'] <= result['elapsedMicros'] and 0 < m['outputPieces'] <= m['outputBytes'] <= 16384, 'invalid live output measurements')
        require(m['sampledPeakRamBytes'] > 0 and m['ramSamples'] > 0, 'memory unmeasured')
        require(m['engineCpuMicros'] is not None and m['engineCpuMicros'] > 0, 'CPU time unmeasured')
    if full:
        require(len(results) == len(KINDS), 'missing terminal model results')
        require(row[1] == ('complete' if all(r['passed'] for r in results) else 'attention'), 'inconsistent quality verdict')
    return results


def capture(pid, out, phase):
    wait(lambda: named(pid, 'Check loaded model…', actionable=True), 20, 'idle model panel')
    time.sleep(.45)
    subprocess.run(['scrot', str(out / (phase + '.png'))], check=True, timeout=8)


def load(pid):
    action(pid, 'Choose a model', timeout=90)
    action(pid, 'Load selected', timeout=30)
    worker = wait(lambda: model_worker(pid), 120, 'owned genuine inference worker')
    wait(lambda: named(pid, 'Ready to chat'), 120, 'loaded model ready')
    return worker


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'model', 'fresh-data-home', 'out'):
        parser.add_argument('--'+name, type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--model-sha256', required=True)
    a = parser.parse_args()
    require(os.geteuid() != 0, 'ordinary non-root desktop required')
    exe = a.exe.resolve(strict=True)
    require(exe.stat().st_uid == 0 and not exe.stat().st_mode & 0o022 and sha(exe) == a.sha256, 'protected executable identity mismatch')
    require(sha(a.model) == a.model_sha256, 'GGUF identity mismatch')
    home = a.fresh_data_home.resolve();home.mkdir(parents=True, exist_ok=False)
    root = home / 'org.lu.desktop';folder = root / 'lu-models';folder.mkdir(parents=True)
    model = folder / a.model.name;shutil.copyfile(a.model, model)
    require(sha(model) == a.model_sha256, 'fixture copy mismatch')
    path = root / 'lu-dx-traces.sqlite';a.out.mkdir(parents=True, exist_ok=False)
    verdict = {'status':'INCOMPLETE','binary_sha256':a.sha256,'model_sha256':a.model_sha256,'phases':{},'scope':'installed native Debian; fixed text sample tasks only'}
    env = dict(os.environ, XDG_DATA_HOME=str(home))
    app = None;worker = None
    with (a.out / 'native.log').open('wb') as log:
        try:
            app = subprocess.Popen([str(exe)], env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            worker = load(app.pid)
            sample_gui.navigate(app.pid)
            require(journal(path) is None, 'startup initiated a check')
            check_review(app.pid, 'Cancel')
            wait(lambda: named(app.pid, 'Check loaded model…', actionable=True), 15, 'refusal returned idle')
            require(journal(path) is None and model_worker(app.pid) == worker, 'refusal produced effects')
            verdict['phases']['refusal'] = 'PASS'
            check_review(app.pid, 'Check model')
            first = observed(path, lambda r:r[1] in ('complete', 'attention'), 'first completed model suite', 130)
            results = verify_report(first, full=True)
            require(model_worker(app.pid)==worker, 'benchmark replaced model worker')
            verdict['phases']['approved'] = {'status':'PASS','quality_correct':sum(r['passed'] for r in results),'quality_total':len(results),'measurements':results}
            capture(app.pid,a.out,'approved')
            check_review(app.pid,'Check model')
            running = observed(path,lambda r:r[0]!=first[0] and r[1]=='running','running model check')
            start=time.monotonic();action(app.pid,'Stop',timeout=4)
            stopped = observed(path,lambda r:r[0]==running[0] and r[1]=='stopped','durable stop',40)
            require(stopped[2]['reason']=='user_stopped' and len(stopped[2]['results'])<len(KINDS),'stop did not prevent unfinished credit')
            verify_report(stopped)
            verdict['phases']['stop']={'status':'PASS','elapsed_seconds':time.monotonic()-start,'verified_prefix':len(stopped[2]['results']),'same_worker_retained':model_worker(app.pid)==worker}
            capture(app.pid,a.out,'stopped')
            # New opt-in selection after stopping must not inherit approval.
            if model_worker(app.pid)==worker:
                check_review(app.pid,'Cancel')
                wait(lambda: named(app.pid, 'Check loaded model…', actionable=True), 15, 'new refusal returned idle')
                require(journal(path)==stopped,'fresh refusal altered report')
                verdict['phases']['fresh_refusal']='PASS'
            else:
                verdict['phases']['fresh_refusal']='NOT RUN: engine safely ejected after stop'
            verdict['status']='PASS'
        except BaseException as e:
            verdict['error']=str(e);raise
        finally:
            if app and app.poll() is None:
                app.terminate()
                try:app.wait(15)
                except subprocess.TimeoutExpired:app.kill();app.wait(5)
            (a.out / 'receipt.json').write_text(json.dumps(verdict,indent=2)+'\n')

if __name__=='__main__':main()
