import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const body=readFileSync(new URL('../../scripts/m4-windows-installed-memory-acceptance.ps1',
  import.meta.url),'utf8');
test('M4 installed model-swap acceptance cannot pass from one model launch',()=>{
 for(const marker of [
   'PhysicalInteractiveWindowsConfirmed','ManualMemoryTrialConfirmed',
   'Review $InstalledExe','PrimaryGGUF','ReplacementGGUF',
   'Windows.Automation.AutomationElement','firstUiProcessIdentity',
   'replacementWorkerObserved','replacementWorkerReaped',
   'workerReapedAfterClose','Close-App $second','Get-CimInstance',
   'OPERATOR_REPORTED_PASS','INCOMPLETE_NOT_ACCEPTED']){
   assert.ok(body.includes(marker),marker);
 }
 assert.match(body,/replacementWorkerReaped -eq 'PASS_OS_REAP_OBSERVED'/);
 assert.match(body,/installedAcceptance -ne/);
 assert.doesNotMatch(body,/Invoke-WebRequest|curl\.exe|DownloadFile/);
});
