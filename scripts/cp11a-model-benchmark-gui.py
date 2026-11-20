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
import sqlite3
import signal
import subprocess
import time

spec = importlib.util.spec_from_file_location('sample_gui', Path(__file__).with_name('cp11a-native-checks-gui.py'))
sample_gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sample_gui)
gui = sample_gui.gui
require, wait, action, named, nodes = gui.require, gui.wait, gui.action, gui.named, gui.nodes
journal, observed = sample_gui.journal, sample_gui.observed
TITLE = 'Lu — Check model'
KINDS = ['model_extract', 'model_arithmetic', 'model_history', 'model_order', 'model_unicode', 'model_constraints']


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
                text.append(gui.Atspi.Text.get_text(iface, 0, -1))
        except Exception:
            continue
    disclosure = ' '.join(' '.join(text).split())
    for phrase in ('currently loaded local model', 'six short sample tasks', 'known results', 'No personal conversations', 'new conversation work takes priority'):
        require(phrase in disclosure, 'missing disclosure: ' + phrase)
    action(pid, choice, dialog)


def verify_report(row, full=False):
    body = row[2]
    require(body.get('modelSuite') is True and body['expanded'] and body.get('modelSuiteVersion') == 2, 'incorrect suite')
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


def navigate(pid):
    if named(pid, 'Run checks…', actionable=True) or named(pid, 'Stop', actionable=True):
        return
    action(pid, 'Settings', timeout=90)
    # Settings retains its subsection when switching tabs. Avoid clicking an
    # ambiguous heading/button pair if the actual panel is already visible.
    if not named(pid, 'Run checks…', actionable=True) and not named(pid,'Stop', actionable=True):
        action(pid, 'Privacy & Observe')
    wait(lambda: named(pid,'Run checks…',actionable=True) or named(pid,'Stop',actionable=True),30,'check panel')


def send_chat(pid,prompt):
    node=wait(lambda:named(pid,'Message Lu',actionable=True),30,'ordinary composer')
    component=node.get_component_iface()
    require(component and component.grab_focus(),'composer focus refused')
    wait(lambda:node.get_state_set().contains(gui.Atspi.StateType.FOCUSED),5,'composer focus')
    subprocess.run(['xdotool','type','--clearmodifiers','--',prompt],check=True,timeout=12)
    wait(lambda:gui.Atspi.Text.get_text(node.get_text_iface(),0,-1)==prompt,5,'exact composer text')
    action(pid,'Send')


def ticks(worker):
    identity = gui.proc_identity(worker[0])
    if not identity or identity[1] != worker[1]:
        return None
    try:
        stat = Path(f'/proc/{worker[0]}/stat').read_text().rsplit(')',1)[1].split()
        return int(stat[11])+int(stat[12])
    except (OSError, ValueError, IndexError):
        return None


def active_inference(path, old_run, worker):
    row = observed(path,lambda r:r[0]!=old_run and r[1]=='running','new running model check')
    before=ticks(worker)
    require(before is not None,'original worker absent')
    def active():
        current=journal(path); now=ticks(worker)
        return current if current and current[0]==row[0] and current[1]=='running' and now is not None and now>before else None
    return wait(active,10,'live worker CPU advancement during check')


def completed_chat(root, prompt):
    for db in (root/'ansf'/'books').glob('*.sqlite'):
        try:
            with sqlite3.connect(db.as_uri()+'?mode=ro',uri=True,timeout=.05) as con:
                row=con.execute("""SELECT length(e.text) FROM ansf_live_chat_events u
                  JOIN ansf_visible_chat d ON d.session_id=u.session_id AND d.generation_id=u.user_turn_id
                  JOIN ansf_live_chat_events e ON e.seq=d.latest_event_seq
                  WHERE u.text=? AND u.kind='user' AND u.origin='host' AND u.source_entry_id IS NOT NULL
                    AND d.role='assistant' AND d.status='final' AND e.kind='assistant_progress' AND e.origin='host'
                    AND length(e.text)>0 AND EXISTS(SELECT 1 FROM ansf_live_chat_events c WHERE c.session_id=u.session_id
                       AND c.generation_id=u.user_turn_id AND c.kind='assistant_completed' AND c.origin='host')
                    AND NOT EXISTS(SELECT 1 FROM ansf_live_chat_events x WHERE x.session_id=u.session_id
                       AND x.generation_id=u.user_turn_id AND x.kind='assistant_cancelled')""",(prompt,)).fetchone()
                if row: return row[0]
        except sqlite3.OperationalError:
            continue
    return None


def capture(pid, out, phase):
    wait(lambda: named(pid, 'Check loaded model…', actionable=True), 20, 'idle model panel')
    time.sleep(.45)
    subprocess.run(['scrot', str(out / (phase + '.png'))], check=True, timeout=8)


