import { build } from 'esbuild';
import { mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const output = join(root, 'dist');
mkdirSync(join(output, 'assets'), { recursive: true });

// esbuild/bin/esbuild may be a platform-native ELF or PE executable after
// npm installs its optional binary. Node must never try to parse it as JS.
// The public JS API resolves the installed platform binary correctly.
try {
  await build({
    entryPoints: [join(root, 'web', 'main.jsx')],
    bundle: true,
    platform: 'browser',
    target: 'es2020',
    format: 'esm',
    jsx: 'automatic',
    minify: true,
    outfile: join(output, 'assets', 'app.js'),
    logLevel: 'info',
  });
} catch (error) {
  console.error('Standalone IDE bundle failed:', error);
  process.exit(1);
}
copyFileSync(join(root, 'web', 'index.html'), join(output, 'index.html'));
copyFileSync(join(root, 'web', 'styles.css'), join(output, 'assets', 'styles.css'));
console.log('Standalone Lu IDE web resources built');
