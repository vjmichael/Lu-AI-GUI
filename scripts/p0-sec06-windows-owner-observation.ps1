# SEC-06 Windows installed-owner evidence starter.
# Records ORIGINAL installed binary identity, but intentionally refuses to
# claim human GUI, restart or OS-custody acceptance automatically.
# No elevation, deletion, process kill, permission grant, CI or merge.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$InstalledExe,
    [string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') {
    throw 'SEC06_NOT_AVAILABLE: real Windows operating system required'
}
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$source = (& git -C $repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $source -notmatch '^[0-9a-f]{40}$') {
    throw 'SEC06_SOURCE_IDENTITY_UNAVAILABLE'
}
$inventory = & node (Join-Path $repo 'scripts/verify-source-tree.mjs') 2>&1
if ($LASTEXITCODE -ne 0 -or ($inventory -join ' ') -notmatch 'REPO01_SOURCE_PASS') {
    throw 'SEC06_SOURCE_LOCK_MISMATCH'
}
$installed = Get-Item -LiteralPath $InstalledExe -ErrorAction Stop
if ($installed.PSIsContainer -or ($installed.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'SEC06_UNSAFE_INSTALLED_BINARY_PATH'
}
if ($installed.Extension -ne '.exe') {
    throw 'SEC06_NOT_AN_INSTALLED_EXECUTABLE'
}
if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $EvidenceDirectory = Join-Path $env:TEMP ('lu-sec06-owner-' + [guid]::NewGuid().ToString('N'))
}
New-Item -ItemType Directory -Path $EvidenceDirectory -ErrorAction Stop -Force | Out-Null
$evidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$exeSha = (Get-FileHash -LiteralPath $installed.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$sourceLockSha = (Get-FileHash -LiteralPath (Join-Path $repo 'source-tree.lock.json') -Algorithm SHA256).Hash.ToLowerInvariant()
$checks = @(
    'installed_exe_launch_and_no_worker_without_approval',
    'reviewed_tool_approval_refusal_then_fresh_authorization',
    'chat_A_revoke_does_not_kill_chat_B',
    'cancel_race_and_no_replay_of_external_effect',
    'abrupt_exit_restart_original_journal_reconciliation',
    'guardian_or_Job_loss_shown_as_uncertain_not_completed',
    'tampered_or_unavailable_journal_fails_closed',
    'accessibility_of_security_status_and_cancel_controls',
    'uninstall_reinstall_preserves_approved_user_data',
    'independent_original_Windows_Job_ACTIVE_ZERO_evidence'
)
$manual = @()
foreach ($check in $checks) {
    $manual += [ordered]@{
        id = $check
        status = 'NOT_RUN_OWNER_OBSERVATION_REQUIRED'
        observation = $null
        evidence_reference = $null
    }
}
$observation = [ordered]@{
    schema = 'lu-sec06-installed-owner-observation-v1'
    sourceHead = $source
    sourceLockSha256 = $sourceLockSha
    originalInstalledExePath = $installed.FullName
    originalInstalledExeSha256 = $exeSha
    sourceVerification = 'REPO01_SOURCE_PASS'
    status = 'OWNER_OBSERVATION_PENDING_NOT_SECURITY_ACCEPTED'
    oldPermissionsReusable = $false
    automaticEffectReplayAllowed = $false
    originalWindowsJobCustodyIndependentlyProven = $false
    nativeEvidenceAndReleaseApprovalNotGranted = $true
    manualChecks = $manual
}
$path = Join-Path $evidence 'sec06-owner-observation.PENDING.json'
$observation | ConvertTo-Json -Depth 8 |
    Set-Content -LiteralPath $path -Encoding utf8 -NoNewline
Write-Output ('SEC06_OWNER_EVIDENCE=' + $path)
Write-Output 'SEC06_STATUS=OWNER_OBSERVATION_PENDING_NOT_SECURITY_ACCEPTED'
Write-Output 'Execute the documented interaction matrix in the installed GUI.'
Write-Output 'Attach genuine original screenshots, process/Job receipts, and restart logs.'
Write-Output 'Do not relabel this PENDING record as an independently verified PASS.'
