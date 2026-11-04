import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,copyFileSync,
  existsSync,rmSync} from 'node:fs';
import {join,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {spawnSync} from 'node:child_process';

const stageScript=new URL('../scripts/stage-browser-runtime.mjs',import.meta.url);
function sha(bytes){return createHash('sha256').update(bytes).digest('hex');}
function setup(){
  const root=mkdtempSync(join(tmpdir(),'lu-browser-stage-'));
  const packageDir=join(root,'supplied');
  const scripts=join(root,'lu-desktop','scripts');
  mkdirSync(packageDir,{recursive:true});
  mkdirSync(scripts,{recursive:true});
  const targetScript=join(scripts,'stage-browser-runtime.mjs');
  copyFileSync(stageScript,targetScript);
  const names=process.platform==='win32'
    ? ['chrome.exe','node.exe','lu_browser_worker.mjs','node_modules/playwright/index.mjs']
    : ['chrome','node','lu_browser_worker.mjs','node_modules/playwright/index.mjs'];
  const files=names.map((path,index)=>{
    const data=Buffer.from('fixture component '+index+' for '+path);
    const file=join(packageDir,...path.split('/'));
    mkdirSync(dirname(file),{recursive:true});
    writeFileSync(file,data);
    return {path,sha256:sha(data),bytes:data.length};
  });
  const manifest={schema:1,runtime:'lu-browser',status:'reviewed',
    playwrightVersion:'1.64.1',platform:process.platform,
    arch:process.arch,files};
  const text=JSON.stringify(manifest,null,2)+'\n';
  writeFileSync(join(packageDir,'manifest.json'),text);
  return {root,packageDir,targetScript,manifest,text};
}
function stage(data,expected){
  return spawnSync(process.execPath,[data.targetScript],{
    env:{...process.env,LU_BROWSER_PACKAGE_DIR:data.packageDir,
      LU_BROWSER_MANIFEST_SHA256:expected},encoding:'utf8',timeout:15000
  });
}
test('reviewed manifest and every exact file stage offline with no extra payload',()=>{
  const data=setup();
  try{
    writeFileSync(join(data.packageDir,'unlisted-native.dll'),'not in manifest');
    const result=stage(data,sha(data.text));
    assert.equal(result.status,0,result.stderr);
    const target=join(data.root,'lu-desktop','src-tauri','resources','lu-browser');
    for(const file of data.manifest.files){
      assert.ok(existsSync(join(target,...file.path.split('/'))));
    }
    assert.equal(readFileSync(join(target,'manifest.json'),'utf8'),data.text);
    assert.equal(existsSync(join(target,'unlisted-native.dll')),false);
  }finally{rmSync(data.root,{recursive:true,force:true});}
});
test('an unreviewed or mismatched manifest refuses to stage any runtime',()=>{
  const data=setup();
  try{
    const result=stage(data,'0'.repeat(64));
    assert.notEqual(result.status,0);
    const target=join(data.root,'lu-desktop','src-tauri','resources','lu-browser');
    assert.equal(existsSync(target),false);
  }finally{rmSync(data.root,{recursive:true,force:true});}
});
test('modifying a reviewed component fails staging without side effects',()=>{
  const data=setup();
  try{
    writeFileSync(join(data.packageDir,data.manifest.files[0].path),'modified');
    const result=stage(data,sha(data.text));
    assert.notEqual(result.status,0);
    const target=join(data.root,'lu-desktop','src-tauri','resources','lu-browser');
    assert.equal(existsSync(target),false);
  }finally{rmSync(data.root,{recursive:true,force:true});}
});
