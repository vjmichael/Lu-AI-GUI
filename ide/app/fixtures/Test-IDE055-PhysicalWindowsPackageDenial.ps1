# IDE-05.5 *physical* Windows installed-app negative acceptance.
# Windows uv execution is NOT approved (staged source ACL missing). This
# captures a real operator observation of fail-closed UI only, never
# pretends to witness a package approval or successfully installed files.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExecutable,
  [Parameter(Mandatory=$true)][string]$ProfileDirectory,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason) { throw "IDE055_PHYSICAL_WINDOWS_DENIED:$reason" }
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -eq 'true') {
  Deny 'requires_actual_interactive_windows_not_ci'
}
$session=(Get-Process -Id $PID).SessionId
if ($session -le 0) { Deny 'not_an_interactive_desktop_session' }
$binary=(Resolve-Path -LiteralPath $InstalledExecutable).Path
if ([IO.Path]::GetFileName($binary) -ne 'lu-ide-desktop.exe') {
  Deny 'wrong_installed_app_executable'
}
$exeInfo=Get-Item -LiteralPath $binary
if ($exeInfo.LinkType) { Deny 'installed_executable_is_alias' }
$processes=@(Get-Process -Name 'lu-ide-desktop' -ErrorAction SilentlyContinue |
  Where-Object { $_.SessionId -eq $session -and
    $_.MainWindowHandle -ne [IntPtr]::Zero -and
    $_.Path -eq $binary })
if ($processes.Count -ne 1) { Deny 'one_visible_exact_installed_app_required' }
$profile=[IO.Path]::GetFullPath($ProfileDirectory)
if (!(Test-Path -LiteralPath $profile -PathType Container)) {
  Deny 'native_profile_directory_missing'
}
$journal=Join-Path $profile 'build-job-receipts.jsonl'
$packageJournal=Join-Path $profile 'package-output-lifecycle-v1.jsonl'
function CaptureReceipts {
  $result=@{}
  foreach($path in @($journal,$packageJournal)) {
    $key=[IO.Path]::GetFileName($path)
    if (Test-Path -LiteralPath $path -PathType Leaf) {
      $result[$key]=(Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
    } else { $result[$key]='ABSENT' }
  }
  return $result
}
$before=CaptureReceipts
Write-Host 'IDE-05.5 physical Windows NEGATIVE package gate.'
Write-Host 'In the visible Lu IDE choose a registered project and Packages & Workflows.'
Write-Host 'Attempt to inspect and revalidate one uv Sync preview.'
Write-Host 'The Windows offline Sync worker MUST remain disabled and must NOT open an execution-approval dialog.'
Write-Host 'No uv package process, AppContainer grant or project mutation may occur.'
$ack=Read-Host 'After checking the actual physical application, enter WINDOWS_UV_DENIED'
if ($ack -cne 'WINDOWS_UV_DENIED') { Deny 'human_witness_missing_or_execution_was_enabled' }
$after=CaptureReceipts
foreach($key in @($before.Keys)) {
  if ($before[$key] -cne $after[$key]) {
    Deny "native_journal_changed_during_denied_package_operation_$key"
  }
}
$dest=[IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Path $dest -Force | Out-Null
$receipt=[ordered]@{
  schema='ide055-physical-windows-negative-v1'
  scope='physical_interactive_windows_installed_app'
  result='operator_witnessed_fail_closed_only'
  installedExecutableSha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash
  visibleDesktopPid=$processes[0].Id
  desktopSessionId=$session
  originalD3JournalShaBefore=$before['build-job-receipts.jsonl']
  originalD3JournalShaAfter=$after['build-job-receipts.jsonl']
  packageJournalShaBefore=$before['package-output-lifecycle-v1.jsonl']
  packageJournalShaAfter=$after['package-output-lifecycle-v1.jsonl']
  nativeProtectedUvExecuted=$false
  realHumanApprovalTested=$false
  originalFilesystemProvenUnchanged=$false
  signedReleaseAccepted=$false
  windowsSourceAclGate='OPEN'
  witnessedUtc=[DateTime]::UtcNow.ToString('o')
}
$file=Join-Path $dest 'ide055-physical-windows-denial.json'
($receipt | ConvertTo-Json -Depth 5) | Set-Content -LiteralPath $file -Encoding UTF8
Write-Host "IDE055_PHYSICAL_WINDOWS_FAIL_CLOSED_WITNESS=CAPTURED;FILE=$file"
Write-Host 'No package operation or installed dependency acceptance is inferred.'
