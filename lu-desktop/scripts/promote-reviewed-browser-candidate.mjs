#!/usr/bin/env node
/** C6.2: offline, explicit source-reviewed Chromium candidate promotion.
 * No downloads, changes to tracked source, or automatic approval from CI.
 * A candidate is NOT a reviewed bundle just because its hashes are internally consistent.
 */
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {copyFileSync,existsSync,lstatSync,mkdirSync,readFileSync,readdirSync,realpathSync,rmSync,renameSync,writeFileSync} from 'node:fs';
import {dirname,join,relative,resolve,sep} from 'node:path';
const repo=resolve(import.meta.dirname,'../..');
const approvalPath=join(repo,'docs/acceptance/LU_C62_WINDOWS_BROWSER_REVIEW.json');
const candidate=process.env.LU_BROWSER_CANDIDATE_DIR;
const approvedOut=process.env.LU_BROWSER_REVIEWED_OUT;
const hex=/^[0-9a-f]{64}$/;
function sha(value){return createHash('sha256').update(value).digest('hex');}
function ordinaryFile(path){
  const meta=lstatSync(path);
  if(!meta.isFile()||meta.isSymbolicLink()||meta.nlink!==1)throw Error('Untrusted nonordinary browser file: '+path);
  return meta;
}
function safePath(path){
  return typeof path==='string'&&path.length<512&&
    /^[a-zA-Z0-9._ -]+(?:\/[a-zA-Z0-9._ -]+)*$/.test(path)&&
    path.split('/').every(x=>x!=='.'&&x!=='..'&&x.trim()===x);
}
function assertDirectoryParts(dir,base){
  const rel=relative(base,dir);
  if(rel.startsWith('..')||rel=== '..'||resolve(base,rel)!==dir)
    throw Error('Browser directory outside its pinned source root');
  let here=base;
  const rootMeta=lstatSync(base);
  if(!rootMeta.isDirectory()||rootMeta.isSymbolicLink())throw Error('Untrusted browser root');
  for(const part of rel.split(sep).filter(Boolean)){
    here=join(here,part);
    const meta=lstatSync(here);
    if(!meta.isDirectory()||meta.isSymbolicLink())throw Error('Untrusted browser directory symlink: '+here);
  }
}
async function fileHash(path){
  const h=createHash('sha256');
  for await(const chunk of createReadStream(path))h.update(chunk);
  return h.digest('hex');
}
if(!candidate||!approvedOut)throw Error('Explicit candidate and output directories required');
const input=resolve(candidate),output=resolve(approvedOut);
if(input===output||output===repo||output===dirname(repo)||output===resolve('/')||
   output.startsWith(input+sep)||input.startsWith(output+sep))
  throw Error('Unsafe browser review output path');
if(!existsSync(approvalPath))throw Error('No independently committed C6.2 review approval record');
const review=JSON.parse(readFileSync(approvalPath,'utf8'));
if(review.schema!=='lu-c62-windows-browser-review-v1'||review.disposition!=='approved'||
   review.platform!=='win32'||review.arch!=='x64'||review.playwrightVersion!=='1.64.1'||
   !/^[0-9a-f]{40}$/.test(review.sourceSha)||!Number.isSafeInteger(review.runId)||
   review.runId<1||typeof review.artifactName!=='string'||!review.artifactName.startsWith('Lu-C62A-UNAPPROVED-Windows-Browser-')||
   !hex.test(review.candidateManifestSha256)||!hex.test(review.reviewedManifestSha256))
  throw Error('C6.2 out-of-band review record incomplete or ambiguous');
if(review.artifactName!=='Lu-C62A-UNAPPROVED-Windows-Browser-'+review.sourceSha)
  throw Error('Reviewed artifact does not match source SHA');
assertDirectoryParts(input,input);
ordinaryFile(join(input,'manifest.json'));
const candidateBytes=readFileSync(join(input,'manifest.json'));
if(sha(candidateBytes)!==review.candidateManifestSha256)
  throw Error('Candidate manifest is not the independently reviewed source digest');
