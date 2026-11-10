# Group 6A physical installed Windows Source Control acceptance.
# Requires real operator, never synthetic CI clicks or automatic approval.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExecutable,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Deny([string]$reason) { throw "G06A_PHYSICAL_DENIED:$reason" }
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -eq 'true') {
  Deny 'interactive_windows_operator_required'
}
$session=(Get-Process -Id $PID).SessionId
if ($session -le 0) { Deny 'desktop_session_missing' }
$exe=(Resolve-Path -LiteralPath $InstalledExecutable).Path
if ([IO.Path]::GetFileName($exe) -ne 'lu-ide-desktop.exe') { Deny 'wrong_installed_executable' }
$fi=Get-Item -LiteralPath $exe
if ($fi.LinkType) { Deny 'executable_alias_denied' }
$sha=(Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash
function RunningDesktop {
  return @(Get-Process -Name 'lu-ide-desktop' -ErrorAction SilentlyContinue |
    Where-Object { $_.SessionId -eq $session -and
      $_.MainWindowHandle -ne [IntPtr]::Zero -and $_.Path -eq $exe })
}
$before=@(RunningDesktop)
if ($before.Count -ne 1) { Deny 'one_visible_installed_desktop_required' }
Write-Host "IDE G06A physical installed source-control acceptance. Exact executable SHA256: $sha"
Write-Host 'Open a registered project with two local branches and one linked worktree.'
Write-Host 'Verify the Source Control panel identifies branch tips, stored upstream, refs, health and linked worktree.'
Write-Host 'Verify linked worktree absolute paths are NOT exposed or automatically opened.'
Write-Host 'Switch to a second registered project in the project picker. Verify old branches/history are NOT displayed.'
Write-Host 'Return to the first project. Verify a tracked file history and blame from local committed objects.'
$first=Read-Host 'When actually witnessed, type G06A_SWITCH_VERIFIED (anything else aborts)'
if ($first -cne 'G06A_SWITCH_VERIFIED') { Deny 'operator_switch_witness_missing' }
Write-Host 'Close the installed desktop normally, then reopen the EXACT installed executable.'
$ack=Read-Host 'After reopening the app and verifying the proper registered project and no stale other-project data, type G06A_RESTART_VERIFIED'
if ($ack -cne 'G06A_RESTART_VERIFIED') { Deny 'operator_restart_witness_missing' }
$after=@(RunningDesktop)
if ($after.Count -ne 1) { Deny 'restarted_visible_desktop_missing_or_ambiguous' }
if ($after[0].Id -eq $before[0].Id) { Deny 'process_was_not_restarted' }
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash -cne $sha) {
  Deny 'installed_executable_changed_during_acceptance'
}
$dest=[IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Force -Path $dest | Out-Null
$receipt=[ordered]@{
  schema='ide-g06a-physical-installed-inventory-v1'
  scope='windows_visible_installed_app_operator_witness'
  installedExeSha256=$sha
  originalPid=$before[0].Id
  restartedPid=$after[0].Id
  desktopSessionId=$session
  operatorObservedBranchWorktreeAndBlame=$true
  operatorObservedProjectSwitchAndRestart=$true
  nativeGitObjectsIndependentlyHashed=$false
  originalRepositoryUnchangedIndependentlyVerified=$false
  gitOperationsAuthorized=$false
  signedReleaseApproved=$false
  witnessedUtc=[DateTime]::UtcNow.ToString('o')
}
$file=Join-Path $dest 'g06a-physical-installed-windows.json'
if (Test-Path -LiteralPath $file) { Deny 'receipt_overwrite_denied' }
($receipt | ConvertTo-Json -Depth 5) | Set-Content -LiteralPath $file -Encoding UTF8
Write-Host "G06A_PHYSICAL_WINDOWS_OPERATOR_WITNESS_CAPTURED:$file"
Write-Host 'This is operator observation, not proof of OS isolation, complete source immutability or signed release.'
