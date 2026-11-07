import { build } from 'esbuild';
import { mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

// Invoke esbuild's supported JS API instead of spawning an extensionless
// executable name, which fails on Windows when npm only exposes esbuild.cmd.
const root=resolve(import.meta.dirname,'..');
const out=join(root,'dist');
mkdirSync(join(out,'assets'),{recursive:true});
try{
  await build({
    entryPoints:[join(root,'web','main.jsx')],
    bundle:true,
    platform:'browser',
    target:'es2020',
    format:'esm',
    jsx:'automatic',
    minify:true,
    outfile:join(out,'assets','app.js'),
    logLevel:'info',
  });
}catch(error){
  console.error('Lu desktop bundle failed:',error);
  process.exit(1);
}
copyFileSync(join(root,'web','index.html'),join(out,'index.html'));
copyFileSync(join(root,'web','styles.css'),join(out,'assets','styles.css'));
console.log('Lu independent React UI built to dist/');
