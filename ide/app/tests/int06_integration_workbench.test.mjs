import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {inspectProjectWorkbench,projectWorkbench,
  checkedWorkbenchNavigation} from '../web/integration_workbench.js';
const id='ide-'+'a'.repeat(32);
const sha='c'.repeat(64), head='d'.repeat(40);
const snapshot={
  git:{schemaVersion:1,projectId:id,state:'local_inventory_inspected',
    headState:'attached',currentHeadSha:head,branches:[],worktrees:[],
    remoteQueried:false,gitProcessStarted:false,refsOrFilesModified:false,
    linkedWorktreeAccessGranted:false},
  packages:{schemaVersion:1,projectId:id,fingerprint:sha,candidates:[],
    blockers:[],executionAuthorized:false,releaseAccepted:false,
    processStarted:false,networkAccessed:false,filesModified:false,
    resolutionSupported:false,applySupported:false},
};
test('INT06 fixed cross-tool view cannot approve work or execution',()=>{
  const w=projectWorkbench(id,snapshot);
  assert.equal(w.rows.length,9);
  assert.equal(w.executionAuthorized,false);
  assert.equal(w.acceptanceVerified,false);
  assert.equal(w.installedAppAccepted,false);
  assert.equal(w.rows.find(x=>x.id==='source').state,'observed');
  assert.equal(w.rows.find(x=>x.id==='acceptance').state,'unverified');
  assert.ok(w.rows.every(r=>r.executionAuthorized===false));
});
test('INT06 stale or untrusted native Git/package identity fails closed',()=>{
  const old={...snapshot,git:{...snapshot.git,projectId:'ide-'+'b'.repeat(32)}};
  assert.equal(projectWorkbench(id,old).rows.find(x=>x.id==='source').state,'unverified');
  const bad={...snapshot,packages:{...snapshot.packages,networkAccessed:true}};
  assert.equal(projectWorkbench(id,bad).rows.find(x=>x.id==='packages').state,'unverified');
  const forge={...snapshot,git:{...snapshot.git,currentHeadSha:'e'.repeat(64)}};
  assert.equal(projectWorkbench(id,forge).rows.find(x=>x.id==='source').state,'unverified');
});
test('INT06 recovery always precedes source review or build workflow',()=>{
  const w=projectWorkbench(id,{...snapshot,build:{
    quarantined:[{requiresReconciliation:true}],journal:[],
  }});
  assert.equal(w.recoveryRequired,true);
  assert.equal(w.priority,'build');
  assert.equal(w.rows.find(x=>x.id==='build').state,'reconciliation_required');
});
test('INT06 all seven read-only probes and no mutation',async()=>{
  const calls=[];
  const answers={
    ide_read_git_repository_inventory:snapshot.git,
    ide_inspect_package_managers:snapshot.packages,
  };
  const w=await inspectProjectWorkbench(id,(name,args)=>{
    calls.push([name,args]);return Promise.resolve(answers[name]||null);
  },()=>true);
  assert.ok(w);
  assert.deepEqual(calls.map(c=>c[0]).sort(),[
    'ide_build_status','ide_debug_status','ide_discover_project_run_profiles',
    'ide_inspect_package_managers','ide_lsp_worker_status',
    'ide_read_git_repository_inventory','ide_read_project_context',
  ].sort());
  assert.deepEqual(calls.find(c=>c[0]==='ide_lsp_worker_status')[1],undefined);
  assert.ok(calls.filter(c=>c[1]).every(c=>c[1].projectId===id));
  assert.equal(w.executionAuthorized,false);
});
test('INT06 interrupted project switch discards late read-only results',async()=>{
  let resolve;
  const pending=new Promise(r=>{resolve=r});
  const promise=inspectProjectWorkbench(id,()=>pending,()=>false);
  resolve({});
  assert.equal(await promise,null);
});
test('INT06 route rejects different project and cannot launch',()=>{
  const w=projectWorkbench(id,snapshot);
  assert.deepEqual(checkedWorkbenchNavigation(w,'source',id),{
    section:'source',projectId:id,executionAuthorized:false,
  });
  assert.equal(checkedWorkbenchNavigation(w,'source','ide-'+'b'.repeat(32)),null);
  assert.equal(checkedWorkbenchNavigation(w,'arbitrary_shell',id),null);
});
test('INT06 desktop links panel only to read-only observation',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(ui,/inspectProjectWorkbench/);
  assert.match(ui,/checkedWorkbenchNavigation/);
  assert.match(ui,/IDE-INT06 Workbench/);
  assert.match(ui,/Non-atomic observations/);
  assert.doesNotMatch(ui,/shell:allow-execute|Command\.create\(/);
});

test('INT06 continuity records are historical, not approval or release',()=>{
  const ledger={schemaVersion:1,projectId:id,events:[],tasks:[{
    taskId:'IDE-INT06',status:'completed',requiredGates:['source_review'],
    acceptanceGates:[],taskAcceptance:'not_verified',
  }]};
  const w=projectWorkbench(id,{...snapshot,continuity:ledger});
  assert.equal(w.rows.find(x=>x.id==='continuity').state,'observed');
  assert.equal(w.rows.find(x=>x.id==='acceptance').state,'unverified');
  assert.equal(w.acceptanceVerified,false);
  assert.equal(checkedWorkbenchNavigation(w,'continuity',id).section,'continuity');
  const foreign=projectWorkbench(id,{...snapshot,continuity:{
    ...ledger,projectId:'ide-'+'b'.repeat(32)}});
  assert.equal(foreign.rows.find(x=>x.id==='continuity').state,'unverified');
});
