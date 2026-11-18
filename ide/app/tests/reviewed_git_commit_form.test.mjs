import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {readReviewedCommitMetadata,matchesReviewedCommitReceipt} from '../web/reviewed_git_commit_form.js';
const form=() => { const f=new FormData(); f.set('message','One reviewed file'); f.set('authorName','Author'); f.set('authorEmail','author@localhost.invalid'); return f; };
test('native commit input uses submitted data and preserves exact reviewed values',()=>{
  const f=form(); f.set('message','  Reviewed message  ');
  assert.deepEqual(readReviewedCommitMetadata(f),{message:'  Reviewed message  ',authorName:'Author',authorEmail:'author@localhost.invalid'});
  f.set('authorName','Updated after native paste');
  assert.equal(readReviewedCommitMetadata(f).authorName,'Updated after native paste');
});
test('missing, whitespace, duplicate, control and oversized fields fail closed',()=>{
  for(const key of ['message','authorName','authorEmail']) {
    for(const bad of ['','   ','bad\nvalue','bad\0value','bad\u0085value','é'.repeat(1201)]) {
      const f=form(); f.set(key,bad); assert.equal(readReviewedCommitMetadata(f),null,key);
    }
    const missing=form(); missing.delete(key); assert.equal(readReviewedCommitMetadata(missing),null);
    const dup=form(); dup.append(key,'other'); assert.equal(readReviewedCommitMetadata(dup),null);
  }
  const f=form(); f.set('authorName','é'.repeat(49)); assert.equal(readReviewedCommitMetadata(f),null);
  assert.equal(readReviewedCommitMetadata(null),null);
});
test('actual form retains project, stage and native commit consent requirements',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const action=ui.split('async function commitReviewedGitFile(event)')[1].split('function addWorkflowStep')[0];
  assert.match(action,/gitStageReceipt\.projectId !== activeProject\.projectId/);
  assert.match(action,/readReviewedCommitMetadata\(new FormData\(form\)\)/);
  assert.ok(action.indexOf('if (!metadata || !form.reportValidity())')<action.indexOf("invoke('ide_git_commit_reviewed'"));
  assert.ok(action.indexOf('gitCommitInFlight.current=true')<action.indexOf("invoke('ide_git_commit_reviewed'"));
  assert.match(action,/gitCommitInFlight\.current \|\|/);
  assert.match(action,/matchesReviewedCommitReceipt\(stage, done\)/);
  assert.match(ui,/onClick=\{commitReviewedGitFile\}/);
  assert.match(ui,/onSubmit=\{commitReviewedGitFile\}/);
  const command=host.split('async fn ide_git_commit_reviewed(')[1].split('/// WO9-A/B:')[0];
  assert.match(command,/\.parent\(&owner\)/);
  assert.ok(command.indexOf('if !allow')<command.indexOf('registry.commit_reviewed_git_file'));
});
test('source action awaits native fixed confirmation before rechecking live editor context',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const action=ui.split("const approved=await invoke('ide_confirm_source_action'")[1];
  assert.ok(action.indexOf('if (approved!==true) return;')<action.indexOf('SOURCE_ACTION_CONTEXT_CHANGED'));
  assert.ok(action.indexOf('SOURCE_ACTION_CONTEXT_CHANGED')<action.indexOf('const receipt=await invoke(command,params)'));
  const confirm=host.split('async fn ide_confirm_source_action(')[1].split('/// Exact-source no-clobber')[0];
  assert.match(confirm,/preview\.revision != expected_revision/);
  assert.match(confirm,/item\.revision == expected_revision/);
  assert.match(confirm,/item\.payload_present && !item\.original_present/);
  assert.match(confirm,/SOURCE_CONFIRM_INVALID_ACTION/);
  assert.match(confirm,/\.blocking_show\(\)/);
  assert.doesNotMatch(confirm,/registry\.(move_source_file|trash_source_file|restore_source_file)\(/);
  assert.match(host,/\n\s+ide_confirm_source_action,/);
});

test('reviewed commit receipt must match exact staged source, scope, parent and no remote authority',()=>{
  const stage={projectId:'p',relativePath:'a.rs',baseHead:'a'.repeat(40),sourceSha256:'b'.repeat(64)};
  const receipt={projectId:'p',relativePath:'a.rs',previousHead:stage.baseHead,sourceSha256:stage.sourceSha256,
    commitSha:'c'.repeat(40),gitHooksExecuted:false,remoteUpdated:false,releaseApproved:false};
  assert.equal(matchesReviewedCommitReceipt(stage,receipt),true);
  for(const [field,value] of Object.entries({projectId:'q',relativePath:'b.rs',previousHead:'d'.repeat(40),
    sourceSha256:'e'.repeat(64),commitSha:stage.baseHead,gitHooksExecuted:true,remoteUpdated:true,releaseApproved:true}))
    assert.equal(matchesReviewedCommitReceipt(stage,{...receipt,[field]:value}),false,field);
  assert.equal(matchesReviewedCommitReceipt(stage,null),false);
});
