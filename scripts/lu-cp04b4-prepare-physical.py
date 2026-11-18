#!/usr/bin/env python3
"""Prepare exact CP-04B3R3 artifacts for physical Windows acceptance.

Offline inspection/extraction only. Does not run downloaded code, approve a
browser, contact any provider, change the repository, or claim live acceptance.
"""
import argparse
import hashlib
import json
import re
import shutil
import stat
import subprocess
import zipfile
from pathlib import Path

SOURCE = '2ea78b23963830de90a2a9951d00c674dc13ef06'
BROWSER_ARTIFACT = 14414415865
BROWSER_ZIP = '15756d62a692f73b5da8b9f2adf63e7fa6b9aafdc54f3b5cbc51735c6c0f942f'
MANIFEST = 'f511bfa8a1b01530a41915f64ae018b28575f3dd8fd45eb660f584895383f97f'
WORKER = 'c9ea3d8a6a6fe5d9864b095e1276c4284c296dcaedf8692b57cc958ecbf63dd9'
ENGINE_ARTIFACT = 14409181478
ENGINE_ZIP = 'f47145fe94b88188b0625fb6a043b6b154f9ce67d8e060ab00fc02b82c9d340b'
ENGINE = '48a3b2a09e5bc732d97f061a7de04a556b6dc26b9bbd30a995eb4636d52ebd53'
MODEL = '7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d'
MODEL_NAME = 'Qwen3-0.6B.Q4_K_M.gguf'
REPO = Path(__file__).resolve().parent.parent
HEX = re.compile(r'[a-f0-9]{64}\Z')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(stream):
    h = hashlib.sha256()
    for chunk in iter(lambda: stream.read(1024 * 1024), b''):
        h.update(chunk)
    return h.hexdigest()


def file_digest(path):
    with path.open('rb') as stream:
        return digest(stream)


def read_json(archive, name):
    require(archive.getinfo(name).file_size <= 2_000_000, 'Unbounded metadata: ' + name)
    return json.loads(archive.read(name))


def index(archive):
    """Reject path aliases, special files and unexpected archive structure."""
    files, seen = {}, set()
    for entry in archive.infolist():
        name = entry.filename.rstrip('/') if entry.is_dir() else entry.filename
        parts = name.split('/')
        require(len(name) < 512 and all(
            re.fullmatch(r'[A-Za-z0-9._ -]+', p) and p not in ('.', '..')
            and p == p.strip() and not p.endswith('.')
            and not re.fullmatch(r'(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])', p.split('.')[0], re.I)
            for p in parts), 'Unsafe ZIP member: ' + name)
        mode = stat.S_IFMT(entry.external_attr >> 16)
        require(mode in (0, stat.S_IFDIR if entry.is_dir() else stat.S_IFREG), 'Nonordinary ZIP member: ' + name)
        key = name.casefold()
        require(key not in seen, 'Duplicate or case-alias ZIP member: ' + name)
        seen.add(key)
        if not entry.is_dir():
            files[name] = entry
    folded_files = {n.casefold() for n in files}
    for name in files:
        parts = name.split('/')
        require(all('/'.join(parts[:i]).casefold() not in folded_files
                    for i in range(1, len(parts))), 'File/directory collision: ' + name)
    return files


def verify_payload(archive, files, path, size, sha):
    require(isinstance(sha, str) and HEX.fullmatch(sha), 'Invalid digest: ' + path)
    require(type(size) is int and 0 <= size <= 2_000_000_000, 'Invalid file size: ' + path)
    require(path in files and files[path].file_size == size, 'File size mismatch: ' + path)
    with archive.open(files[path]) as stream:
        require(digest(stream) == sha, 'File digest mismatch: ' + path)


def verify_browser(archive):
    files = index(archive)
    require(files['manifest.json'].file_size <= 2_000_000, 'Unbounded browser manifest')
    raw = archive.read('manifest.json')
    require(hashlib.sha256(raw).hexdigest() == MANIFEST, 'Wrong browser candidate manifest')
    m = json.loads(raw)
    require(m['schema'] == 1 and m['runtime'] == 'lu-browser' and m['status'] == 'candidate'
            and m['platform'] == 'win32' and m['arch'] == 'x64' and m['playwrightVersion'] == '1.64.1',
            'Unexpected browser runtime or approval state')
    entries = {f['path']: f for f in m['files']}
    require(len(m['files']) == len(entries) == 482, 'Browser payload count mismatch')
    require(set(files) == set(entries) | {'manifest.json', 'manifest.sha256', 'candidate-summary.json'},
            'Browser archive has missing or undeclared files')
    required = {'chrome.exe', 'node.exe', 'lu_browser_worker.mjs', 'node_modules/playwright/index.mjs',
                'node_modules/playwright-core/index.mjs'}
    require(required <= entries.keys(), 'Missing genuine browser entrypoints')
    require(entries['lu_browser_worker.mjs']['sha256'] == WORKER, 'Stale browser worker')
    summary = read_json(archive, 'candidate-summary.json')
    require(summary['status'] == 'candidate' and summary['files'] == 482
            and summary['manifestSha256'] == MANIFEST and summary['platform'] == 'win32'
            and summary['arch'] == 'x64' and summary['playwrightVersion'] == '1.64.1', 'Unexpected browser summary')
    require(files['manifest.sha256'].file_size <= 128 and archive.read('manifest.sha256').decode().strip() == MANIFEST,
            'Manifest sidecar mismatch')
    for path, f in entries.items():
        verify_payload(archive, files, path, f['bytes'], f['sha256'])
    return files


