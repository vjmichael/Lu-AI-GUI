import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const host = read('../../host/src/package_manager.rs');
const tauri = read('../src-tauri/src/lib.rs');
const ui = read('../web/main.jsx');

test('IDE05 fixed native probe uses only registered project and existing read-only SHA capability',()=>{
  assert.match(host,/const FIXED_FILES: \[&str; 6\]/);
  for(const path of ['pyproject.toml','uv.lock','vcpkg.json',
                     'conanfile.py','conanfile.txt','conan.lock'])
    assert.ok(host.includes('"'+path+'"'));
  assert.match(host,/self\.source_capability\(project_id\)/);
  assert.match(host,/self\.probe_project_resource\(project_id, path\)/);
  assert.doesNotMatch(host, /std::process::Command|Command::new|std::net::|ureq::|reqwest::/);
  assert.match(host,/network_accessed: false, process_started: false/);
  assert.match(host,/files_modified: false, resolution_supported: false/);
  assert.match(host,/apply_supported: false, execution_authorized: false/);
});
test('Tauri command accepts only an opaque project id and never an install plan',()=>{
  const command=tauri.match(/async fn ide_inspect_package_managers\([\s\S]*?\n\}/)?.[0];
  assert.ok(command,'native read-only command must exist');
  assert.match(command,/project_id: String/);
  assert.doesNotMatch(command,/manager_id: String|package_name: String|command: String|path: String|grant: String|token: String/);
  assert.match(command,/registry\.inspect_package_managers\(&project_id\)/);
  assert.match(tauri,/ide_discover_project_run_profiles,\s*ide_inspect_package_managers,/);
});
test('UI cannot claim successful package resolution or install',()=>{
  assert.match(ui,/invoke\('ide_inspect_package_managers',\{projectId\}\)/);
  assert.match(ui,/result\.applySupported!==false/);
  assert.match(ui,/result\.executionAuthorized!==false/);
  assert.match(ui,/candidate\.executableManifest/);
  assert.match(ui,/Conan Python recipes are untrusted executable code/);
  assert.match(ui,/no package tool is run/i);
});
test('unverifiable and competing managers remain blockers, not approvals',()=>{
  assert.match(host,/cpp_conflicting_package_managers/);
  assert.match(host,/uv_orphan_lockfile/);
  assert.match(host,/conan_multiple_recipe_manifests/);
  assert.match(host,/unverifiable_manifest_or_lockfile/);
  assert.match(host,/per_file_sha_not_atomic_resolve_requires_reinspection/);
  assert.match(host,/fn symlinked_lockfile_is_unverifiable_and_not_selected/);
});
