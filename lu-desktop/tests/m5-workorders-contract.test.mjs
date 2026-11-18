import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const installed=read('../../scripts/m5-windows-installed-memory-pilot.ps1');
const native=read('../../src-tauri/lu-core/tests/m5_effect_authority_acceptance.rs');
const pilot=read('../../benchmarks/memory/m5_comparative_acceptance.py');
test('physical installed Windows gate is fail-closed and hardware sourced',()=>{
  for(const required of [
    'RealInteractiveWindowsAndOwnerConsent',
    'DedicatedDisposableTestProfileConfirmed',
    'Win32_VideoController',
    'UIAutomationClient',
    'System.Windows.Automation.AutomationElement',
    'ProcessId',
    'llama-server.exe',
    'WindowTimeoutSeconds',
    'primary-before-restart',
    'same-primary-after-restart',
    'replacement-after-model-swap',
    'OS_PRIOR_WORKERS_NOT_PRESENT',
    'reportedModelAnswer',
    'reportedSourceId',
    'OPERATOR_REPORTED_PASS',
    'INCOMPLETE_NOT_ACCEPTED',
    'NEVER_GRANT_BY_THIS_SCRIPT'
  ])assert.ok(installed.includes(required),required);
  assert.match(installed,/if\(\$env:OS -ne 'Windows_NT'/);
  assert.match(installed,/exit 2/);
  assert.doesNotMatch(installed,/Invoke-WebRequest|DownloadFile|curl\.exe/);
});
test('actual native core authority, file effect and uncertain journal protected',()=>{
  for(const needle of [
    'OriginalBookVerifier',
    'ConversationAuthority::new()',
    'open_user_task(',
    'authorize_worker_call(',
    'with_authorized_worker_effect(',
    'fs::write(&target',
    'revoke_task("task-one")',
    'revoke_conversation("chat-one")',
    'EffectState::Uncertain',
    'reconcile_interrupted()',
    'PrepareOutcome::Existing(EffectState::Uncertain)'
  ])assert.ok(native.includes(needle),needle);
});
test('comparative pilot never accepts release based on source-only data',()=>{
  assert.match(pilot,/release_accepted":False/);
  assert.match(pilot,/SUBMITTED_NOT_INDEPENDENTLY_ATTESTED/);
  assert.match(pilot,/installed_first_token_latency_ms/);
  assert.match(pilot,/autonomous/);
});
