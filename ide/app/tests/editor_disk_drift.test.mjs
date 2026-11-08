import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  diskAuditTargets,classifyDiskAudit,summarizeExternalDifference,
  MAX_DISK_AUDIT_TABS,COMPARISON_VISIBLE_LINES,COMPARISON_LINE_CHARS,
} from '../web/editor_disk_drift.js';

const A='ide-'+'a'.repeat(32),B='ide-'+'b'.repeat(32);
const H='a'.repeat(64),NEW='b'.repeat(64);
const tab=(i,p=A,options={})=>({
  key:p+':src/file'+i+'.rs',projectId:p,relativePath:'src/file'+i+'.rs',
  revision:H,savedText:'fn old() {}\n',draft:'fn old() {}\n',
  dirty:false,generation:0,bufferRevision:0,...options,
});
const native=(t,rev=H)=>({
  projectId:t.projectId,relativePath:t.relativePath,revision:rev,text:'native disk',
});

test('audit is limited to 48 registered project-relative tabs with no code text',()=>{
  const tabs=Array.from({length:48},(_,i)=>tab(i));
  tabs.push(tab(0,B));
  const targets=diskAuditTargets(tabs,A);
  assert.equal(MAX_DISK_AUDIT_TABS,48);
  assert.equal(targets.length,48);
  assert.equal(JSON.stringify(targets).includes('fn old'),false);
  assert.deepEqual(Object.keys(targets[0]).sort(),[
    'projectId','key','relativePath','savedRevision','generation','bufferRevision',
  ].sort());
  assert.throws(()=>diskAuditTargets([...tabs,tab(99)],A),/B10_AUDIT_BUDGET/);
});

test('path traversal, fake tab identity and duplicate key deny inspection',()=>{
  const t=tab(1);
  assert.throws(()=>diskAuditTargets([{...t,relativePath:'../outside'}],A),
    /B10_INVALID_SOURCE_IDENTITY/);
  assert.throws(()=>diskAuditTargets([{...t,key:'fake'}],A),
    /B10_INVALID_SOURCE_IDENTITY/);
  assert.throws(()=>diskAuditTargets([t,t],A),/B10_INVALID_SOURCE_IDENTITY/);
  assert.throws(()=>diskAuditTargets([{...t,revision:'bad'}],A),
    /B10_INVALID_SOURCE_IDENTITY/);
  assert.throws(()=>diskAuditTargets([t],B),/B10_INVALID_AUDIT|B10_INVALID_SOURCE_IDENTITY|B10_AUDIT_BUDGET/);
});

test('same saved SHA is current, external revision is changed',()=>{
  const t=tab(1),target=diskAuditTargets([t],A)[0];
  assert.deepEqual(classifyDiskAudit(target,t,native(t),A,true),
    {status:'current',diskRevision:H});
  assert.deepEqual(classifyDiskAudit(target,t,native(t,NEW),A,true),
    {status:'changed',diskRevision:NEW});
});

test('after edit, reload, save or project switch the disk response is stale',()=>{
  const t=tab(3),target=diskAuditTargets([t],A)[0],p=native(t,NEW);
  for (const next of [
    {...t,bufferRevision:2},{...t,generation:1},{...t,revision:NEW},
    {...t,projectId:B},null,
  ]) {
    assert.equal(classifyDiskAudit(target,next,p,A,true).status,'stale');
  }
  assert.equal(classifyDiskAudit(target,t,p,B,true).status,'stale');
  assert.equal(classifyDiskAudit(target,t,p,A,false).status,'stale');
});

test('native identity mismatch is untrusted and must never drive actions',()=>{
  const t=tab(3),target=diskAuditTargets([t],A)[0];
  assert.equal(classifyDiskAudit(target,t,{...native(t),projectId:B},A,true).status,
    'invalid_native_result');
  assert.equal(classifyDiskAudit(target,t,{...native(t),relativePath:'other'},A,true).status,
    'invalid_native_result');
  assert.equal(classifyDiskAudit(target,t,{...native(t),text:null},A,true).status,
    'invalid_native_result');
  assert.equal(classifyDiskAudit(target,t,null,A,true).status,'invalid_native_result');
});

