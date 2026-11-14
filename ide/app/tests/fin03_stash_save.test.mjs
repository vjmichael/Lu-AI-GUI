import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../../host/src/git_reviewed_stash_save.rs');
const commands=read('../src-tauri/src/lib.rs');
const cargo=read('../src-tauri/Cargo.toml');
test('FIN03 stash save is a reviewed native one-file mutation, never a shell',()=>{
 assert.match(host,/review_single_file_restore\(id\)/);
 assert.match(host,/execute_reviewed_one_file_stash_save/);
 assert.match(host,/source_again\.review_sha256!=fresh\.source_review_sha256/);
 assert.match(host,/stash_save\(&signature/);
 assert.match(host,/FIN03_STASH_REF_READBACK_UNCERTAIN/);
 assert.match(host,/FIN03_STASH_WORKTREE_READBACK_UNCERTAIN/);
 assert.match(host,/FIN03_STASH_ORIGINAL_BACKUP_CHANGED/);
 assert.doesNotMatch(host,/std::process::Command|Command::new|\.push\(/);
});
test('FIN03 stash mutation has native OS dialog and default-deny feature',()=>{
 assert.match(cargo,/fin03-reviewed-stash = \[\]/);
 assert.match(commands,/if !cfg!\(feature="fin03-reviewed-stash"\)/);
 assert.match(commands,/FIN03_STASH_NATIVE_ACCEPTANCE_REQUIRED/);
 assert.match(commands,/title\("Lu IDE · One-use protected stash save"\)/);
 assert.match(commands,/FIN03_STASH_HUMAN_DENIED/);
 assert.match(commands,/ide_git_one_file_stash_recovery,/);
});
test('FIN03 recoverability requires original backup and never auto replays',()=>{
 assert.match(host,/FIN03_STASH_ORIGINAL_BACKUP_MISSING/);
 assert.match(host,/FIN03_STASH_JOURNAL_INVALID/);
 assert.match(host,/FIN03_STASH_GIT_DIRECTORY_SYNC_FAILED/);
 assert.doesNotMatch(host,/stash_apply|stash_drop/);
});
