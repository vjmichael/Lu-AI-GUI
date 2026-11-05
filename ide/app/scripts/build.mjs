import { spawnSync } from 'node:child_process';
import { mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const output = join(root, 'dist');
mkdirSync(join(output, 'assets'), { recursive: true });
// Run esbuild through Node's platform-neutral JS entrypoint. npm install's
// optional native binary is selected by esbuild itself on Windows/Linux.
const esbuild = join(root, 'node_modules', 'esbuild', 'bin', 'esbuild');
const args = [
  join(root, 'web', 'main.jsx'),
  '--bundle', '--platform=browser', '--target=es2020',
  '--format=esm', '--jsx=automatic', '--minify',
  `--outfile=${join(output, 'assets', 'app.js')}`
];
const result = spawnSync(process.execPath, [esbuild, ...args], { cwd: root, stdio: 'inherit' });
if (result.error || result.status !== 0) {
  console.error(result.error || `Standalone IDE bundle failed: ${result.status}`);
  process.exit(1);
}
copyFileSync(join(root, 'web', 'index.html'), join(output, 'index.html'));
copyFileSync(join(root, 'web', 'styles.css'), join(output, 'assets', 'styles.css'));
console.log('Standalone Lu IDE web resources built');
