/**
 * TOOLS-10C CI-only browser runtime candidate assembler.
 * Run after:
 *   npm install --prefix src-tauri/lu-core/workers --no-save --no-package-lock playwright@1.64.1
 *   node src-tauri/lu-core/workers/node_modules/playwright/cli.js install --no-shell chromium
 *
 * Downloads occur in CI provisioning, not here. This copies the exact
 * installed headed Chromium build, Node executable, Playwright/Playwright
 * Core, Lu browser worker and Linux X11 helper into a self-contained candidate.
 * The emitted 'candidate' manifest is NOT an approved production digest.
 */
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {
  copyFileSync,existsSync,lstatSync,mkdirSync,realpathSync,
  readdirSync,readFileSync,rmSync,writeFileSync
} from 'node:fs';
import {join,resolve,dirname,sep} from 'node:path';
import {pathToFileURL} from 'node:url';

const repo=resolve(import.meta.dirname,'../..');
const worker=join(repo,'src-tauri/lu-core/workers');
const playwrightEntry=join(worker,'node_modules/playwright/index.mjs');
const playwrightCore=join(worker,'node_modules/playwright-core');
const browserWorker=join(worker,'lu_browser_worker.mjs');
const expectedVersion='1.64.1';
if(!existsSync(playwrightEntry)||!existsSync(playwrightCore)||!existsSync(browserWorker))
  throw Error('CI Playwright/worker files not provisioned');
const packageJson=JSON.parse(readFileSync(join(worker,'node_modules/playwright/package.json'),'utf8'));
if(packageJson.version!==expectedVersion)
  throw Error('Unexpected Playwright version '+packageJson.version);
const {chromium}=await import(pathToFileURL(playwrightEntry).href);
const executable=realpathSync(chromium.executablePath());
if(!existsSync(executable))throw Error('Playwright headed Chromium is missing');
const expectedBinary=process.platform==='win32'?'chrome.exe':'chrome';
if(executable.split(sep).at(-1)!==expectedBinary)
  throw Error('Expected headed Chromium executable, got '+executable);
if(process.arch!=='x64'||!['win32','linux'].includes(process.platform))
  throw Error('Browser CI candidate currently supports Windows/Linux x64 only');
const destination=resolve(process.env.LU_BROWSER_CANDIDATE_OUT||join(repo,'out','lu-browser-candidate'));
if(destination===repo||destination===worker||destination==='/'||destination.length<12)
  throw Error('Refuse unsafe candidate destination');
rmSync(destination,{recursive:true,force:true});
mkdirSync(destination,{recursive:true});
const seen=new Set();
function copyRegular(src,relativePath,root){
  const relativeParts=relativePath.split('/');
  if(!relativeParts.every(p=>p&&p!=='.'&&p!=='..'
    &&/^[a-zA-Z0-9._ -]+$/.test(p)&&p.trim()===p))
    throw Error('Unsafe browser candidate path '+relativePath);
  if(seen.has(relativePath))throw Error('Duplicate candidate path '+relativePath);
  const actual=realpathSync(src);
  if(root){
    const base=realpathSync(root);
    if(actual!==base&&!actual.startsWith(base+sep))
      throw Error('Component symlink escapes trusted source root: '+src);
  }
  const stat=lstatSync(actual);
  if(!stat.isFile())throw Error('Not a regular browser candidate file: '+src);
  const target=join(destination,...relativeParts);
  mkdirSync(dirname(target),{recursive:true});
  copyFileSync(actual,target);
  if(process.platform!=='win32') {
    // Only preserve expected executable permissions; all other files are
    // regular files. Artifact download users may need to chmod after unzip.
    const mode=stat.mode&0o777;
    const {chmodSync}=awaitedFs;
    chmodSync(target,mode);
  }
  seen.add(relativePath);
}
const awaitedFs=await import('node:fs');
function copyTree(root,prefix){
  const base=realpathSync(root);
  function walk(dir,sub=''){
    for(const e of readdirSync(dir,{withFileTypes:true})){
      const p=join(dir,e.name);
      const part=sub?sub+'/'+e.name:e.name;
      const resolved=realpathSync(p);
      if(resolved!==base&&!resolved.startsWith(base+sep))
        throw Error('Candidate symlink escapes component tree');
      const meta=lstatSync(resolved);
      if(meta.isDirectory())walk(resolved,part);
      else if(meta.isFile())copyRegular(resolved,prefix?prefix+'/'+part:part,base);
      else throw Error('Unsupported candidate component '+p);
    }
  }
  walk(base);
}
// Chromium payload is a version-matched *headed* distribution containing
// adjacent shared libraries, locales, resources and helper executables.
copyTree(dirname(executable),'');
// Both JS packages are needed for Playwright's normal import resolution.
copyTree(join(worker,'node_modules/playwright'),'node_modules/playwright');
copyTree(playwrightCore,'node_modules/playwright-core');
copyRegular(browserWorker,'lu_browser_worker.mjs',worker);
copyRegular(process.execPath,process.platform==='win32'?'node.exe':'node',dirname(process.execPath));
if(process.platform==='linux'){
  const xdotool=process.env.LU_BROWSER_XDOTOOL;
  if(!xdotool||!existsSync(xdotool))throw Error('Provide LU_BROWSER_XDOTOOL from CI');
  copyRegular(xdotool,'xdotool',dirname(realpathSync(xdotool)));
}
const entries=[];
for(const rel of [...seen].sort()){
  const file=join(destination,...rel.split('/'));
  const hash=createHash('sha256');
  for await(const chunk of createReadStream(file))hash.update(chunk);
  entries.push({path:rel,sha256:hash.digest('hex'),bytes:lstatSync(file).size});
}
const required=process.platform==='win32'
  ? ['chrome.exe','node.exe','lu_browser_worker.mjs','node_modules/playwright/index.mjs']
  : ['chrome','node','xdotool','lu_browser_worker.mjs','node_modules/playwright/index.mjs'];
for(const name of required){
  if(!seen.has(name))throw Error('Candidate missing required entrypoint: '+name);
}
const manifest={
  schema:1,runtime:'lu-browser',status:'candidate',
  playwrightVersion:expectedVersion,
  platform:process.platform,arch:process.arch,
  files:entries
};
const manifestBytes=Buffer.from(JSON.stringify(manifest,null,2)+'\n');
writeFileSync(join(destination,'manifest.json'),manifestBytes);
const digest=createHash('sha256').update(manifestBytes).digest('hex');
writeFileSync(join(destination,'manifest.sha256'),digest+'\n');
const summary={status:'candidate',platform:process.platform,arch:process.arch,
  playwrightVersion:expectedVersion,files:entries.length,
  manifestSha256:digest,sourceChromium:executable};
writeFileSync(join(destination,'candidate-summary.json'),JSON.stringify(summary,null,2)+'\n');
console.log(JSON.stringify(summary,null,2));
