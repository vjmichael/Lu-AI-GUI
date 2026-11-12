import test from 'node:test';
import assert from 'node:assert/strict';
import { nativeProfileAvailability, planProtectedBuildProfile,
  isCurrentProtectedBuildPlan, protectedBuildOutcome } from '../web/protected_build_profiles.js';
import { boundedBuildTestHints } from '../web/build_test_output_hints.js';

const project='ide-'+'a'.repeat(32);
const sha='b'.repeat(64);
const status=()=>({
  ready:true,cargoReady:true,current:null,quarantined:[],journal:[],
  globalRecoveryRequired:false,activeInAnotherProject:false,globalWorkerBusy:false,
  multilangReady:[{profileId:'python_pytest',ready:true,
    code:'DEV03_RUNTIME_PIN_VERIFIED_NATIVE_ADMISSION_PER_JOB'},
    {profileId:'js_tests',ready:false,code:'DEV03_RUNTIME_NOT_READY'}],
});
const catalog=()=>({projectId:project,profiles:[
  {id:'rust_tests',sha,manifest:'Cargo.toml',
    existingRecipe:'rust_tests',manifestState:'saved_file_sha_observed_not_validated'},
  {id:'python_pytest',sha,manifest:'pyproject.toml',
    existingRecipe:null,manifestState:'saved_file_sha_observed_not_validated'},
]});
test('native admission needs the pinned *recipe*, not any toolchain',()=>{
  const value=status();value.cargoReady=false;
  assert.equal(nativeProfileAvailability(value,'rust_tests'),'cargo_toolchain_not_verified');
  assert.equal(nativeProfileAvailability(value,'python_pytest'),'reviewable_native');
  assert.equal(nativeProfileAvailability(value,'js_tests'),'adapter_runtime_not_approved');
  value.quarantined=[{requiresReconciliation:true}];
  assert.equal(nativeProfileAvailability(value,'python_pytest'),
    'worker_unavailable_or_recovery_required');
});
test('reviewed plan is only fixed recipe plus saved manifest SHA',()=>{
  const plan=planProtectedBuildProfile(catalog(),status(),project,'python_pytest');
  assert.equal(plan.expectedManifestSha256,sha);
  assert.equal(plan.executionAuthorized,false);
  assert.equal(isCurrentProtectedBuildPlan(plan,catalog(),status(),project),true);
  const old=catalog();old.profiles[1].sha='c'.repeat(64);
  assert.equal(isCurrentProtectedBuildPlan(plan,old,status(),project),false);
  assert.throws(()=>planProtectedBuildProfile(catalog(),status(),'ide-'+'c'.repeat(32),
    'python_pytest'));
});
test('native result classification never promotes log or acceptance',()=>{
  const plan=planProtectedBuildProfile(catalog(),status(),project,'rust_tests');
  const original={
    projectId:project,recipeId:'rust_tests',manifestSha256:sha,
    jobId:'job-'+'1'.repeat(32),planSha256:'2'.repeat(64),
    inputSnapshotSha256:'3'.repeat(64),toolchainSha256:'4'.repeat(64),
    terminalReceiptSha256:'5'.repeat(64),sourceHead:'6'.repeat(40),
    phase:'completed',nativeExitCode:0,requiresReconciliation:false,
    timedOut:false,installedAppAccepted:false,
  };
  const result=protectedBuildOutcome(original,plan,project);
  assert.equal(result.nativeCompleted,true);
  assert.equal(result.testAssertionsVerified,false);
  assert.equal(result.installedAccepted,false);
  assert.equal(protectedBuildOutcome({...original,requiresReconciliation:true},
    plan,project).nativeCompleted,false);
  assert.throws(()=>protectedBuildOutcome({...original,
    manifestSha256:'7'.repeat(64)},plan,project));
});
test('compiler/log hints remain read-only and reject path escapes',()=>{
  const job='job-'+'1'.repeat(32);
  const output={projectId:project,jobId:job,liveUnverified:true,chunks:[
    {stream:'stderr',text:'--> src/lib.rs:17:6\n--> ../../secret.rs:2:2\nerror: test failed\n'},
  ]};
  const rows=boundedBuildTestHints(output,project,job);
  assert.equal(rows.length,2);
  assert.equal(rows[0].relativePath,'src/lib.rs');
  assert.equal(rows.every(row=>row.verified===false && row.navigable===false),true);
  assert.equal(boundedBuildTestHints({...output,jobId:'job-'+'9'.repeat(32)},
    project,job).length,0);
});

test('FIN01 foreign project global supervisor cannot be reviewed',()=>{
  const current=status();
  current.activeInAnotherProject=true;current.globalWorkerBusy=true;
  assert.equal(nativeProfileAvailability(current,'rust_tests'),
    'worker_unavailable_or_recovery_required');
  const broken=status(); delete broken.globalRecoveryRequired;
  assert.equal(nativeProfileAvailability(broken,'rust_tests'),
    'worker_unavailable_or_recovery_required');
});
