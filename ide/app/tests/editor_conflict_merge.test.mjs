import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  planThreeWayMerge,renderReviewedMerge,conflictExcerpt,mergeCanApply,
  MAX_MERGE_LINES,MAX_MERGE_CHARS,MAX_MERGE_CONFLICTS,
  MAX_MERGE_PREVIEW_LINES,MAX_MERGE_PREVIEW_CHARS,
} from '../web/editor_conflict_merge.js';

function resolve(base,draft,disk,choices={}) {
  const plan=planThreeWayMerge(base,draft,disk);
  return {plan,answer:renderReviewedMerge(plan,choices)};
}
const A='ide-'+'a'.repeat(32);
const B='ide-'+'b'.repeat(32);
const SHA='a'.repeat(64),DISK='b'.repeat(64);
const state=text=>({sliceDoc:()=>text});
const fixture=(text='local')=>{
  const editorState=state(text);
  const tab={key:A+':src/test.rs',projectId:A,relativePath:'src/test.rs',
    revision:SHA,generation:3,bufferRevision:7,savedText:'base',draft:text};
  const review={projectId:A,tabKey:tab.key,relativePath:tab.relativePath,
    savedRevision:tab.revision,generation:tab.generation,
    bufferRevision:tab.bufferRevision,savedText:'base',draftText:text,
    diskRevision:DISK,diskText:'external',editorState};
  const preview={projectId:A,relativePath:tab.relativePath,
    revision:DISK,text:'external'};
  return {editorState,tab,review,preview};
};

test('nonoverlapping edits to distinct lines combine without a conflict',()=>{
  const {plan,answer}=resolve('a\nb\nc\nd\n','a\nLOCAL\nc\nd\n',
    'a\nb\nc\nEXTERNAL\n');
  assert.equal(plan.conflicts.length,0);
  assert.equal(answer.ready,true);
  assert.equal(answer.text,'a\nLOCAL\nc\nEXTERNAL\n');
});

test('same-line divergent edits remain blocked until explicit human choice',()=>{
  const plan=planThreeWayMerge('alpha\nbeta\nomega\n',
    'alpha\nLOCAL\nomega\n','alpha\nEXTERNAL\nomega\n');
  assert.equal(plan.conflicts.length,1);
  assert.deepEqual(renderReviewedMerge(plan,{}),{
    ready:false,missing:'conflict-1',text:null});
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'draft'}).text,
    'alpha\nLOCAL\nomega\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'disk'}).text,
    'alpha\nEXTERNAL\nomega\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'base'}).text,
    'alpha\nbeta\nomega\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'invalid'}).ready,false);
});

test('two simultaneous insertions at the same boundary conflict',()=>{
  const base='before\nafter\n';
  const plan=planThreeWayMerge(base,'before\nlocal\nafter\n',
    'before\ndisk\nafter\n');
  assert.equal(plan.conflicts.length,1);
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'draft'}).text,
    'before\nlocal\nafter\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'disk'}).text,
    'before\ndisk\nafter\n');
});

test('distinct insertion anchors combine in original order',()=>{
  const result=resolve('a\nb\nc\n',
    'a\nLOCAL\nb\nc\n','a\nb\nDISK\nc\n');
  assert.equal(result.plan.conflicts.length,0);
  assert.equal(result.answer.text,'a\nLOCAL\nb\nDISK\nc\n');
});

test('identical changes are resolved without asking twice',()=>{
  const result=resolve('a\nold\nz\n','a\nnew\nz\n','a\nnew\nz\n');
  assert.equal(result.plan.conflicts.length,0);
  assert.equal(result.answer.text,'a\nnew\nz\n');
});

test('deletion plus disjoint replacement retains both changes',()=>{
  const result=resolve('a\nb\nc\nd\n',
    'a\nc\nd\n','a\nb\nc\nD\n');
  assert.equal(result.plan.conflicts.length,0);
  assert.equal(result.answer.text,'a\nc\nD\n');
});

