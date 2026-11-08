import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {rememberedWorkspaceFromTabs,checkedRememberedWorkspace,
  revalidatedResumeDecision} from '../web/workspace_layout.js';

const A='ide-'+'a'.repeat(32),B='ide-'+'b'.repeat(32);
const H='a'.repeat(64),NEXT='b'.repeat(64);
const tab=(i,p=A,opts={})=>({
  key:p+':src/file'+i+'.rs',projectId:p,relativePath:'src/file'+i+'.rs',
  revision:H,savedText:'saved file '+i,draft:'saved file '+i,
  dirty:false,bufferRevision:0,...opts,
});
const snapshot=(tabs,primary=tabs[0]?.key,second=null,split=false,live=null)=>
  rememberedWorkspaceFromTabs(tabs,A,primary,second,split,0,
    key=>live?.get(key)??tabs.find(t=>t.key===key)?.draft);
const preview=(file,sha=H)=>({
  projectId:A,relativePath:file.relativePath,revision:sha,text:'verified saved source',
});

test('only navigation fields, hashes and dirty indicator survive serialization',()=>{
  const files=[tab(0),tab(1,A,{draft:'secret unsaved code',dirty:true})];
  const layout=snapshot(files,files[0].key,files[1].key,true);
  assert.deepEqual(Object.keys(layout).sort(),[
    'schemaVersion','projectId','files','primaryPath','secondaryPath',
    'splitOpen','tabWindowStart',
  ].sort());
  assert.deepEqual(layout.files.map(file=>file.wasDirty),[false,true]);
  assert.equal(JSON.stringify(layout).includes('secret unsaved code'),false);
  assert.equal(JSON.stringify(layout).includes('saved file 0'),false);
  assert.equal(layout.primaryPath,files[0].relativePath);
  assert.equal(layout.secondaryPath,files[1].relativePath);
  assert.deepEqual(checkedRememberedWorkspace(layout,A).files,layout.files);
});

test('in-flight typing newer than React is recorded as dirty without source bytes',()=>{
  const file=tab(1);
  const live=new Map([[file.key,'modified after render']]);
  const layout=snapshot([file],file.key,null,false,live);
  assert.equal(layout.files[0].wasDirty,true);
  assert.equal(JSON.stringify(layout).includes('modified after render'),false);
});

test('foreign project path and traversal are not saved',()=>{
  const source=tab(0);
  assert.throws(()=>snapshot([{...source,relativePath:'../bad.rs'}]),/B9_UNTRUSTED_TAB/);
  assert.throws(()=>snapshot([tab(0,B)]),/B9_TAB_BUDGET/);
  assert.throws(()=>snapshot([tab(0,A,{revision:'not a hash'})]),/B9_UNTRUSTED_TAB/);
  assert.throws(()=>snapshot([{...source,key:'spoofed'}]),/B9_UNTRUSTED_TAB/);
});

test('48 file bound and duplicate paths deny whole snapshot',()=>{
  const limit=Array.from({length:48},(_,i)=>tab(i));
  assert.equal(snapshot(limit).files.length,48);
  assert.throws(()=>snapshot([...limit,tab(48)]),/B9_TAB_BUDGET/);
  assert.throws(()=>snapshot([tab(0),tab(0)]),/B9_UNTRUSTED_TAB/);
});

test('stored unknown or damaged metadata cannot be presented as a trusted plan',()=>{
  const base=snapshot([tab(0)]);
  assert.throws(()=>checkedRememberedWorkspace(base,B),/B9_INVALID_STORED_WORKSPACE/);
  assert.throws(()=>checkedRememberedWorkspace({...base,files:[{
    ...base.files[0],relativePath:'/etc/passwd'}]},A),/B9_INVALID_STORED_WORKSPACE/);
  assert.throws(()=>checkedRememberedWorkspace({...base,files:[
    base.files[0],base.files[0]]},A),/B9_INVALID_STORED_WORKSPACE/);
  assert.throws(()=>checkedRememberedWorkspace({...base,primaryPath:'missing.rs'},A),
    /B9_INVALID_STORED_WORKSPACE/);
});

test('unknown stored fields are not permitted to become source or worker hints',()=>{
  const value=snapshot([tab(0)]);
  const recovered=checkedRememberedWorkspace({
    ...value,privateDraftText:'DO_NOT_PERSIST',runWorker:true,
    files:[{...value.files[0],absolutePath:'/private/path'}],
  },A);
  assert.equal(JSON.stringify(recovered).includes('DO_NOT_PERSIST'),false);
  assert.equal(JSON.stringify(recovered).includes('/private/path'),false);
  assert.equal(JSON.stringify(recovered).includes('runWorker'),false);
});

test('native restore requires exact registered project and returned relative path',()=>{
  const layout=snapshot([tab(0)]),file=layout.files[0];
  assert.equal(revalidatedResumeDecision(layout,file,preview(file),A).action,'open_saved');
  assert.equal(revalidatedResumeDecision(layout,file,preview(file),B).action,'skip');
  assert.equal(revalidatedResumeDecision(layout,file,{
    ...preview(file),relativePath:'wrong.rs'},A).action,'skip');
  assert.equal(revalidatedResumeDecision(layout,file,{...preview(file),text:null},A).action,
    'skip');
});

test('changed saved disk revision is recorded, never treated as old draft',()=>{
  const layout=snapshot([tab(0,A,{draft:'private dirty',dirty:true})]);
  const outcome=revalidatedResumeDecision(layout,layout.files[0],
    preview(layout.files[0],NEXT),A);
  assert.deepEqual(outcome,{
    action:'open_saved',changedOnDisk:true,hadUnsavedDraft:true,
  });
  assert.equal(JSON.stringify(layout).includes('private dirty'),false);
});

test('no automatically opened files or source writes in frontend command wiring',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const native=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const host=readFileSync(new URL('../../host/src/workspace_layout.rs',import.meta.url),'utf8');
  assert.match(ui,/aria-label="Remember open tabs for restart"/);
  assert.match(ui,/aria-label="Restore remembered saved tabs"/);
  assert.match(ui,/aria-label="Forget remembered tab layout"/);
  assert.match(ui,/invoke\('ide_workspace_layout_save',\{layout\}\)/);
  assert.match(ui,/invoke\('ide_workspace_layout_read',\{projectId\}\)/);
  assert.match(ui,/invoke\('ide_workspace_layout_clear',\{projectId\}\)/);
  assert.match(ui,/revalidatedResumeDecision\(layout,entry,preview,projectId\)/);
  assert.match(ui,/const ticket=beginExplorerIo\(\)/);
  assert.match(native,/ide_workspace_layout_save,/);
  assert.match(native,/ide_workspace_layout_read,/);
  assert.match(host,/registry\.preview_source_file\(/);
  assert.match(host,/source\.revision != file\.saved_revision/);
  assert.match(host,/options\.write\(true\)\.create_new\(true\)/);
  const effect=ui.slice(ui.indexOf("void invoke('ide_workspace_layout_read'"),
    ui.indexOf("},[activeProject?.projectId]);"));
  assert.doesNotMatch(effect,/ide_preview_source_file|setTabs\(/);
  const handler=ui.slice(ui.indexOf('async function resumeRememberedWorkspace()'),
    ui.indexOf('function closeCleanBackgroundTabs()'));
  assert.doesNotMatch(handler,/ide_save_source_file|ide_draft_load|ide_lsp_start_session/);
});
