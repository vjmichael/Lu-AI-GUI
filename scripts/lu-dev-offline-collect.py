#!/usr/bin/env python3
"""CP-DEV-01C: collect exact public dependencies; never install or compile Lu."""
import argparse, base64, concurrent.futures, hashlib, json, os, pathlib, re, shutil, subprocess, tempfile, tomllib, urllib.parse, urllib.request

SOURCE = 'fc0c2e3f5e457c32d66276a84b2ec40d09e60c34'
TREE = 'cef4cf77290e6c6f3bbbfce86dde51aa7f07dd48'
LOCKS = ('src-tauri/lu-core/Cargo.lock', 'lu-desktop/src-tauri/Cargo.lock',
         'src-tauri/lu-win-worker-job/Cargo.lock', 'lu-desktop/tests/native-file-scan/Cargo.lock')
NPM = 'lu-desktop/package-lock.json'
MODEL = 'Qwen3-0.6B.Q4_K_M.gguf'
MODEL_SHA = '7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d'
MODEL_URL = 'https://huggingface.co/QuantFactory/Qwen3-0.6B-GGUF/resolve/main/' + MODEL


def digest(p, algorithm='sha256'):
    with p.open('rb') as f:
        return hashlib.file_digest(f, algorithm).hexdigest()


def run(*cmd, cwd=None, env=None):
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def lock_inventory(root):
    crates = {}
    for lock in LOCKS:
        for pkg in tomllib.loads((root / lock).read_text())['package']:
            source = pkg.get('source')
            if source is None:
                continue
            if source != 'registry+https://github.com/rust-lang/crates.io-index':
                raise ValueError('unexpected dependency registry')
            name, version, sha = pkg['name'], pkg['version'], pkg['checksum']
            if not re.fullmatch(r'[a-zA-Z0-9_-]+', name) or not re.fullmatch(r'[a-zA-Z0-9.+_-]+', version) or not re.fullmatch(r'[a-f0-9]{64}', sha):
                raise ValueError('invalid crate identity')
            filename = f'{name}-{version}.crate'
            if filename in crates and crates[filename] != sha:
                raise ValueError('conflicting crate checksums')
            crates[filename] = sha
    npm = {}
    for path, pkg in json.loads((root / NPM).read_text())['packages'].items():
        if not path:
            continue
        if ('os' in pkg and 'linux' not in pkg['os']) or ('cpu' in pkg and 'x64' not in pkg['cpu']):
            continue
        url = pkg['resolved']; u = urllib.parse.urlsplit(url)
        if u.scheme != 'https' or u.netloc != 'registry.npmjs.org' or u.query or u.fragment:
            raise ValueError('unexpected npm tarball registry')
        filename = pathlib.PurePosixPath(u.path).name
        if not re.fullmatch(r'[a-zA-Z0-9._-]+\.tgz', filename):
            raise ValueError('invalid npm archive name')
        algo, b64 = pkg['integrity'].split('-', 1)
        if algo != 'sha512':
            raise ValueError('unexpected npm integrity algorithm')
        sha = base64.b64decode(b64, validate=True).hex()
        if len(sha) != 128:
            raise ValueError('invalid npm checksum')
        entry = {'url': url, 'sha512': sha, 'path': path}
        if filename in npm and npm[filename] != entry:
            raise ValueError('conflicting npm archive')
        npm[filename] = entry
    if len(crates) < 500 or len(npm) != 6:
        raise ValueError('unexpected dependency inventory size')
    return {'schema': 1, 'source': SOURCE, 'tree': TREE,
            'locks': {p: digest(root / p) for p in (*LOCKS, NPM)},
            'crates': dict(sorted(crates.items())), 'npm_linux_x64': dict(sorted(npm.items())),
            'model': {'filename': MODEL, 'sha256': MODEL_SHA}}


def verify_source(root):
    if git(root, 'rev-parse', 'HEAD') != SOURCE or git(root, 'rev-parse', 'HEAD^{tree}') != TREE:
        raise ValueError('wrong accepted source commit or tree')
    if git(root, 'status', '--porcelain', '--untracked-files=normal'):
        raise ValueError('source checkout not clean')
    run('node', 'scripts/verify-source-tree.mjs', cwd=root)


def npm_fetch(item, out):
    name, entry = item
    target = out / name
    # No package lifecycle scripts. HTTPS and exact lockfile integrity required.
    run('curl', '--fail', '--silent', '--show-error', '--proto', '=https', '--connect-timeout', '20', '--max-time', '180', '--retry', '3', '--max-filesize', '100000000', '-o', str(target), entry['url'])
    if digest(target, 'sha512') != entry['sha512']:
        raise ValueError(f'npm integrity mismatch: {name}')


def collect(root, out, cargo_home):
    verify_source(root)
    inv = lock_inventory(root)
    out.mkdir(parents=True, exist_ok=True)
    (out / 'inventory.json').write_text(json.dumps(inv, indent=2) + '\n')
    env = dict(os.environ, CARGO_HOME=str(cargo_home), CARGO_NET_OFFLINE='false', CARGO_REGISTRIES_CRATES_IO_PROTOCOL='sparse')
    # cargo fetch does not compile or execute package build scripts.
    for lock in LOCKS:
        manifest = str(root / pathlib.Path(lock).with_name('Cargo.toml'))
        run('cargo', 'fetch', '--locked', '--manifest-path', manifest, cwd=root, env=env)
    cache_dirs = list((cargo_home / 'registry/cache').glob('index.crates.io-*'))
    if len(cache_dirs) != 1:
        raise ValueError('unexpected cargo cache directory')
    cache_dir = cache_dirs[0]
    for name, sha in inv['crates'].items():
        if digest(cache_dir / name) != sha:
            raise ValueError(f'crate checksum mismatch: {name}')
    # Deterministic, bounded transport shards; every crate and index is reverified on restore.
    crates_dir = out / 'crates'; crates_dir.mkdir()
    for i, names in enumerate([list(inv['crates'])[i:i+100] for i in range(0, len(inv['crates']), 100)]):
        shard = crates_dir / f'crates-{i:02d}.tar.gz'
        run('tar', '-czf', str(shard), '-C', str(cache_dir), *names)
    run('tar', '-czf', str(out / 'cargo-index.tar.gz'), '-C', str(cargo_home / 'registry'), 'index')
    npm_out = out / 'npm'; npm_out.mkdir()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(lambda item: npm_fetch(item, npm_out), inv['npm_linux_x64'].items()))
    # Prove the source has not changed during fetching. No install or runtime execution.
    verify_source(root)
    files = sorted(p for p in out.rglob('*') if p.is_file())
    (out / 'SHA256SUMS').write_text(''.join(f'{digest(p)}  {p.relative_to(out).as_posix()}\n' for p in files))
    print(f'CP_DEV_OFFLINE_DEPENDENCIES_PASS crates={len(inv["crates"])} npm={len(inv["npm_linux_x64"])}')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--repo', type=pathlib.Path, required=True)
    ap.add_argument('--out', type=pathlib.Path)
    ap.add_argument('--cargo-home', type=pathlib.Path)
    ap.add_argument('--plan', action='store_true')
    a = ap.parse_args()
    root = a.repo.resolve()
    if a.plan:
        print(json.dumps(lock_inventory(root), indent=2)); return
    if a.out is None or a.cargo_home is None:
        ap.error('--out and --cargo-home required for collection')
    collect(root, a.out.resolve(), a.cargo_home.resolve())

if __name__ == '__main__':
    main()
