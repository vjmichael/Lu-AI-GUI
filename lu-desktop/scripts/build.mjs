import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const fallback = join(root, 'node_modules', '@esbuild', 'linux-x64', 'bin', 'esbuild');
const executable = process.env.ESBUILD_BIN ?? (existsSync(fallback) ? fallback : 'esbuild');
const out = join(root, 'dist');
mkdirSync(join(out, 'assets'), {recursive:true});
const args = [
  join(root, 'web', 'main.jsx'), '--bundle', '--platform=browser', '--target=es2020',
  '--format=esm', '--jsx=automatic', '--minify',
  `--outfile=${join(out, 'assets', 'app.js')}`
];
const r = spawnSync(executable, args, { cwd: root, stdio: 'inherit' });
if (r.error || r.status !== 0) { console.error(r.error || `bundle failed: ${r.status}`); process.exit(1); }
copyFileSync(join(root,'web','index.html'), join(out,'index.html'));
copyFileSync(join(root,'web','styles.css'), join(out,'assets','styles.css'));
console.log('Lu independent React UI built to dist/');
