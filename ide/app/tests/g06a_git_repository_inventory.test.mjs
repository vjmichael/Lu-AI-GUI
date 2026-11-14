import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const host = read('../../host/src/git_inventory.rs');
const hostMod = read('../../host/src/lib.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('G06A read-only inventory is scoped to registered native project', () => {
  assert.match(hostMod, /pub mod git_inventory;/);
  assert.match(host, /self\.registered_git_root\(project_id\)/);
  assert.match(host, /Repository::open\(root\)/);
  assert.match(host, /actual != root/);
  for (const prohibited of [
    'std::process::Command', 'Command::new', 'repo.checkout', 'repo.fetch',
    'repo.push', 'repo.branch(', 'repo.worktree(', 'repo.prune(',
  ]) {
    // Source fixture creation is allowed in test-only scope.
    assert.ok(!host.split('#[cfg(test)]')[0].includes(prohibited), prohibited);
  }
  assert.match(host, /linked_worktree_access_granted: false/);
  assert.match(host, /absolute_path_exposed: false/);
  assert.match(host, /const MAX_BRANCHES: usize = 96/);
  assert.match(host, /const MAX_WORKTREES: usize = 48/);
});

test('G06A Tauri route receives only the registered project ID', () => {
  const command = tauri.match(/async fn ide_read_git_repository_inventory\([\s\S]*?\n\}/)?.[0];
  assert.ok(command);
  assert.match(command, /project_id: String/);
  assert.doesNotMatch(command, /path: String|ref_name: String|command: String|token: String|grant: String/);
  assert.match(command, /registry\.inspect_git_repository_inventory\(&project_id\)/);
  assert.match(tauri, /ide_read_git_status,\s*ide_read_git_repository_inventory,/);
});

test('G06A desktop renders local-only metadata, not authorized external paths', () => {
  assert.match(ui, /invoke\('ide_read_git_repository_inventory', \{ projectId \}\)/);
  assert.match(ui, /gitInventory\.branches\.map/);
  assert.match(ui, /gitInventory\.worktrees\.map/);
  assert.match(ui, /Not registered for access; external path hidden/);
  assert.match(ui, /gitInventory\.branchesTruncated/);
  assert.match(ui, /gitInventory\.worktreesTruncated/);
  assert.doesNotMatch(ui, /item\.absolutePath|item\.externalPath|item\.worktreePath/);
});

test('G06A05 native blame path is validated, committed and bounded', () => {
  const history = read('../../host/src/git_file_history.rs');
  assert.match(history, /validate_relative\(relative_path, false\)/);
  assert.match(history, /tree\.get_path/);
  assert.match(history, /blame_file/);
  assert.match(history, /const MAX_HUNKS: usize = 96/);
  assert.match(history, /const MAX_FILE_COMMITS: usize = 40/);
  assert.doesNotMatch(history, /Command::new|repo\.checkout|repo\.fetch|repo\.push/);
  assert.match(tauri, /async fn ide_read_git_file_history\(/);
  assert.match(ui, /invoke\('ide_read_git_file_history'/);
});

test('G06A06 references and remotes expose no credential URLs', () => {
  const refs = read('../../host/src/git_refs_health.rs');
  assert.match(refs, /self\.registered_git_root\(project_id\)/);
  assert.match(refs, /repo\.references\(\)/);
  assert.match(refs, /repo\.remotes\(\)/);
  assert.doesNotMatch(refs, /\.url\(\)|\.pushurl\(\)|Command::new|repo\.fetch\(/);
  assert.match(refs, /remote_url_exposed: false/);
  assert.match(tauri, /async fn ide_read_git_refs_health\(/);
  assert.match(ui, /invoke\('ide_read_git_refs_health'/);
});
