import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  safeProjectRunProfileDiscovery,selectExistingProtectedRecipe,
} from '../web/project_run_profiles.js';
const project='ide-'+'a'.repeat(32),foreign='ide-'+'b'.repeat(32);
const SHA='a'.repeat(64);
const expected=[
  ['rust_check','rust','check','Cargo.toml','rust_check'],
  ['rust_tests','rust','test','Cargo.toml','rust_tests'],
  ['python_pytest','python','test','pyproject.toml',null],
  ['js_tests','javascript','test','package.json',null],
  ['cpp_cmake_build','cpp','build','CMakeLists.txt',null],
  ['lu_ide_host_tests','rust','test','ide/host/Cargo.toml','lu_ide_host_tests'],
];
function fixture() {
  return {schemaVersion:1,projectId:project,
    source:'fixed_host_manifest_and_conventional_tests_inspection',
    manifestContentsExported:false,processStarted:false,
    workerGrantIssued:false,taskAccepted:false,
    profiles:expected.map(([profileId,language,operation,manifestRelativePath,
      existingNativeRecipeId])=>({
      profileId,title:'Fixed '+profileId,language,operation,manifestRelativePath,
      manifestState:'saved_file_sha_observed_not_validated',manifestSha256:SHA,
      executionRoute:existingNativeRecipeId
        ? 'existing_wo6_protected_cargo_requires_fresh_authorization'
        : 'not_implemented_requires_reviewed_native_adapter',
      existingNativeRecipeId,executionAuthorized:false,testCandidates:1,
      testHintPaths:['tests/test_demo.py'],
      testScanState:'bounded_conventional_tests_only_not_discovery_complete',
    })),
  };
}

test('six fixed manifest profiles and exactly three existing protected Cargo recipes',()=>{
  const safe=safeProjectRunProfileDiscovery(fixture(),project);
  assert.equal(safe.profiles.length,6);
  assert.equal(safe.executionAuthorized,false);
  assert.equal(safe.profiles.filter(p=>p.existingRecipe).length,3);
  assert.equal(selectExistingProtectedRecipe(safe,'rust_check',project),'rust_check');
  assert.equal(selectExistingProtectedRecipe(safe,'rust_tests',project),'rust_tests');
  assert.equal(selectExistingProtectedRecipe(safe,'lu_ide_host_tests',project),
    'lu_ide_host_tests');
});

test('Python, JS and CMake have no runnable adapter even when manifests exist',()=>{
  const safe=safeProjectRunProfileDiscovery(fixture(),project);
  for (const id of ['python_pytest','js_tests','cpp_cmake_build']) {
    assert.throws(()=>selectExistingProtectedRecipe(safe,id,project),
      /DEV03_EXECUTION_ADAPTER_UNAVAILABLE/);
  }
  assert.throws(()=>selectExistingProtectedRecipe(safe,'node -e malicious',project),
    /DEV03_EXECUTION_ADAPTER_UNAVAILABLE/);
});

test('cannot select missing or unreadable Cargo manifest',()=>{
  for (const state of ['not_found','not_readable_or_unverifiable']) {
    const value=fixture();
    value.profiles[0].manifestState=state;
    value.profiles[0].manifestSha256=null;
    const safe=safeProjectRunProfileDiscovery(value,project);
    assert.throws(()=>selectExistingProtectedRecipe(safe,'rust_check',project),
      /DEV03_EXECUTION_ADAPTER_UNAVAILABLE/);
  }
});

test('reject stale or foreign project even when a recipe matches',()=>{
  const safe=safeProjectRunProfileDiscovery(fixture(),project);
  assert.throws(()=>selectExistingProtectedRecipe(safe,'rust_check',foreign),
    /DEV03_PROJECT_STALE/);
  assert.throws(()=>safeProjectRunProfileDiscovery(fixture(),foreign),
    /DEV03_DISCOVERY_SHAPE_DENIED/);
});

test('reject any backend claim of authority or executed project code',()=>{
  for (const key of ['processStarted','workerGrantIssued','taskAccepted',
    'manifestContentsExported']) {
    const data=fixture();
    data[key]=true;
    assert.throws(()=>safeProjectRunProfileDiscovery(data,project),
      /DEV03_DISCOVERY_SHAPE_DENIED/);
  }
  const data=fixture();
  data.profiles[0].executionAuthorized=true;
  assert.throws(()=>safeProjectRunProfileDiscovery(data,project),
    /DEV03_PROFILE_UNTRUSTED/);
});

