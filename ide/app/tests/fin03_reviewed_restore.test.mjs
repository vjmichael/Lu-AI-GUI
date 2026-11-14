import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const host=read('../../host/src/git_reviewed_restore.rs');
const tauri=read('../src-tauri/src/lib.rs');
test('FIN03 one-file worktree restore preserves original and excludes shell or ref changes',()=>{
 assert.match(host,/Status::WT_MODIFIED/);
 assert.match(host,/statuses\.len\(\)!=1/);
 assert.match(host,/get_path\(Path::new\(&path\),0\)/);
 assert.match(host,/create_new\(true\)/);
 assert.match(host,/backup_file\.write_all\(&original\)/);
 assert.match(host,/marker_file\.write_all/);
 assert.match(host,/cancel\.compare_exchange\(0,1,Ordering::SeqCst/);
 assert.match(host,/FIN03_RESTORE_READBACK_MISMATCH/);
 assert.doesNotMatch(host,/std::process::Command|Command::new|\.push\(|\.commit\(|\.fetch\(/);
});
test('FIN03 restore requires native one-use consent, no renderer authority',()=>{
 assert.match(tauri,/async fn ide_git_review_single_file_restore/);
 assert.match(tauri,/async fn ide_git_execute_single_file_restore/);
 assert.match(tauri,/sessions\.begin\(&id\)/);
 assert.match(tauri,/title\("Lu IDE · One-use native file restore"\)/);
 assert.match(tauri,/FIN03_RESTORE_HUMAN_DENIED/);
 assert.match(tauri,/ide_git_single_file_restore_recovery,/);
});

test('FIN03 restore remains opt-in and disabled in default app profile',()=>{
 const cargo=read('../src-tauri/Cargo.toml');
 assert.match(cargo,/fin03-reviewed-restore = \[\]/);
 assert.match(tauri,/if !cfg!\(feature="fin03-reviewed-restore"\)/);
 assert.match(tauri,/FIN03_RESTORE_OPT_IN_AND_NATIVE_ACCEPTANCE_REQUIRED/);
 assert.match(host,/changed_path_is_plain_safe/);
 assert.match(host,/output\.set_permissions\(target_permissions\)/);
});