def load(pid):
    action(pid, 'Choose a model', timeout=90)
    action(pid, 'Load selected', timeout=30)
    worker = wait(lambda: model_worker(pid), 120, 'owned genuine inference worker')
    # Reopen the actual model menu. The enabled unload action is a native
    # accessibility witness that the model has completed loading.
    action(pid, 'Choose a model', timeout=120)
    wait(lambda: named(pid, '⏏ Unload active model', actionable=True) or named(pid, 'Unload active model', actionable=True), 120, 'loaded model ready')
    action(pid, 'Choose a model')
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
            navigate(app.pid)
            require(journal(path) is None, 'startup initiated a check')
            check_review(app.pid, 'Cancel')
            wait(lambda: named(app.pid, 'Check loaded model…', actionable=True), 15, 'refusal returned idle')
            require(journal(path) is None and model_worker(app.pid) == worker, 'refusal produced effects')
            verdict['phases']['refusal'] = 'PASS'
            check_review(app.pid, 'Check model')
            first = observed(path, lambda r:r[1] in ('complete', 'attention'), 'first completed model suite', 245)
            results = verify_report(first, full=True)
            require(model_worker(app.pid)==worker, 'benchmark replaced model worker')
            verdict['phases']['approved'] = {'status':'PASS','quality_correct':sum(r['passed'] for r in results),'quality_total':len(results),'measurements':results}
            capture(app.pid,a.out,'approved')
            check_review(app.pid,'Check model')
            second = observed(path,lambda r:r[0]!=first[0] and r[1] in ('complete','attention'),'repeat completed suite',245)
            repeated=verify_report(second,full=True)
            require(model_worker(app.pid)==worker,'repeat changed loaded worker')
            verdict['phases']['repeat']={'status':'PASS','measurements':repeated,'same_outcomes':[x['passed'] for x in results]==[x['passed'] for x in repeated]}
            # Submit a real human message from Chat while worker CPU is
            # advancing on a separate benchmark. Read the real terminal ANSF
            # event, not just a GUI spinner or queue reservation.
            check_review(app.pid,'Check model')
            active=active_inference(path,second[0],worker)
            action(app.pid,'Chat')
            prompt='Say blue and nothing else. /no_think'
            send_chat(app.pid,prompt)
            start=time.monotonic()
            yielded=observed(path,lambda r:r[0]==active[0] and r[1]=='stopped' and r[2]['reason']=='chat_busy','benchmark yielded to conversation',45)
            verify_report(yielded)
            require(len(yielded[2]['results'])<len(KINDS),'benchmark did not yield')
            answer_bytes=wait(lambda:completed_chat(root,prompt),120,'authenticated completed ordinary chat')
            require(model_worker(app.pid)==worker,'chat priority ejected or replaced worker')
            verdict['phases']['priority']={'status':'PASS','elapsed_seconds':time.monotonic()-start,'verified_prefix':len(yielded[2]['results']),'same_worker':True,'ordinary_chat_nonempty':answer_bytes>0}
            navigate(app.pid)
            check_review(app.pid,'Check model')
            running=active_inference(path,yielded[0],worker)
            # Actual ungraceful process kill during witnessed engine activity.
            app.kill();app.wait(15)
            require(journal(path) is not None and journal(path)[0]==running[0] and journal(path)[1]=='running','abrupt interruption was not durable running state')
            wait(lambda:gui.proc_identity(worker[0]) is None or gui.proc_identity(worker[0])[1]!=worker[1],30,'interrupted worker exited')
            app=subprocess.Popen([str(exe)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
            navigate(app.pid)
            interrupted=observed(path,lambda r:r[0]==running[0] and r[1]=='interrupted','cold interruption reconciliation',30)
            verify_report(interrupted)
            require(model_worker(app.pid) is None,'restart replayed inference')
            require(interrupted[2]['results'][:len(running[2]['results'])]==running[2]['results'],'lost verified results')
            verdict['phases']['abrupt_recovery']={'status':'PASS','verified_prefix':len(interrupted[2]['results']),'no_replay':True,'worker_exited':True}
            action(app.pid,'Chat');worker=load(app.pid);navigate(app.pid)
            check_review(app.pid,'Cancel')
            wait(lambda:named(app.pid,'Check loaded model…',actionable=True),15,'fresh refusal after crash')
            require(journal(path)==interrupted,'fresh refusal altered interrupted report')
            verdict['phases']['fresh_refusal_after_crash']='PASS'
            check_review(app.pid,'Check model')
            running = observed(path,lambda r:r[0]!=interrupted[0] and r[1]=='running','running model check')
            start=time.monotonic();action(app.pid,'Stop',timeout=4)
            stopped = observed(path,lambda r:r[0]==running[0] and r[1]=='stopped','durable stop',40)
            require(stopped[2]['reason']=='user_stopped' and len(stopped[2]['results'])<len(KINDS),'stop did not prevent unfinished credit')
            verify_report(stopped)
            verdict['phases']['stop']={'status':'PASS','elapsed_seconds':time.monotonic()-start,'verified_prefix':len(stopped[2]['results']),'same_worker_retained':model_worker(app.pid)==worker}
            capture(app.pid,a.out,'stopped')
            # Cold reopening an already stopped report must not replay it or
            # automatically load a model. Explicitly reload and reject the new review.
            app.terminate()
            app.wait(timeout=15)
            require(journal(path)==stopped, 'shutdown altered stopped report')
            app = subprocess.Popen([str(exe)], env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            navigate(app.pid)
            require(model_worker(app.pid) is None and journal(path)==stopped, 'restart replayed model check or loaded model')
            verdict['phases']['cold_reopen_no_replay']='PASS'
            action(app.pid, 'Chat')
            reloaded = load(app.pid)
            require(reloaded!=worker and journal(path)==stopped, 'explicit reload altered report or reused worker')
            navigate(app.pid)
            check_review(app.pid,'Cancel')
            wait(lambda: named(app.pid, 'Check loaded model…', actionable=True), 15, 'new refusal returned idle')
            require(journal(path)==stopped and model_worker(app.pid)==reloaded,'fresh refusal altered report or model')
            verdict['phases']['fresh_refusal']='PASS'
            capture(app.pid,a.out,'fresh-refusal')
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