test('reject added, duplicate, spoofed or arbitrary command profiles',()=>{
  const data=fixture();
  data.profiles[0].profileId='sh -c whoami';
  assert.throws(()=>safeProjectRunProfileDiscovery(data,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const duplicate=fixture();
  duplicate.profiles[0]=duplicate.profiles[1];
  assert.throws(()=>safeProjectRunProfileDiscovery(duplicate,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const changed=fixture();
  changed.profiles[2].existingNativeRecipeId='python -m pytest';
  assert.throws(()=>safeProjectRunProfileDiscovery(changed,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const forge=fixture();
  forge.profiles[3].manifestRelativePath='../outside/package.json';
  assert.throws(()=>safeProjectRunProfileDiscovery(forge,project),
    /DEV03_PROFILE_UNTRUSTED/);
});

test('reject fabricated SHA, inconsistency, oversized hints and unsafe paths',()=>{
  const invalid=fixture();
  invalid.profiles[0].manifestSha256='invalid';
  assert.throws(()=>safeProjectRunProfileDiscovery(invalid,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const inconsistent=fixture();
  inconsistent.profiles[0].manifestState='not_found';
  assert.throws(()=>safeProjectRunProfileDiscovery(inconsistent,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const oversized=fixture();
  oversized.profiles[1].testHintPaths=Array(21).fill('tests/a.rs');
  assert.throws(()=>safeProjectRunProfileDiscovery(oversized,project),
    /DEV03_PROFILE_UNTRUSTED/);
  const escaped=fixture();
  escaped.profiles[3].testHintPaths=['tests/../secret.js'];
  assert.throws(()=>safeProjectRunProfileDiscovery(escaped,project),
    /DEV03_PROFILE_UNTRUSTED/);
});

test('safe Unicode test filenames remain displayable without path traversal',()=>{
  const data=fixture();
  data.profiles[2].testHintPaths=['tests/test_épreuve.py'];
  const safe=safeProjectRunProfileDiscovery(data,project);
  assert.deepEqual(safe.profiles[2].hints,['tests/test_épreuve.py']);
});
test('native source reads fixed known manifests, no untrusted path or executable',()=>{
  const host=readFileSync(new URL('../../host/src/project_run_profiles.rs',
    import.meta.url),'utf8');
  const tauri=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const catalog=readFileSync(new URL('../../host/src/capability_schemas.json',
    import.meta.url),'utf8');
  const meta=readFileSync(new URL('../../host/src/capabilities.rs',
    import.meta.url),'utf8');
  assert.match(host,/const SPECS: \[Spec; MAX_PROFILE_COUNT\]/);
  assert.match(host,/registry\.preview_source_file\(project_id, relative_path\)/);
  assert.match(host,/registry\.list_source_directory\(project_id, "tests"\)/);
  assert.match(host,/pub execution_authorized: bool/);
  assert.match(tauri,/async fn ide_discover_project_run_profiles\(/);
  assert.match(tauri,/ide_discover_project_run_profiles,/);
  assert.match(meta,/"job.discoverProfiles"/);
  const schema=JSON.parse(catalog);
  assert.deepEqual(schema.operations['job.discoverProfiles'].request.required,
    ['projectId']);
  assert.equal(schema.operations['job.discoverProfiles'].result.$ref,
    '#/$defs/ProjectRunProfileDiscovery');
  assert.equal(schema.definitions.ProjectRunProfileDiscovery
    .properties.processStarted.const,false);
  assert.equal(schema.definitions.ProjectRunProfile.properties
    .executionAuthorized.const,false);
  const start=ui.indexOf('async function inspectProjectRunProfiles()');
  const end=ui.indexOf('async function refreshProtectedBuildStatus()',start);
  const observed=ui.slice(start,end);
  assert.ok(start>=0&&end>start);
  assert.match(observed,/invoke\('ide_discover_project_run_profiles',\{projectId\}\)/);
  assert.doesNotMatch(observed,/ide_build_start|ide_prepare_execution_intent|eval\(|spawn\(/);
  assert.match(ui,/aria-label="DEV-03 project build and test profiles"/);
  assert.match(ui,/Select existing protected Cargo recipe/);
});