test('overlapping delete and replacement require conflict review',()=>{
  const plan=planThreeWayMerge('a\nb\nc\n','a\nc\n','a\nX\nc\n');
  assert.equal(plan.conflicts.length,1);
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'draft'}).text,'a\nc\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'disk'}).text,'a\nX\nc\n');
});

test('empty file and end-of-file insertion are not skipped',()=>{
  const plan=planThreeWayMerge('','hello\n','world\n');
  assert.equal(plan.conflicts.length,1);
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'base'}).text,'');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'draft'}).text,'hello\n');
  assert.equal(renderReviewedMerge(plan,{'conflict-1':'disk'}).text,'world\n');
});

test('exact CRLF/CR/LF source bytes and missing final newline remain intact',()=>{
  const first=resolve('one\r\ntwo\r\n',
    'ONE\r\ntwo\r\n','one\r\nTWO\r\n');
  assert.equal(first.plan.conflicts.length,0);
  assert.equal(first.answer.text,'ONE\r\nTWO\r\n');
  const second=resolve('one\rtwo','ONE\rtwo','one\rTWO');
  assert.equal(second.answer.text,'ONE\rTWO');
  const third=resolve('a\nb','a\nB','a\nb\n');
  // Changing the final line while another source adjusts its newline is
  // overlapping, so the user must explicitly pick the intended bytes.
  assert.equal(third.plan.conflicts.length,1);
  assert.equal(renderReviewedMerge(third.plan,{'conflict-1':'draft'}).text,'a\nB');
});

test('Unicode including astral code points survives without byte corruption',()=>{
  const r=resolve('🎬\nwe are filming\n終了\n',
    '🎬\nlocal change\n終了\n','🎬\nwe are filming\nфинал\n');
  assert.equal(r.answer.text,'🎬\nlocal change\nфинал\n');
});

test('bounded 600-line LCS and 128-Ki UTF-16 code-unit budget fail closed',()=>{
  const src=Array.from({length:MAX_MERGE_LINES},(_,i)=>'line'+i+'\n').join('');
  const plan=planThreeWayMerge(src,src,src);
  assert.equal(plan.conflicts.length,0);
  assert.throws(()=>planThreeWayMerge(src+'extra\n',src,src),/B11_LINE_BUDGET/);
  assert.throws(()=>planThreeWayMerge('a'.repeat(MAX_MERGE_CHARS+1),'',''),
    /B11_INPUT_TOO_LARGE/);
});

test('bounded previews do not expose more than five 180-character lines',()=>{
  const excerpt=conflictExcerpt(Array.from({length:20},
    (_,i)=>'line'+i+'x'.repeat(300)+'\n').join(''));
  assert.equal(excerpt.lines.length,MAX_MERGE_PREVIEW_LINES);
  assert.equal(excerpt.extraLines,15);
  assert.ok(excerpt.clipped);
  assert.ok(excerpt.lines.every(line=>line.length<=MAX_MERGE_PREVIEW_CHARS));
});

test('review may apply only to original registered file/draft/EditorState and disk SHA',()=>{
  const {editorState,tab,review,preview}=fixture();
  assert.equal(mergeCanApply(review,tab,editorState,A,preview),true);
  assert.equal(mergeCanApply(review,tab,state('local'),A,preview),false);
  assert.equal(mergeCanApply(review,{...tab,bufferRevision:8},
    editorState,A,preview),false);
  assert.equal(mergeCanApply(review,{...tab,generation:4},
    editorState,A,preview),false);
  assert.equal(mergeCanApply(review,{...tab,revision:DISK},
    editorState,A,preview),false);
  assert.equal(mergeCanApply(review,tab,editorState,B,preview),false);
  assert.equal(mergeCanApply(review,tab,editorState,A,
    {...preview,revision:SHA}),false);
  assert.equal(mergeCanApply(review,tab,editorState,A,
    {...preview,text:'newer external update'}),false);
});

