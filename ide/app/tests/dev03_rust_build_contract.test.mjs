import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

function source(path) {
  return readFileSync(new URL(path, import.meta.url),'utf8');
}
const supervisor=()=>source('../../supervisor/src/lib.rs');
const snapshots=()=>source('../../supervisor/src/snapshot.rs');
const linux=()=>source('../../supervisor/src/linux_worker.rs');
const windows=()=>source('../../supervisor/src/windows_worker.rs');
const desktop=()=>source('../src-tauri/src/build_backend.rs');
const intents=()=>source('../../host/src/execution_intents.rs');

test('new Cargo build recipe is a fixed exact plan, never arbitrary command',()=>{
  const host=supervisor();
  assert.match(host,/"rust_check" \| "rust_build" \| "rust_tests" \| "lu_ide_host_tests"/);
  assert.match(host,/value\.effect_class == "test_build_isolated"/);
  assert.match(host,/value\.memory_limit_bytes <= MAX_MEMORY_BYTES/);
  assert.match(host,/value\.runtime_limit_ms <= MAX_RUNTIME_MS/);
  assert.match(host,/build\.recipe_id = "cargo build --release"/);
  assert.match(host,/for recipe in \["python_pytest", "js_tests", "cpp_cmake_build"\]/);
  assert.match(host,/validate_plan\(&build\), cfg!\(feature = "multilang-worker"\)/);
  assert.match(host,/build\.recipe_id = "sh -c echo bad"/);
});

test('new build freezes project source via same sealed manifest snapshot',()=>{
  const snap=snapshots();
  assert.match(snap,/"rust_check" \| "rust_build" \| "rust_tests" => "Cargo.toml"/);
  assert.match(snap,/publish_input\(&project, &stage, "rust_build"\)/);
  assert.match(snap,/verify_input\(&frozen\)/);
  assert.match(snap,/retire_input\(frozen\)/);
  assert.match(snap,/publish_input\(&project, &stage, "python_pytest"\)/);
});

test('Linux executes fixed cargo build inside original cgroup/Bubblewrap route',()=>{
  const code=linux();
  assert.match(code,/"rust_tests" \| "rust_check" \| "rust_build" => "\/workspace\/Cargo.toml"/);
  assert.match(code,/"rust_check" \| "rust_build" \| "rust_tests" \| "lu_ide_host_tests" if runtime\.is_none\(\)/);
  assert.match(code,/"rust_build" => "build"/);
  assert.match(code,/cmd\.arg\(CARGO\)/);
  assert.match(code,/Command::new\(BWRAP\)/);
  assert.match(code,/--unshare-all/);
  assert.match(code,/--ro-bind/);
  assert.match(code,/--offline/);
  assert.match(code,/attach_pid_in_pre_exec/);
  assert.match(code,/--manifest-path/);
});

test('Windows runs build only through fixed AppContainer/Job Cargo command',()=>{
  const code=windows();
  assert.match(code,/"rust_build" => \("build", "Cargo.toml"\)/);
  assert.match(code,/tools\.join\("bin\/cargo.exe"\)/);
  assert.match(code,/--offline --locked --manifest-path/);
  assert.match(code,/fn isolated_cargo_stdio\(/);
  assert.match(code,/Job|AppContainer/);
});

test('desktop Build still requires clean Git HEAD, frozen snapshot, one human approval',()=>{
  const code=desktop();
  assert.match(code,/const VALID_RECIPES: \[&str; 4\]/);
  assert.match(code,/if !VALID_RECIPES\.contains\(&recipe_id\)/);
  assert.match(code,/registry\.trusted_native_worker_root\(project_id\)/);
  assert.match(code,/inspection\.worktree != "clean"/);
  assert.match(code,/publish_input\(&source_root, &self\.staged, recipe_id\)/);
  assert.match(code,/verify_input\(&input\)/);
  assert.match(code,/app\.dialog\(\)/);
  assert.match(code,/record_human_approval/);
  assert.match(code,/execute_windows_worker_after_staging_approval/);
  assert.match(code,/execute_linux_worker/);
  assert.doesNotMatch(code,/Command::new\(/);
});

test('intent review remains non-executing and manifest-fixed',()=>{
  const code=intents();
  assert.match(code,/static RECIPES: \[Recipe; 4\]/);
  assert.match(code,/id: "rust_build"[\s\S]*?manifest: "Cargo.toml"/);
  assert.match(code,/cargo build --locked --offline --manifest-path Cargo.toml/);
  assert.match(code,/fn safe_manifest\(/);
  assert.match(code,/IntentError::SourceDirty/);
  assert.match(code,/recipe\("npm test"\)\.is_none\(\)/);
});

test('frontend selectable build IDs never include npm, pytest, CMake or shell',()=>{
  const ui=source('../web/main.jsx');
  const profiles=source('../web/project_run_profiles.js');
  const schema=JSON.parse(source('../../host/src/capability_schemas.json'));
  assert.match(ui,/value="rust_build"/);
  assert.match(ui,/\['rust_check', 'rust_build', 'rust_tests', 'lu_ide_host_tests'\]/);
  assert.match(profiles,/\["rust_build", "rust", "build", "Cargo.toml", "rust_build"\]/);
  assert.equal(schema.definitions.ProjectRunProfileDiscovery.properties.profiles.maxItems,7);
  assert.ok(schema.definitions.ProjectRunProfile.properties.profileId.enum.includes('rust_build'));
  assert.ok(schema.definitions.ProjectRunProfile.properties.existingNativeRecipeId.anyOf[0]
    .enum.includes('rust_build'));
  assert.ok(schema.operations['job.prepare'].request.properties.recipeId.enum
    .includes('rust_build'));
  assert.match(ui,/invoke\('ide_build_start',/);
  const portion=ui.slice(ui.indexOf('function chooseProjectRunProfile'),
    ui.indexOf('async function refreshProtectedBuildStatus'));
  assert.doesNotMatch(portion,/ide_build_start|spawn\(|eval\(/);
});
