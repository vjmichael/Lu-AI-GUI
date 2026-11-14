// Cross-platform Lu UI bundler. Use the installed esbuild Node API rather
// than trying to spawn a bare executable (which fails on Windows runners).
import { build } from 'esbuild';
import { mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const out = join(root, 'dist');
mkdirSync(join(out, 'assets'), {recursive:true});
await build({
  entryPoints: [join(root, 'web', 'main.jsx')],
  outfile: join(out, 'assets', 'app.js'),
  bundle: true,
  platform: 'browser',
  target: 'es2020',
  format: 'esm',
  jsx: 'automatic',
  minify: true,
  logLevel: 'info',
});
copyFileSync(join(root, 'web', 'index.html'), join(out, 'index.html'));
copyFileSync(join(root, 'web', 'styles.css'), join(out, 'assets', 'styles.css'));
console.log('Lu independent React UI built to dist/');
