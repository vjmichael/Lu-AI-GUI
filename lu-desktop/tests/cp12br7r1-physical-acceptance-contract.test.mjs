import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const script=readFileSync(new URL('../../scripts/cp12br7r1-windows-installed-acceptance.ps1',import.meta.url),'utf8');
test('CP-12B-R7-R1 Windows acceptance runner enforces exact native installation and test consent',()=>{
 for(const x of ['DisposableTestProfileConfirmed','ExpectedExeSha256','ExpectedModelSha256','Get-FileHash','Check-PE','Windows_NT','UserInteractive','two_independent_lu_processes','two_native_worker_processes'])assert.ok(script.includes(x),x);
 assert.match(script,/if\(\$exeHash -ne \$ExpectedExeSha256/);
 assert.match(script,/if\(\$modelHash -ne \$ExpectedModelSha256/);
 assert.match(script,/\$subsystem -ne 2/);
});
test('R7-R1 requires both consent dialogs, recovery, rejection, replay, separate-account checks',()=>{
 for(const x of ['sender_consent_refusal','receiver_consent_refusal','two_model_verified_stage','sender_review_cancellation','receiver_review_cancellation','revocation_and_replay','consumed_invitation_replay','sender_crash_stale_invitation','receiver_crash_stale_invitation','other_account_real_process_denial'])assert.ok(script.includes(x),x);
 assert.match(script,/\$confirm -ceq 'CRASH'/);
 assert.match(script,/Stop-Process -Id \$[ab]\.Id -Force/);
});
test('R7-R1 keeps manual observations non-authoritative and avoids raw credential/invitation logs',()=>{
 assert.match(script,/conclusion='NOT_ACCEPTED'/);
 assert.match(script,/USER_REPORTED_ONLY/);
 assert.match(script,/Not independently automated pass|not an independently automated pass/);
 assert.doesNotMatch(script,/Get-Clipboard|Set-Clipboard|Get-Credential|ConvertFrom-SecureString|\.CommandLine\s*\}/);
 assert.match(script,/Never save[\s\S]*invitation codes/);
});
