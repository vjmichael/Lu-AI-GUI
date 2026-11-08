import assert from 'node:assert/strict';
import test from 'node:test';
import {
  safeProjectRelativePath, collectQuickOpenFiles, matchQuickOpenFiles,
} from '../web/quick_open.js';

const tree = {
  '': [{kind:'file',relativePath:'README.md'},
       {kind:'directory',relativePath:'src'},
       {kind:'directory',relativePath:'docs'},
       {kind:'symlink',relativePath:'linked'}],
  src: [{kind:'file',relativePath:'src/main.rs'},
        {kind:'directory',relativePath:'src/nested'}],
  docs: [{kind:'file',relativePath:'docs/info.md'}],
  'src/nested': [{kind:'file',relativePath:'src/nested/hello.rs'}],
};
const listing = async (_, dir) => ({entries:tree[dir],truncated:false});

test('quick open descends only native project-relative file/directory entries', async () => {
  const result=await collectQuickOpenFiles({projectId:'project-1',listDirectory:listing});
  assert.deepEqual(result.paths, [
    'docs/info.md','README.md','src/main.rs','src/nested/hello.rs',
  ].sort((a,b)=>a.localeCompare(b)));
  assert.equal(result.directoriesVisited,4);
  assert.equal(result.truncated,false);
  assert.ok(!result.paths.includes('linked'));
});

test('quick open filtering ranks basename exact/prefix before other paths', () => {
  const paths=['docs/main.rs','src/main.rs','src/main_test.rs','main.rs',
               'README.md','src/test-main.rs'];
  assert.deepEqual(matchQuickOpenFiles(paths,'main.rs').slice(0,3),
    ['main.rs','docs/main.rs','src/main.rs']);
  assert.deepEqual(matchQuickOpenFiles(paths,'test').sort(),
    ['src/main_test.rs','src/test-main.rs'].sort());
  assert.equal(matchQuickOpenFiles(paths,'missing').length,0);
  assert.equal(matchQuickOpenFiles(paths,'x'.repeat(257)).length,0);
});

test('reject traversal, OS absolute paths, control chars and non-native directory entries', async () => {
  for (const path of ['../secret','x/../secret','/tmp/a','C:/secret','a\\b',
                      'a//b','./x','a/./b','a\nsecret','']) {
    assert.equal(safeProjectRelativePath(path),false,path);
  }
  for (const bad of ['../evil','src/other.txt','/tmp/hack','src\\evil']) {
    await assert.rejects(collectQuickOpenFiles({
      projectId:'p',listDirectory:async (_,dir)=>({entries:dir===''?
        [{kind:'file',relativePath:bad}]:[],truncated:false})
    }),/QUICK_OPEN_INVALID_NATIVE_ENTRY/);
  }
});

test('reject foreign native project response, malformed or oversized native listings', async () => {
  await assert.rejects(collectQuickOpenFiles({
    projectId:'p',listDirectory:async()=>({projectId:'foreign',entries:[],truncated:false}),
  }),/QUICK_OPEN_WRONG_PROJECT/);
  await assert.rejects(collectQuickOpenFiles({
    projectId:'p',listDirectory:async()=>({entries:[],truncated:'false'}),
  }),/QUICK_OPEN_INVALID_NATIVE_LISTING/);
  await assert.rejects(collectQuickOpenFiles({
    projectId:'p',listDirectory:async()=>({entries:Array(401).fill({kind:'file',
      relativePath:'x'}),truncated:false}),
  }),/QUICK_OPEN_INVALID_NATIVE_LISTING/);
});

test('real native listing truncation and directory/file budgets remain explicit', async () => {
  const result=await collectQuickOpenFiles({
    projectId:'p',listDirectory:listing,maxDirectories:2,
  });
  assert.equal(result.truncated,true);
  assert.ok(result.directoriesVisited<=2);
  const files=await collectQuickOpenFiles({projectId:'p',listDirectory:listing,maxFiles:2});
  assert.equal(files.truncated,true);
  assert.equal(files.paths.length,2);
});

test('project-change cancellation invalidates pending native response', async () => {
  let current=true;
  await assert.rejects(collectQuickOpenFiles({
    projectId:'p',
    listDirectory:async()=>{current=false;return {entries:[],truncated:false};},
    isCurrent:()=>current
  }),/QUICK_OPEN_CANCELLED/);
});

test('filter caps results and rejects invalid limit or host-provided paths', () => {
  const paths=Array.from({length:60},(_,i)=>'src/file-'+i+'.rs');
  assert.equal(matchQuickOpenFiles(paths,'',30).length,30);
  assert.deepEqual(matchQuickOpenFiles(['../secret','src/good.rs'],'good'),['src/good.rs']);
  assert.throws(()=>matchQuickOpenFiles(paths,'',31),/QUICK_OPEN_INVALID_FILTER/);
});
