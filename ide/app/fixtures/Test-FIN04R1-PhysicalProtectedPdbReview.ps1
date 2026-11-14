# FIN-04R-1: physical Windows human + protected PE/PDB review fixture.
# It NEVER clicks dialogs, creates a debugger control grant or relaunches an
# interrupted action. A passing receipt is an acceptance INPUT only: it does
# not certify a production step/breakpoint release gate.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$ProfileDirectory,
  [Parameter(Mandatory=$true)][string]$ExpectedDesktopExecutable,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')]
  [string]$ExpectedDesktopSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{40}$')]
  [string]$ExpectedSourceHead,
  [Parameter(Mandatory=$true)][string]$InstalledOwnerBundle,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
  [Parameter(Mandatory=$true)]
  [ValidateSet('ApproveReviewStep','ApproveReviewBreakpoint',
    'DeclineReviewStep','CancelWhileDialog','TimeoutReview','RestartNoReplay')]
  [string]$Scenario,
  [switch]$RequireRestart
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason) {throw "FIN04R1_PHYSICAL_DENIED:$reason"}
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -eq 'true') {
  Deny 'requires_physical_interactive_windows_no_CI'
}
$session=(Get-Process -Id $PID).SessionId
if ($session -le 0){Deny 'noninteractive_operator'}
$head=(git rev-parse HEAD 2>$null)
if ($LASTEXITCODE -ne 0 -or $head.Trim() -ine $ExpectedSourceHead){
  Deny 'different_exact_source_HEAD'
}
$desktopPath=(Resolve-Path -LiteralPath $ExpectedDesktopExecutable).Path
if ((Get-FileHash -LiteralPath $desktopPath -Algorithm SHA256).Hash -ine $ExpectedDesktopSha256){
  Deny 'desktop_binary_hash_mismatch'
}
$running=@(Get-Process -Name lu-ide-desktop -ErrorAction SilentlyContinue |
  Where-Object {$_.SessionId -eq $session -and $_.MainWindowHandle -ne [IntPtr]::Zero})
if ($running.Count -ne 1){Deny 'one_visible_installed_desktop_required'}
try {$runningPath=(Resolve-Path -LiteralPath $running[0].MainModule.FileName).Path}
catch {Deny 'process_image_unverifiable'}
if (![string]::Equals($runningPath,$desktopPath,[StringComparison]::OrdinalIgnoreCase)){
  Deny 'running_desktop_not_reviewed_image'
}
$bundle=(Resolve-Path -LiteralPath $InstalledOwnerBundle).Path
$provenance=Join-Path $bundle 'fin04-owner-provenance.json'
if (!(Test-Path -LiteralPath $provenance -PathType Leaf)){Deny 'missing_first_party_provenance'}
$pin=Get-Content -LiteralPath $provenance -Raw | ConvertFrom-Json
if ($pin.schema -ne 'lu.ide.fin04.first-party-lldb-owner.v1' -or
    $pin.productionDebuggerActions -ne $false -or
    $pin.replayAuthorized -ne $false -or
    $pin.observerOnly -ne $true){
  Deny 'owner_pin_or_production_action_authority_changed'
}
$actual=(Get-FileHash -LiteralPath (Join-Path $bundle 'bin/lldb-dap.exe') -Algorithm SHA256).Hash
if ($actual -ine $pin.lldbDapExeSha256){Deny 'installed_first_party_LLDB_mismatch'}
$profile=[IO.Path]::GetFullPath($ProfileDirectory)
$journal=Join-Path $profile 'debug-fixture-receipts.jsonl'
if (!(Test-Path -LiteralPath $journal -PathType Leaf)){Deny 'original_debug_journal_absent'}
$before=@(Get-ChildItem -LiteralPath $profile -Filter 'lu-debug-action-*.ndjson' -File |
  Select-Object -ExpandProperty Name)
$originalJournalHash=(Get-FileHash -LiteralPath $journal -Algorithm SHA256).Hash
Write-Host "FIN04R1_INTERACTIVE_SCENARIO=$Scenario"
Write-Host 'In the visible, hash-verified Lu IDE use the FIN-04R read-only review controls.'
Write-Host 'The IDE itself must prompt separately for protected LLDB launch and for the ONE action review.'
Write-Host 'No production step/breakpoint command may execute. No second approval may reuse the first challenge.'
switch ($Scenario) {
  'ApproveReviewStep' {Write-Host 'Choose review Step Over, approve the native ACTION review, verify no step was sent.'}
  'ApproveReviewBreakpoint' {Write-Host 'Choose review Breakpoint, approve the native ACTION review, verify no setBreakpoints was sent.'}
  'DeclineReviewStep' {Write-Host 'Approve debugger launch, decline the second native ACTION review.'}
  'CancelWhileDialog' {Write-Host 'Approve launch, cancel debugger while native ACTION modal is open. Any late approval must be revoked.'}
  'TimeoutReview' {Write-Host 'Approve launch and leave the second native ACTION modal unanswered for 35+ seconds.'}
  'RestartNoReplay' {Write-Host 'Finish a one-use review then completely close and restart the desktop; no debugger step, breakpoint or action may replay.'}
}
$ack=Read-Host 'After operating REAL dialogs, enter WITNESSED (nothing is automated)'
if ($ack -cne 'WITNESSED'){Deny 'human_confirmation_missing'}
$new=@(Get-ChildItem -LiteralPath $profile -Filter 'lu-debug-action-*.ndjson' -File |
  Where-Object {$before -notcontains $_.Name})