const manifest=JSON.parse(candidateBytes.toString('utf8'));
if(manifest.schema!==1||manifest.runtime!=='lu-browser'||manifest.status!=='candidate'||
   manifest.platform!==review.platform||manifest.arch!==review.arch||
   manifest.playwrightVersion!==review.playwrightVersion||
   !Array.isArray(manifest.files)||manifest.files.length<100||manifest.files.length>10000)
  throw Error('No genuine complete headed browser candidate');
const required=new Set(['chrome.exe','node.exe','lu_browser_worker.mjs',
  'node_modules/playwright/index.mjs','node_modules/playwright-core/index.mjs']);
const entries=new Set(),actual=new Set();
for(const f of manifest.files){
  if(!f||!safePath(f.path)||entries.has(f.path)||!hex.test(f.sha256)||
     !Number.isSafeInteger(f.bytes)||f.bytes<0||f.bytes>2_000_000_000)
    throw Error('Unsafe candidate file identity');
  entries.add(f.path);required.delete(f.path);
  const item=join(input,...f.path.split('/'));
  assertDirectoryParts(dirname(item),input);
  if(ordinaryFile(item).size!==f.bytes||await fileHash(item)!==f.sha256)
    throw Error('Candidate file differs from its reviewed content: '+f.path);
}
if(required.size)throw Error('Missing browser runtime entrypoints: '+[...required].join(','));
function walk(dir){
  assertDirectoryParts(dir,input);
  for(const e of readdirSync(dir,{withFileTypes:true})){
    const p=join(dir,e.name),st=lstatSync(p);
    if(st.isSymbolicLink())throw Error('Candidate contains an unauthorized symlink');
    if(st.isDirectory())walk(p);
    else {
      ordinaryFile(p);
      actual.add(relative(input,p).split(sep).join('/'));
    }
  }
}
walk(input);
const extras=new Set(['manifest.json','manifest.sha256','candidate-summary.json']);
for(const item of actual)if(!entries.has(item)&&!extras.has(item))
  throw Error('Candidate contained undeclared executable/data: '+item);
for(const item of entries)if(!actual.has(item))throw Error('Candidate lost manifest payload: '+item);
const summary=JSON.parse(readFileSync(join(input,'candidate-summary.json'),'utf8'));
if(summary.status!=='candidate'||summary.manifestSha256!==review.candidateManifestSha256||
   summary.files!==manifest.files.length||summary.platform!=='win32'||summary.arch!=='x64')
  throw Error('Original CI candidate summary differs from reviewed artifact');
if(readFileSync(join(input,'manifest.sha256'),'utf8').trim()!==review.candidateManifestSha256)
  throw Error('Original manifest companion digest differs');
manifest.status='reviewed';
const reviewedBytes=Buffer.from(JSON.stringify(manifest,null,2)+'\n');
if(sha(reviewedBytes)!==review.reviewedManifestSha256)
  throw Error('Reviewed result digest is not independently approved');
if(existsSync(output))throw Error('Refuse overwrite of existing reviewed browser output');
const pending=output+'.staging-'+process.pid;
if(existsSync(pending))throw Error('Ambiguous pre-existing staging directory');
mkdirSync(pending,{recursive:true});
try{
  for(const f of manifest.files){
    const target=join(pending,...f.path.split('/'));
    mkdirSync(dirname(target),{recursive:true});
    copyFileSync(join(input,...f.path.split('/')),target);
    if(await fileHash(target)!==f.sha256)throw Error('Copy changed reviewed component '+f.path);
  }
  writeFileSync(join(pending,'manifest.json'),reviewedBytes,{flag:'wx'});
  renameSync(pending,output);
}catch(e){rmSync(pending,{recursive:true,force:true});throw e;}
console.log(JSON.stringify({schema:'lu-c62-reviewed-stage-v1',sourceSha:review.sourceSha,
  runId:review.runId,artifactName:review.artifactName,
  candidateManifestSha256:review.candidateManifestSha256,
  reviewedManifestSha256:review.reviewedManifestSha256,
  actualFiles:manifest.files.length,status:'reviewed'},null,2));
