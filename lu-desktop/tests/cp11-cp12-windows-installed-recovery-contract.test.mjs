import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL('../../'+p,import.meta.url),'utf8');
const wf=read('.github/workflows/lu-cp11-cp12-windows-installed-recovery.yml');
const browserWf=read('.github/workflows/lu-core-c62b-reviewed-browser-nsis.yml');
const nativeChecks=read('lu-desktop/src-tauri/src/active_checks.rs');
const runner=read('scripts/cp11-cp12-windows-installed-recovery.ps1');
const journal=read('scripts/cp11-cp12-windows-readonly-journal.py');
test('CP-11/12 is one source-locked Windows cohort and rebuilds genuine fixed binary through original reviewed-browser authority',()=>{
  // Git preserves source blobs, but Windows checkout may present CRLF to Node.
  // Normalize ONLY the inspected workflow text; never modify checked-out bytes.
  const eventsFrom=(source)=>{
    const sections=source.replace(/\r\n/g,'\n').split('\non:\n');
    assert.equal(sections.length,2,'Original workflow event declaration required');
    const eventParts=sections[1].split('\npermissions:');
    assert.equal(eventParts.length,2,'Original workflow permissions boundary required');
    return eventParts[0];
  };
  const events=eventsFrom(wf);
  assert.equal(eventsFrom(wf.replace(/\r\n/g,'\n').replace(/\n/g,'\r\n')),events,'CRLF checkout must preserve exact trigger admission');
  assert.match(events,/push:\n    branches:\n      - lu\/cp11-cp12-installed-recovery-20261121\n    paths:\n      - source-tree\.lock\.json/);
  assert.doesNotMatch(events,/pull_request:|pull_request_target:|schedule:/);
  assert.match(wf,/cancel-in-progress: false/);
  assert.match(wf,/ref: \$\{\{ github\.sha \}\}/);
  assert.match(wf,/node scripts\/verify-source-tree\.mjs/);
  assert.match(wf,/uses: \.\/\.github\/workflows\/lu-core-c62b-reviewed-browser-nsis\.yml/);
  assert.match(wf,/needs: qualify-current-reviewed-browser/);
  assert.match(wf,/name: Lu-C62B-Reviewed-Browser-NSIS-\$\{\{ github\.sha \}\}/);
  assert.match(wf,/\$expectedSource=\$env:GITHUB_SHA/);
  assert.match(wf,/Exact current rebuilt native NSIS provenance/);
  assert.doesNotMatch(wf,/run-id: 44979001346|Lu-C62B-Reviewed-Browser-NSIS-2ded370fb7da34498e24376b3e7c0abe4771f96d|continue-on-error: true/);
  // A code correction cannot be certified by an immutable, pre-fix executable.
  // The called original C6.2B workflow must still execute the genuine approved
  // 482-file Chromium/Node/Playwright bundle and native source build.
  assert.match(browserWf,/workflow_call:/);
  assert.match(browserWf,/reviewed=68f7aa46c4cd2d6fdb0e951d8da5f057e1cafdd9/);
  assert.match(browserWf,/run-id: 44976683630/);
  assert.match(browserWf,/f2948f9ef9d2b5bbac8a60e146bd4a18a0714a615883faaa0139b8d5828e5cf3/);
  assert.match(browserWf,/current-windows-unapproved-browser-candidate:/);
  assert.match(browserWf,/native-windows-installed-reviewed-browser:/);
  assert.match(browserWf,/build-current-windows-nsis\.ps1 -SourceSha \$env:SOURCE_SHA/);
  assert.match(browserWf,/Native Rust host independently recomputes the exact compiled reviewed-browser manifest AND all files/);
  assert.match(browserWf,/Genuine hosted Windows install \/ browser bytes \/ unified tabs \/ uninstall \/ AppData preservation/);
  assert.match(browserWf,/Lu-C62B-Reviewed-Browser-NSIS-\$\{\{ github\.sha \}\}/);
  assert.doesNotMatch(browserWf,/continue-on-error: true/);
});
test('CP-11A Windows product fix closes only the owned directory handle before empty-dir removal',()=>{
  assert.match(nativeChecks,/struct Scratch \{ path:PathBuf, dir:Option<cap_std::fs::Dir>, cleaned:bool \}/);
  assert.match(nativeChecks,/match dir\.remove_file\(name\)/);
  assert.match(nativeChecks,/drop\(self\.dir\.take\(\)\);[\s\S]*?std::fs::remove_dir\(&self\.path\)/);
  assert.doesNotMatch(nativeChecks,/remove_dir_all\(&self\.path\)|remove_file\(&self\.path\)/);
  for(const name of [
    'cp11a_windows_exclusive_scratch_cleanup_releases_owned_directory_handle',
    'cp11a_windows_cleanup_preserves_preexisting_scratch_slot'
  ]){
    assert.match(nativeChecks,new RegExp('#\\[cfg\\(windows\\)\\][\\s\\S]{0,70}'+name));
    assert.ok(browserWf.includes('cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib active_checks::tests::'+name),
      'real Windows native test before installed admission: '+name);
  }
  assert.match(nativeChecks,/assert_eq!\(std::fs::read\(old.join\("history.sqlite"\)\)\.unwrap\(\),b"pre-existing unrelated bytes"\)/);
});
test('genuine hosted installed GUI requires original executable, native UIA consent and exact cold restart',()=>{
  for(const phrase of [
    'DisposableWindowsConfirmed','InstallerSha256','AppSha256',
    'Get-FileHash','Start-Process','MainWindowHandle',
    'EnumChildWindows','IsChild','GetRuntimeId','Native-Review',
    'Lu — Check Lu','Run checks','Cancel','Lu — Approve reminder',
    'Schedule reminder','Decline','Stop-Lu $true','Wait-Check',
    'cold_check_recovery_exact_prefix_no_replay','closed_application_did_not_fire_reminder',
    'missed_reminder_single_fire_on_cold_restart','second_restart_durable_exactly_once_notice',
    'plain_diagnostic_status_after_cold_recovery'
  ])assert.ok(runner.includes(phrase),phrase);
  assert.match(runner,/if\(Test-Path -LiteralPath \$profile\)\{throw 'Refuse existing user profile/);
  assert.match(runner,/\$script:window=\[System.Windows.Automation.AutomationElement\]::FromHandle/);
  assert.match(runner,/Current\.ProcessId/);
  assert.match(runner,/\$before\.state -eq 'running'/);
  assert.match(runner,/\$recovered\.prefixHash -ceq \$before\.prefixHash/);
  assert.match(runner,/\$due\.firedCount -eq 1 -and \$due\.unreadCount -eq 1/);
  assert.match(runner,/if\(\$report\.result -ne 'PASS_SCOPED_HOSTED_WINDOWS_CP11A_CP11B_CP12A'\)\{exit 2\}/);
});
test('original native Windows TaskDialog pane is exact owner-controlled consent, not a generic pane',()=>{
  const start=runner.indexOf('function Test-NativeChoiceIdentity(');
  const end=runner.indexOf('function Start-Lu(',start);
  assert.ok(start>=0&&end>start,'narrow TaskDialog identity helper is required');
  const section=runner.slice(start,end).replace(/\r\n/g,'\n');
  for(const s of [
    "'Run checks'{'CommandButton_1000'}",
    "'Schedule reminder'{'CommandButton_1000'}",
    "'Cancel'{'CommandButton_1001'}",
    "'Decline'{'CommandButton_1001'}",
    "$current.ControlType -eq [System.Windows.Automation.ControlType]::Pane",
    "$current.ClassName -ceq 'CCPushButton'",
    '$current.AutomationId -ceq $expectedId',
    '$current.Name -cne $choice',
    '$current.ProcessId -ne $ownerPid',
    '[LuCpRecoveryWin32]::IsChild($ownerHwnd',
    '-not $current.IsEnabled',
    '$current.IsOffscreen',
    '$matches.Count -gt 1',
    'Test-NativeChoiceIdentity $chosen.Current $choice'
  ])assert.ok(section.includes(s),s);
  assert.match(section,/if\(-not \$chosen\)\{throw \('Original native approval choice unavailable:/);
  assert.doesNotMatch(section,/SendKeys|PostMessage|SendMessage|Window\.Confirm|ControlType\.Pane\)\{return \$true\}/);
});
test('native approval is not inferred from a clicked label and check failure keeps actual terminal evidence',()=>{
  const modal=runner.slice(runner.indexOf('function Native-Review('),runner.indexOf('function Start-Lu('));
  const check=runner.slice(runner.indexOf('function Wait-Check('),runner.indexOf('try{\n  if(-not [string]::IsNullOrEmpty($priorArgs))',runner.indexOf('function Wait-Check(')));
  for(const exact of [
    '[LuCpRecoveryWin32]::GetForegroundWindow()',
    '[LuCpRecoveryWin32]::SetForegroundWindow($ownerHwnd)',
    'Native approval control identity changed after foreground admission',
    'Click-Control $chosen $dlg',
    '[LuCpRecoveryWin32]::IsWindow($ownerHwnd)',
    'Native approval click did not dismiss the original dialog: '
  ])assert.ok(modal.includes(exact),exact);
  assert.ok(modal.indexOf('Click-Control $chosen $dlg') < modal.indexOf('[LuCpRecoveryWin32]::IsWindow($ownerHwnd)'),
    'modal dismissal must be checked after real input');
  for(const exact of [
    "Read-Journal 'check'",
    "$report['lastCheck']=[ordered]@{",
    'reason=$(if($s.present)',
    'kinds=$(if($s.present)',
    'verified=$(if($s.present)',
    "('attention','stopped','interrupted','complete')",
    'Real installed check reached unexpected terminal state',
    'original receipt preserved'
  ])assert.ok(check.includes(exact),exact);
  assert.doesNotMatch(check,/UPDATE active_check|INSERT INTO active_check|rawPrompt|conversationText|modelOutput|Set-Clock/);
});
test('read-only private journal oracle cannot seed statuses or view unrelated content',()=>{
  for(const p of ['mode=ro','PRAGMA query_only=ON','active_check','lu_user_reminders','dx_trace','CP11-WINDOWS-TEST-','prefixHash'])assert.ok(journal.includes(p),p);
  assert.doesNotMatch(journal,/\b(?:UPDATE|INSERT|DELETE|REPLACE|DROP|ALTER)\s+(?:active_check|lu_user_reminders|dx_trace)\b/i);
  assert.doesNotMatch(journal,/exec\(|subprocess\.|shell=True|mode=rw|mode=rwc/);
  assert.match(journal,/SELECT id,workspace_id,state,next_due,repeat_seconds,last_due,fired_count,unread_count/);
  assert.match(journal,/SELECT run_id,state,body FROM active_check WHERE slot=1/);
});
test('positive assertion is limited to the model-free and read-only installed scope',()=>{
  assert.match(wf,/Rebuilt exact current C6\.2B-security-qualified NSIS checksum mismatch/);
  assert.match(wf,/Real installed CP-11\/12 GUI\/restart or SQLite readback failed/);
  assert.match(wf,/model-free CP-11A\/11B and local CP-12A only/);
  assert.match(wf,/Two-model Windows DX, scheduled AI\/GGUF, owner physical R9700 and release admission remain separate/);
});
