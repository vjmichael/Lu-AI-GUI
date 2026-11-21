import test from 'node:test';
import assert from 'node:assert/strict';
import {
  projectProblems, stillCurrentProblem, MAX_PROJECT_PROBLEMS,
} from '../web/lsp_problems.js';

const sha = 'a'.repeat(64);
const tab = (key, relativePath, projectId = 'ide-project-one') => ({
  key, relativePath, projectId, revision: sha, bufferRevision: 3,
  generation: 2, draft: 'let x = 1;\nlet y = 2;\n',
});
const response = (file, diagnostics) => ({
  source: {
    projectId: file.projectId, relativePath: file.relativePath,
    diskSha256: file.revision, bufferRevision: file.bufferRevision,
    languageId: 'rust',
  },
  diagnostics,
});
const diagnostic = (severity, message, line = 0) => ({
  range: {start:{line,character:0},end:{line,character:3}},
  severity,message,
});

test('project Problems gathers only exact native source-revision matches in open project tabs',()=>{
  const a=tab('a','src/a.rs'), b=tab('b','src/b.rs'),
    foreign=tab('foreign','secret.rs','ide-other-project');
  const out=projectProblems([a,b,foreign], {
    a:response(a,[diagnostic(2,'warning in a'),diagnostic(1,'error in a',1)]),
    b:response(b,[diagnostic(4,'info in b')]),
    foreign:response(foreign,[diagnostic(1,'secret should not appear')])
  },a.projectId);
  assert.equal(out.total,3);
  assert.equal(out.errors,1);
  assert.equal(out.warnings,1);
  assert.equal(out.infos,1);
  assert.equal(out.verifiedTabs,2);
  assert.deepEqual(out.items.map(item=>item.message),
    ['error in a','warning in a','info in b']);
  assert.ok(out.items.every(item=>item.projectId==='ide-project-one'));
  assert.equal(out.items.find(item=>item.message==='error in a').line,2);
});

test('stale source hash/buffer changes and source-language mismatch are hidden, not navigable',()=>{
  const a=tab('a','src/a.rs');
  const native=response(a,[diagnostic(1,'error')]);
  const problem=projectProblems([a],{a:native},a.projectId).items[0];
  assert.equal(stillCurrentProblem(a,native,problem),true);
  assert.equal(stillCurrentProblem({...a,bufferRevision:4},native,problem),false);
  assert.equal(stillCurrentProblem({...a,revision:'b'.repeat(64)},native,problem),false);
  assert.equal(stillCurrentProblem({...a,generation:3},native,problem),false);
  const stale=projectProblems([{...a,bufferRevision:4}],{a:native},a.projectId);
  assert.equal(stale.items.length,0);
  assert.equal(stale.staleTabs,1);
  assert.equal(stale.verifiedTabs,0);
  const badLanguage=projectProblems([a],{
    a:{...native,source:{...native.source,languageId:'python'}}
  },a.projectId);
  assert.equal(badLanguage.total,0);
  assert.equal(badLanguage.staleTabs,1);
});

test('query and severity filter only exact verified diagnostic messages and paths',()=>{
  const a=tab('a','src/logic.rs');
  const values={a:response(a,[
    diagnostic(1,'broken type'),diagnostic(2,'unused variable'),diagnostic(3,'hint'),
  ])};
  const s=projectProblems([a],values,a.projectId,{severity:'warning',query:'VARIABLE'});
  assert.deepEqual(s.items.map(item=>item.message),['unused variable']);
  assert.equal(s.total,3);
  assert.equal(s.errors,1);
  assert.equal(s.verifiedTabs,1);
  assert.equal(projectProblems([a],values,a.projectId,{query:'LOGIC.RS'}).items.length,3);
  assert.equal(projectProblems([a],values,a.projectId,{query:'unknown'}).items.length,0);
});

test('invalid UTF-16 spans or oversized messages are excluded by native-matched presentation',()=>{
  const a=tab('a','src/a.rs');
  const result=response(a,[
    diagnostic(1,'ok'),
    diagnostic(1,'x'.repeat(4097)),
    {range:{start:{line:99,character:0},end:{line:99,character:2}},
      severity:1,message:'invalid line'},
    {range:{start:{line:0,character:7},end:{line:0,character:1}},
      severity:1,message:'backward'},
  ]);
  const out=projectProblems([a],{a:result},a.projectId);
  assert.deepEqual(out.items.map(item=>item.message),['ok']);
  assert.equal(out.total,1);
});

test('tab and result limits are explicit, not fake full-project coverage',()=>{
  const tabs=Array.from({length:70},(_,i)=>tab('key'+i,
    'src/file'+String(i).padStart(3,'0')+'.rs'));
  const result=Object.fromEntries(tabs.map(file=>[file.key,
    response(file,[diagnostic(1,'x')])]));
  const capped=projectProblems(tabs,result,tabs[0].projectId);
  assert.equal(capped.searchedTabs,64);
  assert.equal(capped.verifiedTabs,64);
  assert.equal(capped.capacityLimited,true);
  assert.equal(capped.truncated,true);
  const many=tab('many','src/many.rs');
  const docs=Array.from({length:200},(_,i)=>diagnostic(1,'error '+i));
  const items=projectProblems([many],{many:response(many,docs)},many.projectId,
    {limit:20});
  assert.equal(items.items.length,20);
  assert.equal(items.total,200);
  assert.equal(items.capacityLimited,true);
  assert.equal(MAX_PROJECT_PROBLEMS,200);
});

test('later-file errors outrank earlier-file warning flood within display budget',()=>{
  const a=tab('a','a.rs'), b=tab('b','b.rs');
  const warnings=Array.from({length:200},(_,i)=>diagnostic(2,'warning '+i));
  const snapshot=projectProblems([a,b],{
    a:response(a,warnings), b:response(b,[diagnostic(1,'critical later file')])
  },a.projectId);
  assert.equal(snapshot.total,201);
  assert.equal(snapshot.items.length,200);
  assert.equal(snapshot.items[0].message,'critical later file');
  assert.equal(snapshot.items[0].relativePath,'b.rs');
  assert.equal(snapshot.capacityLimited,true);
  assert.equal(snapshot.truncated,true);
});

test('invalid query/budget/data rejects rather than accepting untrusted inputs',()=>{
  const file=tab('a','src/a.rs');
  for(const options of [{query:'a'.repeat(129)},{severity:'critical'},{limit:201},
    {limit:0}]) {
    assert.throws(()=>projectProblems([file],{},file.projectId,options),
      /LSP_PROBLEMS_INVALID_SCOPE/);
  }
  assert.throws(()=>projectProblems('not a list',{},file.projectId),
    /LSP_PROBLEMS_INVALID_SCOPE/);
  assert.throws(()=>projectProblems([file],null,file.projectId),
    /LSP_PROBLEMS_INVALID_SCOPE/);
});
