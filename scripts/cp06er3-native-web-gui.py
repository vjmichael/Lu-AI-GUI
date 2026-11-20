#!/usr/bin/env python3
"""Disposable native Linux GUI acceptance, real accessibility actions, no test IPC.
Only fixed public requests leave this profile; no raw model tokens in receipts.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

FETCH = 'Fetch web page https://example.org/'
ACADEMIC = 'Search academic literature for photosynthesis'
SLOW = 'Fetch web page https://httpbin.org/delay/15'
REVIEW = 'Allow web access?'


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def wait(predicate, seconds, label):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        result = predicate()
        if result:
            return result
        time.sleep(.15)
    raise RuntimeError('missing independent witness: ' + label)


def proc_identity(pid):
    try:
        stat = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return (int(stat[1]), stat[19], Path(f'/proc/{pid}/exe').resolve(strict=True))
    except (OSError, ValueError, IndexError):
        return None


def owned(pid, owner):
    for _ in range(12):
        if pid == owner:
            return True
        data = proc_identity(pid)
        if not data or data[0] <= 1:
            return False
        pid = data[0]
    return False


def nodes(root):
    pending = [root]
    count = 0
    while pending and count < 4000:
        node = pending.pop(0)
        count += 1
        try:
            yield node
            pending.extend(n for i in range(min(node.get_child_count(), 250))
                           if (n := node.get_child_at_index(i)) is not None)
        except Exception:
            continue


def named(owner, name, root=None, actionable=False):
    found = []
    for node in nodes(root if root is not None else Atspi.get_desktop(0)):
        try:
            if node.get_name() != name or not owned(node.get_process_id(), owner):
                continue
            state = node.get_state_set()
            if not state.contains(Atspi.StateType.SHOWING):
                continue
            if actionable and not state.contains(Atspi.StateType.ENABLED):
                continue
            found.append(node)
        except Exception:
            continue
    return found[0] if len(found) == 1 else None


def action(owner, name, root=None, timeout=40):
    node = wait(lambda: named(owner, name, root, True), timeout, name)
    iface = node.get_action_iface()
    require(iface and iface.get_n_actions() > 0 and iface.do_action(0), 'native action failed: ' + name)


def send(owner, request):
    wait(lambda: named(owner, 'Cancel') is None and named(owner, 'Stopping…') is None, 150, 'ordinary Chat idle')
    node = wait(lambda: named(owner, 'Message Lu', actionable=True), 30, 'ordinary composer')
    component = node.get_component_iface()
    require(component and component.grab_focus(), 'composer focus refused')
    wait(lambda: node.get_state_set().contains(Atspi.StateType.FOCUSED), 5, 'composer focus')
    # Type via the actual focused desktop, never through application IPC.
    subprocess.run(['xdotool', 'type', '--clearmodifiers', '--', request], check=True, timeout=12)
    wait(lambda: node.get_text_iface().get_text(0, -1) == request, 5, 'exact composer text')
    action(owner, 'Send')


def dialog(owner, choice, expected):
    box = wait(lambda: named(owner, REVIEW), 40, 'actual native review dialog')
    # Require an actual visible X11 toplevel and its exact owning process.
    output = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Allow web access\\?$'], capture_output=True, text=True, timeout=8)
    windows = output.stdout.splitlines()
    require(output.returncode == 0 and len(windows) == 1, 'review window missing/ambiguous')
    pid = int(subprocess.check_output(['xdotool', 'getwindowpid', windows[0]], text=True, timeout=5))
    require(pid == owner, 'review window has wrong owner')
    texts = []
    for node in nodes(box):
        try:
            texts.append(node.get_name() or '')
            iface = node.get_text_iface()
            if iface:
                texts.append(iface.get_text(0, -1))
        except Exception:
            pass
    require(expected in '\n'.join(texts), 'review did not show exact public scope')
    require(not any(term in '\n'.join(texts) for term in ('lu_web_fetch', 'lu_web_academic_search', 'type_url', 'host_route_selected')), 'engineering details leaked into review')
    action(owner, choice, box)
    wait(lambda: named(owner, REVIEW) is None, 10, 'review dismissed')


def audit(path):
    if not path.is_file():
        return []
    try:
        with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, timeout=2) as con:
            return con.execute('SELECT sequence,conversation_id,task_id,kind,payload FROM lu_model_boundary_audit ORDER BY sequence').fetchall()
    except sqlite3.OperationalError:
        return []


def new_rows(path, after, prompt):
    all_rows = [r for r in audit(path) if r[0] > after]
    users = [r for r in all_rows if r[3] == 'USER_REQUEST' and r[4] == prompt]
    if len(users) != 1:
        return []
    return [r for r in all_rows if r[1:3] == users[0][1:3]]


def response(rows):
    return [json.loads(r[4])['response'] for r in rows if r[3] == 'TOOL_FEEDBACK']


def next_case(path):
    return max((r[0] for r in audit(path)), default=0)


def verify_live(owner, path, after, prompt, academic):
    def terminal():
        rows = new_rows(path, after, prompt)
        return rows if any(r[3] in ('FINAL_ANSWER', 'EVIDENCE_PRESENTATION', 'GENERATION_FAILURE') for r in rows) else None
    rows = wait(terminal, 150, 'durable native response')
    kinds = [r[3] for r in rows]
    require(sum(r[3] == 'HOST_ROUTE_SELECTED' and r[4] == 'HOST_NATIVE_APPROVAL_ONE_EXACT_PUBLIC_HTTPS_REQUEST' for r in rows) == 1, 'missing single native approval')
    require(any(r[3] == 'PARSED_CANDIDATE' and r[4] == ('lu_web_academic_search' if academic else 'lu_web_fetch') for r in rows), 'missing genuine model selection')
    results = response(rows)
    require(len(results) == 1 and results[0]['ok'] is True, 'live native result absent')
    result = results[0]['result']
    require(result['source_is_untrusted'] is True, 'untrusted source boundary absent')
    if academic:
        require(result['page'] == 1 and result['leads_verified'] is False and len(result['leads']) > 0, 'unverified academic leads absent')
    else:
        require(result['http_status'] == 200 and result['source_url'] == 'https://example.org/' and bool(result['content']), 'public response absent')
    require('GENERATION_FAILURE' not in kinds and ('FINAL_ANSWER' in kinds or 'EVIDENCE_PRESENTATION' in kinds), 'terminal presentation absent')
    def visible_report():
        history = named(owner, 'Conversation history, cursor paginated')
        if history is None:
            return False
        chunks = []
        for node in nodes(history):
            try:
                if not node.get_state_set().contains(Atspi.StateType.SHOWING):
                    continue
                chunks.append(node.get_name() or '')
                iface = node.get_text_iface()
                if iface:
                    chunks.append(iface.get_text(0, -1))
            except Exception:
                continue
        report = ' '.join(' '.join(chunks).split())
        required = ('OpenAlex returned', 'not verified citations') if academic else ('Retrieved HTTPS response (unverified source)', 'not independently verified factual or citation evidence')
        return all(x in report for x in required) and not any(x in report for x in ('lu_web_fetch', 'lu_web_academic_search', 'HOST_NATIVE_APPROVAL_ONE_EXACT_PUBLIC_HTTPS_REQUEST'))
    wait(visible_report, 30, 'user-visible plain-language source result')
    return {'task': rows[0][2], 'events': kinds, 'visible_user_result': True, 'native_feedback_sha256': hashlib.sha256(json.dumps(results[0], sort_keys=True).encode()).hexdigest()}


def owned_curl(owner, binary):
    for p in Path('/proc').iterdir():
        if p.name.isdecimal():
            identity = proc_identity(int(p.name))
            if identity and identity[0] == owner and identity[2] == binary:
                yield int(p.name), identity


def active_curl(owner, binary):
    # An actual connected TCP socket owned by the admitted curl child; this
    # does not claim TLS validation or a completed response before cancellation.
    output = subprocess.check_output(['ss', '-Htnp', 'state', 'established'], text=True, timeout=3)
    for pid, ident in owned_curl(owner, binary):
        if any(f'pid={pid},' in line and line.split()[3].endswith(':443') for line in output.splitlines() if len(line.split()) >= 5):
            args = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
            if b'https://httpbin.org/delay/15' in args:
                return (pid, ident)
    return None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--exe', type=Path, required=True)
    p.add_argument('--curl', type=Path, required=True)
    p.add_argument('--profile', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    require(os.geteuid() != 0, 'ordinary nonroot desktop required')
    require(args.exe.is_file() and args.curl.is_file() and args.profile.is_dir(), 'missing installed test inputs')
    require(args.exe.stat().st_uid == 0 and args.curl.stat().st_uid == 0, 'unprotected installed test inputs')
    args.out.mkdir(parents=True, exist_ok=False)
    path = args.profile / 'lu-model-web-research-audit.sqlite'
    require(not path.exists(), 'fresh disposable profile required')
    verdict = {'status': 'INCOMPLETE', 'phases': {}, 'operator': 'scripted native accessibility actions, not owner physical acceptance'}
    with (args.out / 'native.log').open('wb') as log:
        app = subprocess.Popen([str(args.exe)], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            owner = app.pid
            action(owner, 'Choose a model', timeout=120)
            action(owner, 'Load selected', timeout=60)
            wait(lambda: named(owner, 'Ready to chat'), 150, 'loaded local model')
            # First verify refusal; no network authority, model call or tool feedback.
            after = next_case(path)
            send(owner, FETCH)
            dialog(owner, 'Cancel', 'https://example.org/')
            rows = wait(lambda: new_rows(path, after, FETCH), 30, 'decline audit')
            require([r[3] for r in rows] == ['USER_REQUEST', 'REJECTED'] and rows[-1][4] == 'HUMAN_DECLINED_NETWORK', 'decline not effect-free')
            require(not list(owned_curl(owner, args.curl)), 'network child after decline')
            verdict['phases']['deny'] = 'PASS'
            for name, prompt, scope, academic in [('fetch', FETCH, 'https://example.org/', False), ('academic', ACADEMIC, 'photosynthesis', True)]:
                after = next_case(path)
                send(owner, prompt)
                dialog(owner, 'Allow once', scope)
                verdict['phases'][name] = verify_live(owner, path, after, prompt, academic)
            after = next_case(path)
            send(owner, SLOW)
            dialog(owner, 'Allow once', 'https://httpbin.org/delay/15')
            pid, identity = wait(lambda: active_curl(owner, args.curl), 38, 'real active public TCP connection')
            start = time.monotonic()
            action(owner, 'Cancel', timeout=3)
            wait(lambda: proc_identity(pid) != identity, 5, 'original curl process reaped')
            rows = wait(lambda: (r if any(e[3] == 'GENERATION_FAILURE' for e in r) else None) if (r := new_rows(path, after, SLOW)) else None, 20, 'durable cancelled turn')
            require(not any(r[3] in ('FINAL_ANSWER', 'EVIDENCE_PRESENTATION') for r in rows) and not any(v['ok'] is True for v in response(rows)), 'cancelled turn falsely claimed success')
            wait(lambda: named(owner, 'Cancel') is None and named(owner, 'Stopping…') is None, 30, 'cancelled Chat idle')
            verdict['phases']['active_cancel'] = {'status': 'PASS', 'connected_original_child': True, 'reaped_within_seconds': time.monotonic() - start}
            # Close genuine native window rather than fabricate a persistence result.
            windows = subprocess.check_output(['xdotool', 'search', '--onlyvisible', '--pid', str(owner)], text=True, timeout=5).splitlines()
            main_windows = [w for w in windows if subprocess.check_output(['xdotool', 'getwindowname', w], text=True, timeout=5).strip() == 'Lu']
            require(len(main_windows) == 1, 'main window identity unavailable')
            before = audit(path)
            subprocess.run(['xdotool', 'windowclose', main_windows[0]], check=True, timeout=5)
            require(app.wait(timeout=30) == 0, 'native close failed')
            app = subprocess.Popen([str(args.exe)], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            wait(lambda: named(app.pid, 'Choose a model'), 100, 'native cold restart')
            require(audit(path) == before and not list(owned_curl(app.pid,args.curl)), 'restart replay or journal mutation')
            verdict['phases']['restart'] = 'PASS'
            verdict['status'] = 'PASS'
        except Exception as exc:
            verdict['failure'] = str(exc)[:300]
            raise
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    app.kill(); app.wait(timeout=5)
            (args.out / 'receipt.json').write_text(json.dumps(verdict, indent=2) + '\n')
    print('CP06ER3_NATIVE_GUI_PASS')

if __name__ == '__main__':
    main()
