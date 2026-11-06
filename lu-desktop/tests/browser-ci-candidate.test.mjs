import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {readFileSync,lstatSync,existsSync,readdirSync} from 'node:fs';
import {resolve,join,relative,sep} from 'node:path';

const out=resolve(import.meta.dirname,'../../out/lu-browser-candidate');
const manifestPath=join(out,'manifest.json');
const summaryPath=join(out,'candidate-summary.json');
const digestPath=join(out,'manifest.sha256');
async function digest(path){
  const hash=createHash('sha256');
  for await(const chunk of createReadStream(path))hash.update(chunk);
  return hash.digest('hex');
}
test('real CI candidate declares exact platform, headed browser and unapproved status',()=>{
  assert.ok(existsSync(manifestPath),'Candidate artifact not assembled');
  const m=JSON.parse(readFileSync(manifestPath,'utf8'));
  assert.equal(m.schema,1);
  assert.equal(m.runtime,'lu-browser');
  assert.equal(m.status,'candidate');
  assert.equal(m.playwrightVersion,'1.64.1');
  assert.equal(m.platform,process.platform);
  assert.equal(m.arch,process.arch);
  assert.ok(m.files.length>4);
  const names=new Set(m.files.map(x=>x.path));
  for(const needed of (process.platform==='win32'
    ? ['chrome.exe','node.exe','lu_browser_worker.mjs',
       'node_modules/playwright/index.mjs','node_modules/playwright-core/index.mjs']
    : ['chrome','node','xdotool','lu_browser_worker.mjs',
       'node_modules/playwright/index.mjs','node_modules/playwright-core/index.mjs'])){
    assert.ok(names.has(needed),'Missing candidate file '+needed);
  }
  assert.equal(new Set(m.files.map(x=>x.path)).size,m.files.length);
  assert.equal(JSON.parse(readFileSync(summaryPath,'utf8')).status,'candidate');
});
test('every candidate byte is covered by SHA256 and manifest has independent digest',async()=>{
  const m=JSON.parse(readFileSync(manifestPath,'utf8'));
  assert.equal(await digest(manifestPath),readFileSync(digestPath,'utf8').trim());
  const expected=new Set(['manifest.json','manifest.sha256','candidate-summary.json']);
  for(const f of m.files){
    assert.match(f.path,/^[a-zA-Z0-9._ -]+(?:\/[a-zA-Z0-9._ -]+)*$/);
    assert.ok(f.path.split('/').every(part=>part===part.trim()));
    const p=join(out,...f.path.split('/'));
    const stat=lstatSync(p);
    assert.ok(stat.isFile()&&!stat.isSymbolicLink(),f.path);
    assert.equal(stat.size,f.bytes,f.path);
    assert.equal(await digest(p),f.sha256,f.path);
    expected.add(f.path);
  }
  const actual=new Set();
  function enumerate(dir){
    for(const entry of readdirSync(dir,{withFileTypes:true})){
      const path=join(dir,entry.name);
      const stat=lstatSync(path);
      assert.ok(!stat.isSymbolicLink());
      if(stat.isDirectory())enumerate(path);
      else{
        assert.ok(stat.isFile());
        actual.add(relative(out,path).split(sep).join('/'));
      }
    }
  }
  enumerate(out);
  assert.deepEqual(actual,expected,'Candidate contains untracked content');
});
