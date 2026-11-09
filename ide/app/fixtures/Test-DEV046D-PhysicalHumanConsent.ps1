# DEV-04.6D witnessed PHYSICAL Windows dialog acceptance, not CI consent.
# Requires an independently compiled and pinned Windows developer-only Lu IDE.
# This script NEVER clicks an approval button, sends DAP or impersonates a human.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$ProfileDirectory,
  [Parameter(Mandatory=$true)][string]$AuditExecutable,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
  [Parameter(Mandatory=$true)]
  [ValidateSet('ApproveStep','RefuseStep','CancelWhilePrompted','ApproveBreakpoint')]
  [string]$Scenario
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason) { throw "DEV046D_PHYSICAL_ACCEPTANCE_DENIED:$reason" }
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -eq 'true') {
  Deny 'requires_physical_interactive_windows_user_not_ci'
}
$userSession=(Get-Process -Id $PID).SessionId
if ($userSession -le 0) { Deny 'interactive_user_session_missing' }
$profile=[IO.Path]::GetFullPath($ProfileDirectory)
if (!(Test-Path -LiteralPath $profile -PathType Container)) { Deny 'native_profile_missing' }
$journal=Join-Path $profile 'debug-fixture-receipts.jsonl'
if (!(Test-Path -LiteralPath $journal -PathType Leaf)) { Deny 'native_supervisor_journal_missing' }
$audit=(Resolve-Path -LiteralPath $AuditExecutable).Path
if ([IO.Path]::GetFileNameWithoutExtension($audit) -ne 'dev046d_restart_receipt_audit') {
  Deny 'unexpected_native_auditor'
}
# A genuine visible application must be in the same interactive Windows user
# session, rather than a headless runner, task service or standalone DAP fixture.
$desktop=@(Get-Process -Name 'lu-ide-desktop' -ErrorAction SilentlyContinue |
  Where-Object { $_.SessionId -eq $userSession -and $_.MainWindowHandle -ne [IntPtr]::Zero })
if ($desktop.Count -ne 1) { Deny 'one_visible_lu_ide_desktop_required' }
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$before=@(Get-ChildItem -LiteralPath $profile -Filter 'lu-debug-action-*.ndjson' -File |
  Select-Object -ExpandProperty Name)
Write-Host "Physical approval acceptance: $Scenario"
Write-Host 'In the open Lu IDE, use the matching developer-only Step Over / Breakpoint button.'
Write-Host 'The IDE itself must show TWO separate native dialogs: launch consent and per-action consent.'
switch($Scenario) {
  'ApproveStep' { Write-Host 'Click Approve in both dialogs; observe a new independently verified stop.' }
  'RefuseStep' { Write-Host 'Approve launch, but click Decline in the OPERATION dialog.' }
  'CancelWhilePrompted' {
    Write-Host 'Approve launch, leave the OPERATION dialog open and click Request debugger cancellation in Lu IDE.'
    Write-Host 'Do not click Approve after cancellation; if you do, the old receipt must still be unusable.'
  }
  'ApproveBreakpoint' {
    Write-Host 'Approve launch then approve the same-line breakpoint; an LLDB ACK is NOT a verified hit.'
  }
}
$ack=Read-Host 'After interacting with the real native dialogs, enter WITNESSED to inspect receipts'
if ($ack -cne 'WITNESSED') { Deny 'no_human_witness_confirmation' }
$after=@(Get-ChildItem -LiteralPath $profile -Filter 'lu-debug-action-*.ndjson' -File |
  Where-Object { $before -notcontains $_.Name })
if ($after.Count -ne 1) { Deny "expected_one_new_action_journal_found_$($after.Count)" }
$file=$after[0]
$events=@(Get-Content -LiteralPath $file.FullName | ForEach-Object { $_ | ConvertFrom-Json })
if ($events.Count -lt 2 -or $events[0].phase -ne 'Prepared') {
  Deny 'action_journal_missing_prepared_phase'
}
foreach($event in $events) {
  if ($event.action_id_sha256 -ne $events[0].action_id_sha256 -or
      $event.job_id -ne $events[0].job_id -or
      $event.owner_sequence -ne $events[0].owner_sequence) {
    Deny 'journal_identity_changed'
  }
}
$phases=@($events | Select-Object -ExpandProperty phase)
$terminal=$phases[-1]
$approved=@($events | Where-Object { $_.phase -eq 'Approved' })
$consumed=@($events | Where-Object { $_.phase -eq 'Consumed' })
switch($Scenario) {
  'ApproveStep' {
    if ($terminal -ne 'ResultVerified' -or $approved.Count -ne 1 -or
        $consumed.Count -ne 1 -or
        $approved[0].outcome_code -ne 'native_human_confirmed' -or
        $events[-1].outcome_code -notmatch '^native_dialog_independent_os_step:[a-f0-9]{64}$') {
      Deny 'native_human_step_and_independent_OS_result_unproven'
    }
  }
  'ApproveBreakpoint' {
    if ($terminal -ne 'ResultUncertain' -or $approved.Count -ne 1 -or
        $consumed.Count -ne 1 -or
        $approved[0].outcome_code -ne 'native_human_confirmed') {
      Deny 'breakpoint_missing_human_approval_or_false_verification'
    }
  }
  'RefuseStep' {
    if ($terminal -ne 'Denied' -or $approved.Count -ne 0 -or $consumed.Count -ne 0) {
      Deny 'refused_native_dialog_still_consumed_or_dispatched'
    }
  }
  'CancelWhilePrompted' {
    if ($consumed.Count -ne 0 -or $approved.Count -ne 0 -or
        $terminal -notin @('Denied','Revoked')) {
      Deny 'cancelled_human_prompt_granted_or_left_live'
    }
  }
}
# Rust audit rechecks the complete hash chain, all foreign action journals,
# original persistent Job identity, and absence of unreconciled native Job trees.
& $audit $journal
if ($LASTEXITCODE -ne 0) { Deny 'independent_native_restart_audit_failed' }
$observed=[ordered]@{
  scope='physical_interactive_developer_build_not_signed_release'
  scenario=$Scenario
  desktopPid=$desktop[0].Id
  sessionId=$userSession
  actionId=$events[0].action_id_sha256
  jobId=$events[0].job_id
  ownerSequence=$events[0].owner_sequence
  journalPhase=$terminal
  actionFileSha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName).Hash
  witnessedUtc=[DateTime]::UtcNow.ToString('o')
  subjectToNativeIndependentReview=$true
}
$destination=Join-Path $EvidenceDirectory ('dev046d-physical-'+$Scenario+'.json')
($observed | ConvertTo-Json -Depth 4) | Set-Content -LiteralPath $destination -Encoding UTF8
Write-Host "DEV046D_PHYSICAL_DIALOG_$Scenario=OBSERVED;native_audit=PASS"
Write-Host "EVIDENCE: $destination"
Write-Host 'No signed installed release, unexecuted scenario or nonphysical UI consent is inferred.'
