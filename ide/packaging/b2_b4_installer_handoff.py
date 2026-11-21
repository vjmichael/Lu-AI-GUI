#!/usr/bin/env python3
"""Read-only same-PR installer transport; not an acceptance/release receipt.

Both lanes retain their original build as fallback and independent OS witness.
A lane can use the other lane's artifact only after its original build, legal
check and upload steps succeeded. Neither trusts the other installed result.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

from verify_p04_license_bundle import audit, sha
from verify_p06_checkout_lock import verify_checkout

SCHEMA = 'lu.ide.b2-b4.installer-handoff.v1'
ROOT = Path('ide/app/src-tauri')
MANIFEST = 'b2-b4-installer-handoff.json'
RECEIPT = 'verification-package-receipt.json'
NOTICES = 'verification-notices'
MAX_ZIP = 1024 * 1024 * 1024
MAX_ENTRIES = 12000
PLATFORMS = {
    'windows-2022': ('windows', 'nsis', 'tauri.installed-smoke.conf.json', '*setup.exe', 'installed-unsigned-native-ui', 'Build real default-disabled unsigned Windows NSIS', 'Verify all collected legal bytes against exact unsigned NSIS hash', 'Attach unsigned Windows NSIS and matching resolved license bundle'),
    'ubuntu-24.04': ('linux', 'deb', 'tauri.wo8.linux.conf.json', '*.deb', 'linux-installed-deb', 'Build frontend and real native Linux Debian package', 'Verify legal bytes and bind inventory to real Debian package digest', 'Attach unsigned Debian and matching resolved dependency licenses'),
}
HEX40 = re.compile(r'[0-9a-f]{40}')
HEX64 = re.compile(r'[0-9a-f]{64}')

class Denied(ValueError): pass
class Unavailable(Exception): pass


def require(test, reason):
    if not test:
        raise Denied(reason)


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8'))


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], text=True, timeout=15).strip()


def context(repo, platform, env):
    source = env['SOURCE_SHA']
    require(HEX40.fullmatch(source) and git(repo, 'rev-parse', 'HEAD') == source,
            'checkout_not_exact_head')
    require(not git(repo, 'diff', 'HEAD', '--name-only'), 'tracked_source_changed')
    event = read_json(Path(env['GITHUB_EVENT_PATH']))
    pr = event.get('pull_request', {})
    rid = int(env['GITHUB_REPOSITORY_ID'])
    same_pr = (env['GITHUB_EVENT_NAME'] == 'pull_request' and
               pr.get('head', {}).get('repo', {}).get('id') == rid and
               pr.get('base', {}).get('repo', {}).get('id') == rid and
               pr.get('head', {}).get('sha') == source)
    plat = PLATFORMS[platform]
    require(env['RUNNER_OS'] == ('Windows' if platform == 'windows-2022' else 'Linux'), 'runner_platform_mismatch')
    return {'sourceSha':source, 'sourceTree':git(repo, 'rev-parse', 'HEAD^{tree}'),
            'repositoryId':rid, 'pullRequest':pr.get('number') if same_pr else None,
            'platform':platform, 'runnerOS':env['RUNNER_OS'], 'runnerArch':env['RUNNER_ARCH'],
            'imageOS':env.get('ImageOS', ''), 'imageVersion':env.get('ImageVersion', ''),
            'recipe':plat[2], 'recipeSha256':sha(repo/ROOT/plat[2]),
            'cargoLockSha256':sha(repo/ROOT/'Cargo.lock'),
            'npmLockSha256':sha(repo/'ide/app/package-lock.json'),
            'tauriCli':'2.12.4', 'rustToolchain':'1.99.1', 'nodeMajor':'22',
            'releaseApproved':False, 'installedAcceptance':False}


def package(base, platform, target=True):
    plat = PLATFORMS[platform]
    folder = base / ('target/release/bundle' if target else 'bundle') / plat[1]
    files = list(folder.glob(plat[3]))
    require(len(files) == 1 and files[0].is_file() and not files[0].is_symlink(), 'installer_not_unique')
    return files[0]


def verify_payload(base, installer, ctx):
    require(installer.stat().st_size > 0 and installer.stat().st_size <= MAX_ZIP, 'installer_size')
    checked = audit(base/NOTICES, ctx['sourceSha'], require_upstream=True)
    checked.update(installerSha256=sha(installer), installerName=installer.name)
    require(read_json(base/RECEIPT) == checked, 'original_package_receipt_mismatch')
    inventory = read_json(base/NOTICES/'inventory.json')
    locks = {x['manager']:x['sha256'] for x in inventory['dependencyLocks']}
    require(locks == {'cargo':ctx['cargoLockSha256'], 'npm':ctx['npmLockSha256']}, 'notice_lock_mismatch')
    return {'installerName':installer.name, 'installerSha256':checked['installerSha256'],
            'installerBytes':installer.stat().st_size, 'packageReceiptSha256':sha(base/RECEIPT),
            'noticeInventorySha256':sha(base/NOTICES/'inventory.json')}


def record(repo, platform, env, lane):
    ctx = context(repo, platform, env)
    base = repo/ROOT
    verify_checkout(repo, platform)
    info = verify_payload(base, package(base, platform), ctx)
    proof = {**ctx, **info, 'schema':SCHEMA, 'producerRunId':int(env['GITHUB_RUN_ID']),
             'producerAttempt':int(env['GITHUB_RUN_ATTEMPT']), 'producerWorkflow':producer_path(platform,lane)}
    with (base/MANIFEST).open('x', encoding='utf-8') as f:
        json.dump(proof, f, indent=2, sort_keys=True)
        f.write('\n')


def producer_path(platform,lane='b2'):
    return '.github/workflows/ide-'+lane+'-installed-editor-' + PLATFORMS[platform][0] + '.yml'


def artifact_name(platform):
    return 'lu-ide-p04-' + PLATFORMS[platform][0] + '-unsigned-verification'


def matching_run(run, ctx, platform,lane='b2'):
    if not (run.get('path') == producer_path(platform,lane) and run.get('event') == 'pull_request'
            and run.get('head_sha') == ctx['sourceSha'] and run.get('run_attempt') == 1
            and run.get('conclusion') not in ('cancelled', 'startup_failure', 'stale', 'action_required')
            and run.get('repository', {}).get('id') == ctx['repositoryId']
            and run.get('head_repository', {}).get('id') == ctx['repositoryId']
            and type(run.get('id')) is int and run['id'] > 0):
        return False
    return any(p.get('number') == ctx['pullRequest'] and p.get('head', {}).get('sha') == ctx['sourceSha']
               and p.get('head', {}).get('repo', {}).get('id') == ctx['repositoryId']
               and p.get('base', {}).get('repo', {}).get('id') == ctx['repositoryId']
               for p in run.get('pull_requests', []))


def exact_job(jobs, run, platform):
    matches = [j for j in jobs if j.get('name') == PLATFORMS[platform][4] and j.get('head_sha') == run['head_sha']
               and j.get('run_id') == run['id'] and j.get('run_attempt') == 1 and platform in j.get('labels', [])]
    return matches[0] if len(matches)==1 else None


def producer_uploaded(jobs, run, platform):
    plat = PLATFORMS[platform]
    job = exact_job(jobs,run,platform)
    if job is None: return False
    steps = job.get('steps', [])
    names = ['Prove exact source checkout before installed build' if platform == 'windows-2022' else 'Verify exact source checkout for native installed package',
             'Validate frontend and Rust host contract' if platform == 'windows-2022' else 'Validate frontend before package admission',
             'Fetch resolved Cargo sources and embed third-party license notices' if platform == 'windows-2022' else 'Fetch resolved Cargo sources before collecting license notices',
             plat[5], plat[6], 'Record exact-source B2/B4 package handoff (not installed acceptance)', plat[7]]
    selected = [[s for s in steps if s.get('name') == n] for n in names]
    return (all(len(s) == 1 and s[0].get('conclusion') == 'success' for s in selected)
            and all(selected[i][0]['number'] < selected[i+1][0]['number'] for i in range(len(selected)-1)))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Api:
    def __init__(self, repository, token):
        require(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository), 'repository_invalid')
        self.base = 'https://api.github.com/repos/' + repository + '/actions/'
        self.headers = {'Authorization':'Bearer '+token, 'Accept':'application/vnd.github+json',
                        'X-GitHub-Api-Version':'2022-11-28'}
        self.opener = urllib.request.build_opener(NoRedirect())

    def json(self, suffix):
        try:
            with self.opener.open(urllib.request.Request(self.base+suffix, headers=self.headers), timeout=15) as r:
                raw = r.read(8*1024*1024+1)
                require(len(raw)<=8*1024*1024, 'api_response_too_large')
                return json.loads(raw)
        except (urllib.error.URLError, TimeoutError) as e:
            raise Unavailable('actions_metadata_unavailable') from e

    def download(self, artifact, destination):
        try:
            self.opener.open(urllib.request.Request(self.base+f'artifacts/{artifact["id"]}/zip', headers=self.headers), timeout=20)
        except urllib.error.HTTPError as e:
            require(e.code in (302, 303, 307) and e.headers.get('Location'), 'artifact_download_redirect_missing')
            location = e.headers['Location']
            e.close()
        else:
            raise Denied('unexpected_artifact_download_response')
        target = urllib.parse.urlsplit(location)
        require(target.scheme == 'https' and not target.username and not target.password and
                target.port in (None,443) and target.hostname and
                any(target.hostname.endswith(s) for s in ('.blob.core.windows.net','.actions.githubusercontent.com','.githubusercontent.com')),
                'untrusted_artifact_download_host')
        # No GitHub API token on the signed storage request; no further redirects.
        total = 0
        digest = hashlib.sha256()
        deadline = time.monotonic()+240
        with self.opener.open(urllib.request.Request(location), timeout=45) as r, destination.open('xb') as f:
            while True:
                require(time.monotonic() <= deadline, 'artifact_download_deadline')
                raw = r.read(1024*1024)
                if not raw: break
                total += len(raw)
                require(total <= MAX_ZIP, 'artifact_download_too_large')
                digest.update(raw)
                f.write(raw)
        require('sha256:'+digest.hexdigest() == artifact['digest'], 'transport_digest_mismatch')


def producer_state(api, ctx, platform,lane):
    query = urllib.parse.urlencode({'event':'pull_request','head_sha':ctx['sourceSha'],'per_page':100})
    data = api.json('workflows/'+PurePosixPath(producer_path(platform,lane)).name+'/runs?'+query)
    if data.get('total_count', 101)>100:
        return None,None,None
    runs = [r for r in data.get('workflow_runs', []) if matching_run(r, ctx, platform,lane)]
    if len(runs) != 1:
        return None,None,None
    run = runs[0]
    jobs = api.json(f'runs/{run["id"]}/attempts/1/jobs?per_page=100')
    if jobs.get('total_count', 101)>100:
        return run,None,None
    rows=jobs.get('jobs',[])
    job=exact_job(rows,run,platform)
    if not producer_uploaded(rows, run, platform):
        return run,job,None
    data = api.json(f'runs/{run["id"]}/artifacts?per_page=100')
    if data.get('total_count', 101)>100:
        return run,job,None
    items = [a for a in data.get('artifacts', []) if a.get('name') == artifact_name(platform)]
    if len(items) != 1:
        return run,job,None
    a = items[0]
    if a.get('expired') is not False or not isinstance(a.get('digest'), str) or not re.fullmatch(r'sha256:[0-9a-f]{64}', a['digest']):
        return run,job,None  # No digest, no reuse; original build remains mandatory.
    require(type(a.get('id')) is int and a['id']>0 and type(a.get('size_in_bytes')) is int and 0<a['size_in_bytes']<=MAX_ZIP,
            'artifact_identity_invalid')
    ar = a.get('workflow_run') or {}
    require(ar.get('id') == run['id'] and ar.get('head_sha') == ctx['sourceSha'] and
            ar.get('repository_id') == ctx['repositoryId'] and ar.get('head_repository_id') == ctx['repositoryId'],
            'artifact_run_binding_mismatch')
    return run,job,a


def find_artifact(api,ctx,platform,lane='b2'):
    run,_,artifact=producer_state(api,ctx,platform,lane)
    return (run,artifact) if artifact else None


def should_wait(api,ctx,platform,run,job,own_run_id):
    # Never occupy a runner waiting for a queued (possibly slot-blocked) job.
    # A strictly earlier started job is the sole permitted wait dependency.
    if not run or not job or run.get('status')!='in_progress' or job.get('status')!='in_progress':
        return False
    steps=job.get('steps',[])
    if any(s.get('conclusion') in ('failure','cancelled','skipped') for s in steps if s.get('name') in PLATFORMS[platform][5:]):
        return False
    if producer_uploaded([job],run,platform):
        return False  # no fresh/usable digest despite upload; build locally
    own=api.json(f'runs/{own_run_id}/attempts/1/jobs?per_page=100')
    if own.get('total_count',101)>100:return False
    current=exact_job(own.get('jobs',[]),{'id':own_run_id,'head_sha':ctx['sourceSha']},platform)
    start=job.get('started_at'); this=current.get('started_at') if current else None
    # GitHub UTC timestamps sort lexically. Run ID breaks an equal-time tie.
    if not all(isinstance(v,str) and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z',v) for v in (start,this)):
        return False
    return (start,run['id']) < (this,own_run_id)


def extract(archive, destination, platform):
    # Fixed upload-root layout from the existing upload-artifact LCA (src-tauri).
    # No extractall, links, case collisions, ADS, executable outside bundle, or path escapes.
    prefix = 'target/release/bundle/'+PLATFORMS[platform][1]+'/'
    seen = set()
    total = 0
    with zipfile.ZipFile(archive) as z:
        entries = z.infolist()
        require(0<len(entries)<=MAX_ENTRIES, 'archive_entry_count')
        for info in entries:
            name = info.filename
            parts = name.rstrip('/').split('/')
            require(name and not name.startswith('/') and all(p not in ('','.','..') and not p.endswith((' ','.'))
                    and not re.search(r'[\\:\x00-\x1f<>"|?*]',p) and not re.fullmatch(r'(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?',p) for p in parts), 'archive_path_invalid')
            normalized = '/'.join(parts)
            require(normalized.casefold() not in seen, 'archive_duplicate_path')
            seen.add(normalized.casefold())
            mode = stat.S_IFMT(info.external_attr >> 16)
            require(mode in (0, stat.S_IFREG, stat.S_IFDIR) and not info.flag_bits & 1, 'archive_link_or_encrypted')
            if info.is_dir():
                require(mode in (0,stat.S_IFDIR) and (prefix.startswith(normalized+'/') or normalized==NOTICES or normalized.startswith(NOTICES+'/')), 'archive_unexpected_directory')
                continue
            require(mode in (0,stat.S_IFREG) and (normalized in (MANIFEST, RECEIPT) or normalized.startswith(NOTICES+'/') or
                    (normalized.startswith(prefix) and '/' not in normalized[len(prefix):])), 'archive_unexpected_file')
            total += info.file_size
            require(total<=MAX_ZIP, 'archive_expanded_size')
            out = destination.joinpath(*parts)
            out.parent.mkdir(parents=True,exist_ok=True)
            with z.open(info) as src, out.open('xb') as dst:
                shutil.copyfileobj(src,dst,1024*1024)
    require((destination/MANIFEST).is_file() and (destination/RECEIPT).is_file(), 'archive_missing_receipt')
    installer = package(destination,platform)
    require(len(list(installer.parent.iterdir()))==1,'archive_extra_bundle_file')
    return installer


def accept_payload(staged, installer, ctx, run, platform,lane='b2'):
    proof = read_json(staged/MANIFEST)
    # Runner image mismatch is a normal compatibility miss. No artifact may be installed.
    if any(proof.get(key) != ctx[key] for key in ('imageOS','imageVersion')):
        raise Unavailable('runner_image_changed')
    info = verify_payload(staged, installer, ctx)
    expected = {**ctx, **info, 'schema':SCHEMA, 'producerRunId':run['id'],
                'producerAttempt':1, 'producerWorkflow':producer_path(platform,lane)}
    require(proof == expected, 'producer_manifest_mismatch')


def consume(repo, platform, env, wait,lane):
    ctx = context(repo,platform,env)
    verify_checkout(repo, platform)
    if not ctx['pullRequest'] or env['GITHUB_RUN_ATTEMPT'] != '1' or not ctx['imageOS'] or not ctx['imageVersion'] or ctx['runnerArch'] != 'X64' or not env.get('GH_TOKEN'):
        raise Unavailable('standalone_or_unmatched_runner')
    api = Api(env['GITHUB_REPOSITORY'],env['GH_TOKEN'])
    other='b4' if lane=='b2' else 'b2'
    deadline = time.monotonic()+wait
    while True:
        run,job,artifact=producer_state(api,ctx,platform,other)
        if artifact: break
        if time.monotonic()>=deadline or not should_wait(api,ctx,platform,run,job,int(env['GITHUB_RUN_ID'])):
            raise Unavailable('no_ready_exact_producer')
        time.sleep(min(10,max(0,deadline-time.monotonic())))
    with tempfile.TemporaryDirectory(prefix='ide-b2-b4-',dir=env['RUNNER_TEMP']) as tmp:
        tmp = Path(tmp)
        archive = tmp/'artifact.zip'
        api.download(artifact,archive)
        staged = tmp/'payload'
        staged.mkdir()
        installer = extract(archive,staged,platform)
        accept_payload(staged,installer,ctx,run,platform,other)
        base = repo/ROOT
        target = base/installer.relative_to(staged)
        require(not target.parent.exists() and not (base/NOTICES).exists() and not (base/RECEIPT).exists(), 'consumer_output_already_exists')
        target.parent.mkdir(parents=True)
        shutil.copyfile(installer,target)
        shutil.copytree(staged/NOTICES,base/NOTICES)
        # Consumer independently re-runs original P04 audit to generate its own receipt.
        require(sha(target)==sha(installer),'consumer_copy_digest_mismatch')
        (Path(env['RUNNER_TEMP'])/'ide-b2-b4-producer.json').write_text(json.dumps({
            'sourceSha':ctx['sourceSha'],'artifactId':artifact['id'],'artifactDigest':artifact['digest'],
            'producerRunId':run['id'],'producerAttempt':1,'producerWorkflow':producer_path(platform,other),'installerSha256':sha(target),
            'installedAcceptance':False,'releaseApproved':False},sort_keys=True,indent=2)+'\n',encoding='utf-8')
    return True


def main():
    p = argparse.ArgumentParser()
    p.add_argument('operation',choices=('record','consume'))
    p.add_argument('--platform',choices=PLATFORMS,required=True)
    p.add_argument('--lane',choices=('b2','b4'),required=True)
    p.add_argument('--repo-root',type=Path,default=Path('.'))
    p.add_argument('--wait-seconds',type=int,choices=range(0,241),default=180)
    args=p.parse_args()
    try:
        if args.operation=='record':
            record(args.repo_root.resolve(),args.platform,os.environ,args.lane)
        else:
            try:
                consume(args.repo_root.resolve(),args.platform,os.environ,args.wait_seconds,args.lane)
                reused='true'
            except Unavailable as e:
                print('B2_B4_REUSE_UNAVAILABLE: '+str(e)); reused='false'
            with Path(os.environ['GITHUB_OUTPUT']).open('a',encoding='utf-8') as f:
                f.write('reused='+reused+'\n')
            print('B2_B4_INSTALLER_REUSED='+reused+'; INSTALLED_ACCEPTANCE=NOT_TESTED')
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, zipfile.BadZipFile, urllib.error.URLError) as e:
        # Avoid printing signed URLs or credentials from transport errors.
        print('DENIED: '+(str(e) if isinstance(e,Denied) else type(e).__name__))
        return 2
    return 0

if __name__=='__main__':
    raise SystemExit(main())
