import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const root=p=>readFileSync(new URL('../../'+p,import.meta.url),'utf8');
const smoke=root('scripts/m3-windows-installed-acceptance.ps1');
const cycle=root('scripts/m3-windows-nsis-cycle.ps1');
test('real installed app requires independent hashes and native UIA process identity',()=>{
  for(const marker of ['PhysicalInteractiveWindowsConfirmed',
      'VerifiedLocalFile','AssistantSha256','EmbeddingSha256',
      'Windows.Automation.AutomationElement',
      'Current.ProcessId','History library','Get-EngineProcesses',
      'WaitForExit(30000)','OPERATOR_REPORTED_PASS',
      'EVIDENCE_READY_FOR_INDEPENDENT_REVIEW_NOT_AUTO_APPROVED']){
    assert.ok(smoke.includes(marker),marker);
  }
  assert.doesNotMatch(smoke,/acceptance='PASS'/);
  assert.doesNotMatch(smoke,/Invoke-WebRequest|DownloadFile|curl.exe/i);
});
test('NSIS actual install and uninstall cannot pass by artifact hash alone',()=>{
  for(const marker of ['DisposableWindowsConfirmed',
      'InstallerSha256','ExpectedLuSha256','ExpectedEngineSha256',
      'Start-Process','WaitForExit','/D=','UninstallerName',
      'PASS_BINARIES_ABSENT','installationStateUncertain']){
    assert.ok(cycle.includes(marker),marker);
  }
  assert.ok(cycle.includes('webViewUiAndModelInference='));
  assert.doesNotMatch(cycle,/webViewUiAndModelInference='PASS'/);
  assert.doesNotMatch(cycle,/Invoke-WebRequest|curl.exe/i);
});