test('three-way comparison distinguishes original saved file, live draft and disk',()=>{
  const comparison=summarizeExternalDifference(
    'alpha\nbeta\ngamma','alpha\nLOCAL\ngamma','alpha\nEXTERNAL\ngamma');
  assert.equal(comparison.localUnsaved,true);
  assert.equal(comparison.savedVersusDisk.identical,false);
  assert.equal(comparison.draftVersusDisk.identical,false);
  assert.equal(comparison.savedVersusDisk.firstDifferentLine,2);
  assert.deepEqual(comparison.savedVersusDisk.left.lines,['beta']);
  assert.deepEqual(comparison.draftVersusDisk.left.lines,['LOCAL']);
  assert.deepEqual(comparison.draftVersusDisk.right.lines,['EXTERNAL']);
  assert.equal(comparison.savedVersusDisk.method,
    'bounded-first-differing-region-not-merge');
});

test('CRLF/LF drift is visible even when normalized logical lines match',()=>{
  const result=summarizeExternalDifference('one\r\ntwo\r\n','one\r\ntwo\r\n','one\ntwo\n');
  assert.equal(result.savedVersusDisk.identical,false);
  assert.equal(result.savedVersusDisk.lineEndingOnlyDifference,true);
  assert.equal(result.savedVersusDisk.leftChangedLines,0);
  assert.equal(result.savedVersusDisk.rightChangedLines,0);
});

test('unchanged disk comparison admits zero changed lines, no implied merge',()=>{
  const result=summarizeExternalDifference('a\nb\n','a\nb\n','a\nb\n');
  assert.equal(result.savedVersusDisk.identical,true);
  assert.equal(result.draftVersusDisk.identical,true);
  assert.equal(result.localUnsaved,false);
});

test('bounded excerpts truncate enormous lines and skip excess changed lines',()=>{
  const a=Array.from({length:25},(_,i)=>'a'+i+'#'.repeat(300)).join('\n');
  const b=Array.from({length:25},(_,i)=>'b'+i+'#'.repeat(300)).join('\n');
  const result=summarizeExternalDifference(a,a,b);
  assert.equal(result.savedVersusDisk.left.lines.length,COMPARISON_VISIBLE_LINES);
  assert.ok(result.savedVersusDisk.left.lines.every(x=>x.length<=COMPARISON_LINE_CHARS));
  assert.ok(result.savedVersusDisk.left.moreLines>0);
  assert.equal(result.savedVersusDisk.left.clippedLines,COMPARISON_VISIBLE_LINES);
  assert.throws(()=>summarizeExternalDifference('a'.repeat(524289),'',''),
    /B10_COMPARE_TOO_LARGE/);
});

test('frontend inspection and comparison are always explicit, read-only native previews',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(ui,/aria-label="Check open tabs for external disk changes"/);
  assert.match(ui,/aria-label="Compare current editor with saved disk"/);
  assert.match(ui,/aria-label="External file changes"/);
  assert.match(ui,/aria-label="Read-only external source comparison"/);
  assert.match(ui,/classifyDiskAudit\(target,current,preview,projectId,/);
  assert.match(ui,/summarizeExternalDifference\(/);
  assert.match(ui,/const ticket=beginExplorerIo\(\)/);
  const start=ui.indexOf('async function inspectOpenTabDiskRevisions()');
  const end=ui.indexOf('function closeCleanBackgroundTabs()',start);
  assert.ok(start>=0 && end>start);
  const code=ui.slice(start,end);
  assert.match(code,/invoke\('ide_preview_source_file'/);
  assert.doesNotMatch(code,/invoke\('ide_save_source_file'/);
  assert.doesNotMatch(code,/invoke\('ide_draft_load'/);
  assert.doesNotMatch(code,/setTabs\(/);
  assert.doesNotMatch(code,/resetBufferIdentity\(/);
  assert.match(code,/explorerIoTicket\.current!==ticket/);
  assert.match(code,/activeProjectIdRef\.current!==projectId/);
});
