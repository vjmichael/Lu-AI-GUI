import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';

const executable=process.env.LU_BACKGROUND_WORKER_TEST_EXE || resolve('lu-desktop/tests/native-file-scan/target/debug/lu-file-scan-worker-acceptance' + (process.platform==='win32'?'.exe':''));
function launch(args){
  const result=spawnSync(executable,args,{encoding:'utf8',timeout:60_000,
    maxBuffer:256*1024,windowsHide:true,env:{...process.env}});
  if(result.error)throw result.error;
  return result;
}

test('real installed-app executable worker reads media-sized file and returns measured digest',()=>{
  const dir=mkdtempSync(join(tmpdir(),'lu-scanner-e2e-'));
  try{
    const source=join(dir,'sample-video.mp4');
    // 12 MiB fixture exercises multiple bounded buffer reads, no image/video decoder.
    const bytes=Buffer.alloc(12*1024*1024);
    for(let i=0;i<bytes.length;i+=4096){bytes.writeUInt32LE(i,i);}
    writeFileSync(source,bytes);
    const expected=createHash('sha256').update(bytes).digest('hex');
    const result=launch(['--lu-file-scan-worker',source]);
    assert.equal(result.status,0,result.stderr);
    const lines=result.stdout.trim().split(/\r?\n/);
    assert.ok(lines.length>3,'multiple real progress events expected');
    const progress=lines.slice(0,-1).map(line=>{
      const match=/^P\t(\d+)\t(\d+)$/.exec(line);
      assert.ok(match,'invalid progress frame '+JSON.stringify(line));
      const current=Number(match[1]),total=Number(match[2]);
      assert.equal(total,bytes.length);
      return current;
    });
    assert.equal(progress[0],0);
    assert.ok(progress.every((p,i)=>i===0||p>=progress[i-1]));
    assert.equal(progress.at(-1),bytes.length);
    assert.equal(lines.at(-1),'D\t'+expected);
  }finally{rmSync(dir,{recursive:true,force:true});}
});
test('worker mode requires a single readable nonempty file',()=>{
  assert.equal(launch(['--lu-file-scan-worker']).status,2);
  const dir=mkdtempSync(join(tmpdir(),'lu-scanner-invalid-'));
  try{
    const empty=join(dir,'empty.mp4');writeFileSync(empty,'');
    assert.equal(launch(['--lu-file-scan-worker',empty]).status,3);
  }finally{rmSync(dir,{recursive:true,force:true});}
});