test('native disk cannot be silently rebased in stale review',()=>{
  const {editorState,tab,review,preview}=fixture();
  const newer={...preview,revision:'c'.repeat(64)};
  assert.equal(mergeCanApply(review,tab,editorState,A,newer),false);
  assert.equal(mergeCanApply(review,tab,editorState,A,null),false);
});

test('81 distinct overlapping conflicts exceed the fail-closed review cap',()=>{
  const base=Array.from({length:162},(_,i)=>
    (i%2===0?'line':'separator')+i+'\n');
  const draft=base.map((line,i)=>i%2===0?'LOCAL '+i+'\n':line);
  const disk=base.map((line,i)=>i%2===0?'DISK '+i+'\n':line);
  assert.equal(MAX_MERGE_CONFLICTS,80);
  assert.throws(()=>planThreeWayMerge(base.join(''),draft.join(''),
    disk.join('')),/B11_CONFLICT_BUDGET/);
});

test('combining individually bounded edits cannot produce an oversized result',()=>{
  const base=Array.from({length:400},(_,i)=>'base'+i+'\n');
  const draft=base.map((line,i)=>i<200?'L'.repeat(500)+'\n':line);
  const disk=base.map((line,i)=>i>=200?'D'.repeat(500)+'\n':line);
  const plan=planThreeWayMerge(base.join(''),draft.join(''),disk.join(''));
  assert.equal(plan.conflicts.length,0);
  assert.throws(()=>renderReviewedMerge(plan,{}),/B11_MERGED_RESULT_TOO_LARGE/);
});

test('CodeMirror refuses incompatible LF/CRLF before any editor transaction',()=>{
  const cm=readFileSync(new URL('../web/code_editor.jsx',import.meta.url),'utf8');
  const before=cm.slice(cm.indexOf('if (!view || !mergeApplyRequest'),
    cm.indexOf('view.dispatch({changes:{from:0,to:view.state.doc.length,'));
  assert.match(before,/view\.state\.facet\(EditorState\.lineSeparator\)/);
  assert.match(before,/breaksMatch\(req\.mergedText\)/);
  assert.match(before,/breaksMatch\(req\.diskText\)/);
  assert.match(before,/B11_LINE_ENDING_RELOAD_REQUIRED/);
});

test('UI is explicit, requires a fresh native read and never dispatches Save',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const cm=readFileSync(new URL('../web/code_editor.jsx',import.meta.url),'utf8');
  const start=ui.indexOf('async function prepareExternalMerge(tab)');
  const end=ui.indexOf('function closeCleanBackgroundTabs()',start);
  assert.ok(start>=0 && end>start);
  const handlers=ui.slice(start,end);
  assert.match(handlers,/invoke\('ide_preview_source_file'/);
  assert.match(handlers,/mergeCanApply\(review,tab,state,activeProjectIdRef\.current,latest\)/);
  assert.match(handlers,/window\.confirm\(/);
  assert.doesNotMatch(handlers,/ide_save_source_file/);
  assert.doesNotMatch(handlers,/createEditorState\(/);
  assert.match(ui,/aria-label="Reviewed external three-way merge"/);
  assert.match(ui,/aria-label="Apply reviewed merge to unsaved editor only"/);
  assert.match(ui,/mergeApplyRequest=\{mergeApplyRequest\?\.tabKey===activeTab\.key/);
  assert.match(cm,/view\.state!==req\.expectedEditorState/);
  assert.match(cm,/view\.dispatch\(\{changes:\{from:0,to:view\.state\.doc\.length,/);
  assert.match(cm,/mergeCallback\.current\?\.\(req\.nonce,true\)/);
  const effect=cm.slice(cm.indexOf('if (!view || !mergeApplyRequest'),cm.indexOf('},[mergeApplyRequest,tabIdentity]'));
  assert.doesNotMatch(effect.replace(/\/\/[^\n]*/g,''),/createEditorState|ide_save_source_file|invoke\(/);
});