if ($new.Count -ne 1){Deny "original_action_journal_count_$($new.Count)"}
$lines=@(Get-Content -LiteralPath $new[0].FullName | ForEach-Object {$_ | ConvertFrom-Json})
if ($lines.Count -lt 2 -or $lines[0].phase -ne 'Prepared'){Deny 'missing_prepared_action'}
$identity=$lines[0].action_id_sha256
foreach($e in $lines){
  if ($e.action_id_sha256 -ne $identity -or $e.job_id -ne $lines[0].job_id -or
      $e.plan_sha256 -ne $lines[0].plan_sha256 -or
      $e.owner_sequence -ne $lines[0].owner_sequence){
    Deny 'original_action_identity_changed'
  }
}
$phases=@($lines|Select-Object -ExpandProperty phase)
if ($phases -contains 'Consumed' -or $phases -contains 'ResultVerified'){
  Deny 'production_debugger_action_was_dispatched_or_claimed_verified'
}
$end=$phases[-1]
if ($Scenario -in @('ApproveReviewStep','ApproveReviewBreakpoint','RestartNoReplay')){
  if ($end -ne 'Revoked' -or $phases -notcontains 'Approved' -or
    @($lines|Where-Object {$_.phase -eq 'Approved'})[0].outcome_code -ne 'native_human_confirmed'){
    Deny 'native_human_approval_not_revoked'
  }
}else{
  if ($end -notin @('Denied','Revoked')){Deny 'refused_cancelled_or_timeout_review_not_terminal'}
}
$ownerPattern="lu-owner-$($lines[0].job_id)-*.receipt"
$owners=@(Get-ChildItem -LiteralPath $profile -Filter $ownerPattern -File)
if ($owners.Count -lt 1){Deny 'original_first_party_owner_receipt_missing'}
if (Test-Path -LiteralPath (Join-Path $profile 'fin04-unsafe-execution-authority')){
  Deny 'unexpected_executable_authority'
}
$restarted=$false
if ($RequireRestart -or $Scenario -eq 'RestartNoReplay'){
  Write-Host 'Close the real Lu IDE and manually relaunch the SAME installed executable.'
  $confirm=Read-Host 'Enter RESTARTED only after the new interactive desktop appears'
  if ($confirm -cne 'RESTARTED'){Deny 'physical_restart_not_witnessed'}
  $after=@(Get-Process -Name lu-ide-desktop -ErrorAction SilentlyContinue |
    Where-Object {$_.SessionId -eq $session -and $_.MainWindowHandle -ne [IntPtr]::Zero})
  if ($after.Count -ne 1 -or $after[0].Id -eq $running[0].Id){
    Deny 'new_desktop_process_not_proven'
  }
  $restartImage=(Resolve-Path -LiteralPath $after[0].MainModule.FileName).Path
  if (![string]::Equals($restartImage,$desktopPath,[StringComparison]::OrdinalIgnoreCase) -or
      (Get-FileHash -LiteralPath $restartImage -Algorithm SHA256).Hash -ine $ExpectedDesktopSha256){
    Deny 'restarted_installed_binary_changed'
  }
  $newHash=(Get-FileHash -LiteralPath $new[0].FullName -Algorithm SHA256).Hash
  if (@(Get-ChildItem -LiteralPath $profile -Filter 'lu-debug-action-*.ndjson' -File).Count -ne ($before.Count+1)){
    Deny 'restart_replayed_or_recreated_an_action'
  }
  $restarted=$true
}
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$record=[ordered]@{
  schema='fin04r1.physical-review-fixture.v1'
  scope='interactive_protected_source_review_no_production_debugger_action'
  sourceHead=$ExpectedSourceHead
  desktopSha256=$ExpectedDesktopSha256.ToLowerInvariant()
  bundleLldbSha256=$actual.ToLowerInvariant()
  originalDebugJournalSha256=$originalJournalHash.ToLowerInvariant()
  originalActionId=$identity
  originalActionFileSha256=(Get-FileHash -LiteralPath $new[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  originalOwnerReceiptCount=$owners.Count
  scenario=$Scenario
  terminalActionPhase=$end
  humanWitnessed=$true
  physicallyRestarted=$restarted
  nativeKernelAndPdbAcceptanceIndependentlyVerified=$false
  productionDebuggerControlsAccepted=$false
  observedUtc=[DateTime]::UtcNow.ToString('o')
}
$evidence=Join-Path $EvidenceDirectory "fin04r1-physical-$Scenario.json"
($record|ConvertTo-Json -Depth 4)|Set-Content -LiteralPath $evidence -Encoding UTF8
Write-Host "FIN04R1_PHYSICAL_REVIEW_$Scenario=OBSERVED_NOT_RELEASE_ACCEPTED"
Write-Host "EVIDENCE: $evidence"
