/**
 * Stage a reviewed, content-pinned Lu browser runtime from an existing local
 * bundle. No downloads, Docker, PATH lookup, or model-provided files.
 *
 * LU_BROWSER_PACKAGE_DIR: absolute existing payload directory
 * LU_BROWSER_MANIFEST_SHA256: out-of-band reviewed manifest SHA-256 (64 hex)
 *
 * The manifest enumerates *every* shipped file, including Chromium shared
 * libraries and Node/Playwright dependencies. Stage only enumerated files.
 * A digest of a manifest is an integrity anchor, not a publisher signature.
 */
import {createHash} from 'node:crypto';
import {
  copyFileSync,existsSync,lstatSync,mkdirSync,readFileSync,
  readdirSync,rmSync,renameSync,statSync,writeFileSync
} from 'node:fs';
import {dirname,join,resolve,sep} from 'node:path';

const input=process.env.LU_BROWSER_PACKAGE_DIR;
const expected=process.env.LU_BROWSER_MANIFEST_SHA256?.toLowerCase();
if(!input||!expected||!/^[a-f0-9]{64}$/.test(expected))
  throw Error('Requires LU_BROWSER_PACKAGE_DIR and reviewed LU_BROWSER_MANIFEST_SHA256');
const source=resolve(input),base=resolve(import.meta.dirname,'..');
if(!lstatSync(source).isDirectory()||lstatSync(source).isSymbolicLink())
  throw Error('Browser package source must be an ordinary directory');
const manifestBytes=readFileSync(join(source,'manifest.json'));
if(createHash('sha256').update(manifestBytes).digest('hex')!==expected)
  throw Error('Browser package manifest differs from reviewed digest');
const manifest=JSON.parse(manifestBytes.toString('utf8'));
if(manifest.schema!==1||manifest.runtime!=='lu-browser'||
   manifest.status!=='reviewed'||manifest.playwrightVersion!=='1.64.1'||
   manifest.platform!==process.platform||manifest.arch!==process.arch||
   !Array.isArray(manifest.files)||manifest.files.length<4)
  throw Error('Unsupported or incomplete browser package manifest');
const required=process.platform==='win32'
  ? ['chrome.exe','node.exe','lu_browser_worker.mjs','node_modules/playwright/index.mjs']
  : ['chrome','node','xdotool','lu_browser_worker.mjs','node_modules/playwright/index.mjs'];
const paths=new Set();
for(const f of manifest.files){
  if(!f||typeof f.path!=='string'||typeof f.sha256!=='string'||
    !/^[a-f0-9]{64}$/.test(f.sha256)||!Number.isSafeInteger(f.bytes)||
    f.bytes<0||f.bytes>2_000_000_000||
    !/^[a-zA-Z0-9._ -]+(?:\/[a-zA-Z0-9._ -]+)*$/.test(f.path)||
    f.path.split('/').some(part=>part.trim()!==part)||
    f.path.split('/').some(part=>part==='.'||part==='..')||
    paths.has(f.path)) throw Error('Untrusted browser package path, hash or size');
  paths.add(f.path);
  const file=join(source,...f.path.split('/'));
  const meta=lstatSync(file);
  if(!meta.isFile()||meta.isSymbolicLink()||meta.size!==f.bytes)
    throw Error('Browser component is missing, symlinked or differs in size: '+f.path);
  // Content hashing through streaming avoids loading Chromium binaries into
  // memory; synchronous file traversal is used only during explicit staging.
}
for(const path of required)if(!paths.has(path))
  throw Error('Required browser component not included in package: '+path);
const {createReadStream}=await import('node:fs');
for(const f of manifest.files){
  const hash=createHash('sha256');
  const file=join(source,...f.path.split('/'));
  for await(const chunk of createReadStream(file))hash.update(chunk);
  if(hash.digest('hex')!==f.sha256)throw Error('Browser component hash mismatch: '+f.path);
}
const destination=join(base,'src-tauri/resources/lu-browser');
const staging=destination+'.staging-'+process.pid;
rmSync(staging,{recursive:true,force:true});
mkdirSync(staging,{recursive:true});
try {
  for(const f of manifest.files){
    const target=join(staging,...f.path.split('/'));
    mkdirSync(dirname(target),{recursive:true});
    copyFileSync(join(source,...f.path.split('/')),target);
  }
  writeFileSync(join(staging,'manifest.json'),manifestBytes);
  // The source directory contains a placeholder manifest in Git. Remove
  // it only after all package files and hashes have been verified.
  rmSync(destination,{recursive:true,force:true});
  renameSync(staging,destination);
}catch(e){rmSync(staging,{recursive:true,force:true});throw e;}
console.log('Lu reviewed browser package staged: '+manifest.files.length+' hashed files');