def verify_engine(archive):
    files = index(archive)
    require(set(files) == {'inputs.json', 'llama-server.exe', MODEL_NAME, 'smoke.engine.log', 'smoke.json'},
            'Unexpected engine archive contents')
    inputs, smoke = read_json(archive, 'inputs.json'), read_json(archive, 'smoke.json')
    require(inputs['schema'] == 'lu-cp04b3-engine-inputs-v1' and inputs['sourceSha'] == '2bc46baa9207d098d03741ac394e7faaf1a955b6'
            and inputs['engineSha256'] == ENGINE and inputs['modelSha256'] == MODEL, 'Wrong engine input identities')
    entries = {f['path']: f for f in inputs['files']}
    require(len(inputs['files']) == 2 and set(entries) == {'llama-server.exe', MODEL_NAME}, 'Unexpected engine payload')
    for path, sha in [('llama-server.exe', ENGINE), (MODEL_NAME, MODEL)]:
        require(entries[path]['sha256'] == sha, 'Wrong engine payload pin')
        verify_payload(archive, files, path, entries[path]['bytes'], sha)
    require(smoke['schema'] == 'lu-cp04b3-standalone-gguf-smoke-v1' and smoke['engineSha256'] == ENGINE
            and smoke['modelSha256'] == MODEL and smoke['generated'] is True and smoke['engineExited'] is True
            and smoke['providerVerified'] is False and smoke['protectedBrowserVerified'] is False
            and smoke['usage']['completion_tokens'] > 0
            and hashlib.sha256(smoke['answer'].encode()).hexdigest() == smoke['answerSha256'], 'Standalone smoke receipt mismatch')
    return files


def extract(archive, files, root):
    root.mkdir()
    for name, entry in files.items():
        target = root.joinpath(*name.split('/'))
        target.parent.mkdir(parents=True, exist_ok=True)
        with archive.open(entry) as src, target.open('xb') as dst:
            shutil.copyfileobj(src, dst, 1024 * 1024)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser-archive', required=True, type=Path)
    parser.add_argument('--engine-archive', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path, help='New local staging directory (must not exist)')
    args = parser.parse_args()
    subprocess.run(['node', 'scripts/verify-source-tree.mjs'], cwd=REPO, check=True)
    require(file_digest(REPO / 'src-tauri/lu-core/workers/lu_browser_worker.mjs') == WORKER, 'Source worker differs from artifact')
    output = args.output.resolve()
    require(not output.exists() and not output.is_relative_to(REPO), 'Requires a fresh output directory outside source checkout')
    # Check the archive bytes using the same open handles used for ZIP reading.
    with args.browser_archive.open('rb') as b, args.engine_archive.open('rb') as e:
        require(digest(b) == BROWSER_ZIP, 'Browser ZIP SHA-256 differs from original CI artifact')
        require(digest(e) == ENGINE_ZIP, 'Engine ZIP SHA-256 differs from original CI artifact')
        b.seek(0); e.seek(0)
        with zipfile.ZipFile(b) as browser, zipfile.ZipFile(e) as engine:
            browser_files = verify_browser(browser)
            engine_files = verify_engine(engine)
            output.mkdir(parents=True, exist_ok=False)
            try:
                extract(browser, browser_files, output / 'browser-candidate')
                extract(engine, engine_files, output / 'engine')
                require(file_digest(output / 'browser-candidate/manifest.json') == MANIFEST, 'Extracted manifest changed')
                m = json.loads((output / 'browser-candidate/manifest.json').read_bytes())
                for f in m['files']:
                    copied = (output / 'browser-candidate').joinpath(*f['path'].split('/'))
                    require(copied.stat().st_size == f['bytes'] and file_digest(copied) == f['sha256'],
                            'Extracted browser file changed: ' + f['path'])
                require(file_digest(output / 'engine/llama-server.exe') == ENGINE and file_digest(output / 'engine' / MODEL_NAME) == MODEL, 'Extracted engine/model changed')
                receipt = {'schema': 'lu-cp04b4-physical-preparation-v1', 'artifactSourceSha': SOURCE,
                           'checkoutSha': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
                           'browserArtifactId': BROWSER_ARTIFACT, 'browserZipSha256': BROWSER_ZIP,
                           'candidateManifestSha256': MANIFEST, 'workerSha256': WORKER, 'browserFileCount': 482,
                           'engineArtifactId': ENGINE_ARTIFACT, 'engineZipSha256': ENGINE_ZIP,
                           'engineSha256': ENGINE, 'modelSha256': MODEL,
                           'status': 'UNAPPROVED_BYTES_VERIFIED', 'providerVerified': False,
                           'protectedBrowserVerified': False, 'nativeSynthesisVerified': False}
                (output / 'preparation.json').write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf-8')
            except BaseException:
                shutil.rmtree(output)
                raise
    print('CP04B4_PHYSICAL_PREPARED_UNAPPROVED (no runtime execution or live acceptance)')


if __name__ == '__main__':
    main()
