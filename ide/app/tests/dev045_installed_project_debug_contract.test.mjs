import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
test('project debug is opt-in, native-only and always binds a sealed build',()=>{
  const cargo=read('../src-tauri/Cargo.toml');
  const host=read('../src-tauri/src/lib.rs');
  const native=read('../src-tauri/src/debug_backend.rs');
  assert.match(cargo,/native-project-debug = \["native-debug", "native-build"\]/);
  const start=host.indexOf('async fn ide_debug_run_project(');
  const end=host.indexOf('async fn ide_debug_cancel(',start);
  assert.ok(start>=0&&end>start);
  const fragment=host.slice(start,end);
  assert.match(fragment,/feature="native-project-debug"/);
  assert.match(fragment,/native_project_debug_target\(&project_id\)/);
  assert.match(fragment,/debug\.run_project\(&app,&registry,&project_id,target\)/);
  assert.doesNotMatch(fragment,/target_path:|program: String|command: String|shell:|std::process::Command/);
  assert.match(native,/DEBUG_KERNEL_RECOVERY_REQUIRED_BEFORE_ANY_NEW_RUN/);
  assert.match(native,/DEV044_PRIVATE_TARGET_CHANGED_DURING_APPROVAL/);
  assert.match(native,/record_human_approval/);
});
test('installed Windows UI drives exact protected build and project DAP with native dialogs',()=>{
  const script=read('./../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const start=script.indexOf("if ($Scenario -eq 'Dev045ProjectDebug')");
  const end=script.indexOf("if ($Scenario -eq 'Dev045ProjectRestartDenied')",start);
  assert.ok(start>0&&end>start);
  const section=script.slice(start,end);
  for(const required of [
    'VerifiedInstalledService','{HOME}{DOWN}{DOWN}{ENTER}',
    "Click 'Request protected Run / Test'",
    "Confirm-NativeOneUseDialog 'Allow one protected build/test job' 'Run this exact job'",
    "Require 'Native exit: 0'","Require 'Reconciliation: not indicated'",
    "Require 'DEV-04.2'","Require 'DEV-04.3'",
    "Click 'Debug last verified project build'",
    "Confirm-NativeOneUseDialog 'Approve one protected project debugger' 'Debug this exact project build'",
    "Find-Control 'Protected project LLDB session'",
    "Require 'Exact build terminal receipt:'",
    "Require 'Project debugger D3 target binding:'",
    "Require 'Thread inspection: observed'","Require 'Stack: observed'",
    "Require 'Kernel tree cleanup: reported by protected supervisor'",
    "Wait-WorkerReturnToBaseline"
  ])assert.ok(section.includes(required),required);
  assert.doesNotMatch(section,/Invoke-RestMethod|Invoke-WebRequest|WriteAllBytes|Start-Process.*lldb|--attach/);
});
test('restart must deny replayed project target and is independent of historical journal',()=>{
  const script=read('./../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const section=script.slice(script.indexOf("if ($Scenario -eq 'Dev045ProjectRestartDenied')"),
    script.indexOf("if ($Scenario -eq 'ProtectedBuildDebug')"));
  assert.match(section,/\$debug\.Current\.IsEnabled/);
  assert.match(section,/stale_project_binary_or_grant_reauthorized_after_restart/);
  assert.match(section,/Durable debugger jobs \(survive restart\)/);
  assert.match(section,/Journal entries are not simulated debugger results/);
});
test('only a signed exact-head candidate with explicit pinned project-DAP asset manifest can enter installed UI gate',()=>{
  const workflow=read('../../../.github/workflows/ide-dev04-5-installed-project-debug-signed.yml');
  assert.match(workflow,/workflow_dispatch:/);
  assert.doesNotMatch(workflow,/(?:^|\n)  pull_request:|(?:^|\n)  push:/);
  assert.match(workflow,/environment: lu-ide-native-e2e/);
  assert.match(workflow,/verify_wo7_candidate\.py/);
  assert.match(workflow,/nativeProjectDebugger/);
  assert.match(workflow,/featureName -cne 'native-project-debug'/);
  assert.match(workflow,/adapterBundleSha256/);
  assert.match(workflow,/Get-FileHash -Algorithm SHA256/);
  assert.match(workflow,/Scenario Dev045ProjectDebug/);
  assert.match(workflow,/Scenario Dev045ProjectRestartDenied/);
  assert.match(workflow,/uninstall\.exe|\*uninstall\*\.exe/);
  assert.match(workflow,/DEV045_WINDOWS_CANCEL_AND_CRASH_INJECTION=NOT_TESTED_BY_THIS_RUN/);
});
test('Linux acceptance remains real delegated cgroup build→private target→DAP, not installed release',()=>{
  const native=read('../../supervisor/examples/wo6_linux_native_build_acceptance.rs');
  const workflow=read('../../../.github/workflows/ide-wo6-native-os-acceptance.yml');
  assert.match(native,/--project-debug/);
  assert.match(native,/seal_project_debug_target/);
  assert.match(native,/start_linux_project_dap/);
  assert.match(native,/target\.verify\(\)/);
  assert.match(native,/DapEvent::StackTraceReceived/);
  assert.match(native,/JobPhase::Completed/);
  assert.match(workflow,/--project-debug/);
  assert.match(workflow,/systemd-run/);
});
