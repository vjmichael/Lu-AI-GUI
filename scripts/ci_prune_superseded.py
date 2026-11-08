#!/usr/bin/env python3
"""Cancel only obsolete queued GH Actions runs; preserve final-head and running jobs."""
import collections
import datetime
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

ELIGIBLE = {'queued', 'pending', 'requested', 'waiting'}
EVENTS = {'pull_request', 'push'}

def candidates(runs, heads, now, limit=25):
    groups = collections.defaultdict(list)
    for r in runs:
        if (r.get('event') in EVENTS and r.get('head_branch')
                and isinstance(r.get('workflow_id'), int) and r.get('head_sha')):
            groups[(r['workflow_id'], r['head_branch'], r['event'])].append(r)
    result = []
    for group in groups.values():
        group.sort(key=lambda r: (r['created_at'], r['id']), reverse=True)
        newest = group[0]
        head = heads.get(newest['head_branch'])
        if (not head or newest['head_sha'] != head
                or newest['status'] not in ELIGIBLE | {'in_progress', 'completed'}
                or newest.get('conclusion') in {'cancelled', 'failure'}):
            continue
        for old in group[1:]:
            if (old['status'] not in ELIGIBLE or old['head_sha'] == head
                    or old['head_sha'] == newest['head_sha']):
                continue
            age = now - datetime.datetime.fromisoformat(old['created_at'].replace('Z', '+00:00')).timestamp()
            if age < 1800:
                continue
            if newest['event'] == 'pull_request':
                previous = {p['number'] for p in old.get('pull_requests', []) if 'number' in p}
                current = {p['number'] for p in newest.get('pull_requests', []) if 'number' in p}
                if not previous & current:
                    continue
            result.append({'old': old['id'], 'new': newest['id'], 'branch': newest['head_branch'],
                           'old_sha': old['head_sha'], 'head_sha': head})
    return sorted(result, key=lambda x: x['old'])[:limit]

def run():
    repo = os.environ['GITHUB_REPOSITORY']
    token = os.environ['GITHUB_TOKEN']
    if len(repo.split('/')) != 2:
        raise RuntimeError('Bad repository scope')
    apply = '--apply' in sys.argv
    base = 'https://api.github.com/repos/' + repo
    def api(path, method='GET'):
        request = urllib.request.Request(base + path, method=method, headers={
            'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
            'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'Lu-CI-Queue-01'})
        with urllib.request.urlopen(request, timeout=25) as response:
            return json.loads(response.read()) if method == 'GET' else None
    runs = []
    for page in range(1, 6):
        batch = api(f'/actions/runs?per_page=100&page={page}')['workflow_runs']
        runs.extend(batch)
        if len(batch) < 100:
            break
    heads = {}
    branches = {r['head_branch'] for r in runs if r.get('status') in ELIGIBLE and r.get('head_branch')}
    for branch in branches:
        try:
            record = api('/branches/' + urllib.parse.quote(branch, safe=''))
            heads[branch] = record['commit']['sha']
        except (urllib.error.HTTPError, KeyError):
            print('SKIP_UNRESOLVED_BRANCH', branch)
    selection = candidates(runs, heads, datetime.datetime.now(datetime.timezone.utc).timestamp())
    print('CI_QUEUE_AUDIT', json.dumps({'inspected': len(runs), 'heads': len(heads),
                                      'superseded': len(selection), 'apply': apply}))
    requested = 0
    for item in selection:
        if not apply:
            print('WOULD_CANCEL', json.dumps(item))
            continue
        try:
            api('/actions/runs/' + str(item['old']) + '/cancel', method='POST')
            requested += 1
            print('CANCEL_REQUEST_ACCEPTED', json.dumps(item))
        except urllib.error.HTTPError as exc:
            print('NOT_CANCELLED', item['old'], exc.code)
    summary = ('Lu CI queue: inspected %d runs, %d superseded queued candidates, '
               '%d cancel requests accepted. In-progress and current-head runs never selected.\n'
               % (len(runs), len(selection), requested))
    print(summary)
    if os.getenv('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf8') as out:
            out.write(summary)

if __name__ == '__main__':
    run()
