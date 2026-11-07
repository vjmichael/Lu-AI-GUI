#requires -Version 7.2
# D7D hostile service-crash fixture. No model/desktop/workspace commands.
# A service crash while the host is alive MUST kill its Job, and a restart
# must NOT invent recovery from a missing kernel object.
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$FixtureExecutable,
  [Parameter(Mandatory)][string]$ServiceExecutable,
  [Parameter(Mandatory)][string]$ExpectedServiceSha256
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: Windows-only fixture' }
if ($ExpectedServiceSha256 -notmatch '^[0-9a-fA-F]{64}$') {
  throw 'DENIED: fixed SHA256 required'
}
$fixture=(Resolve-Path -LiteralPath $FixtureExecutable).Path
$service=(Resolve-Path -LiteralPath $ServiceExecutable).Path
$root=Join-Path $env:RUNNER_TEMP ('d7d-service-loss-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$marker=Join-Path $root 'owner.marker'
$ownerTrace=Join-Path $root 'owner-stages.stderr'
$watchdog=[IO.Path]::ChangeExtension($marker,'.watchdog')
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class D7DLossObserver {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern IntPtr OpenJobObjectW(uint access, bool inherit, string name);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr handle);
}
'@
$owner=$null
$child=[IntPtr]::Zero
try {
  $args=@('--owner-watch',('"' + $service + '"'),$ExpectedServiceSha256,('"' + $marker + '"'))
  $owner=Start-Process -FilePath $fixture -ArgumentList $args -PassThru -WindowStyle Hidden -RedirectStandardError $ownerTrace
  $deadline=[DateTime]::UtcNow.AddSeconds(25)
  while (!(Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $deadline) {
    if ($owner.HasExited) {
      # Emit only fixed native stage identifiers from this disposable CI fixture.
      # Never dump raw stderr, paths, project text or executable arguments.
      foreach ($stage in @(Get-Content -LiteralPath $ownerTrace -ErrorAction SilentlyContinue)) {
        if ($stage -cmatch '^PROTECTED_SERVICE_ADMISSION_STAGE=[a-z0-9_]{1,80}
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $marker)) { throw 'DENIED: owner marker absent' }
  $parts=@(Get-Content -LiteralPath $marker)
  if ($parts.Count -ne 4 -or $parts[2] -notmatch '^job-[0-9a-f]{32}$' -or
      $parts[3] -notmatch '^[a-f0-9]{64}$' -or [int]$parts[0] -ne $owner.Id) {
    throw 'DENIED: malformed native marker'
  }
  $child=[D7DLossObserver]::OpenProcess(0x00100000,$false,[int]$parts[1])
  if ($child -eq [IntPtr]::Zero -or
      [D7DLossObserver]::WaitForSingleObject($child,0) -ne 258) {
    throw 'DENIED: no live contained child before service crash'
  }
  $svc=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
  if (!$svc -or $svc.State -ne 'Running' -or $svc.ProcessId -le 0) {
    throw 'DENIED: protected service not running'
  }
  # This kills the separate protected principal WITHOUT killing the host.
  Stop-Process -Id $svc.ProcessId -Force
  $deadline=[DateTime]::UtcNow.AddSeconds(12)
  while (!(Test-Path -LiteralPath $watchdog) -and [DateTime]::UtcNow -lt $deadline) {
    if ($owner.HasExited) { break }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $watchdog) -or
      (Get-Content -LiteralPath $watchdog -Raw) -ne 'service_lost_job_reaped') {
    throw 'DENIED: live owner did not detect service loss and kill its Job'
  }
  if ([D7DLossObserver]::WaitForSingleObject($child,5000) -ne 0) {
    throw 'DENIED: service-loss worker child survived'
  }
  [void]$owner.WaitForExit(5000)
  if (!$owner.HasExited -or $owner.ExitCode -ne 0) {
    throw 'DENIED: owner did not finish watchdog cleanup'
  }
  $oldName="Global\LuIdeD7-$($parts[2])-$($parts[3].Substring(0,16))"
  $old=[D7DLossObserver]::OpenJobObjectW(0x0004,$false,$oldName)
  if ($old -ne [IntPtr]::Zero) {
    [void][D7DLossObserver]::CloseHandle($old)
    throw 'DENIED: no trusted service should retain the old Job after crash'
  }
  # The service can restart, but a missing old kernel object is not a
  # trustworthy witness; the approved host must reject recovery.
  $deadline=[DateTime]::UtcNow.AddSeconds(10)
  while ((Get-Service LuIdeRecoveryBroker).Status -ne 'Stopped' -and
      [DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 100
  }
  & sc.exe start LuIdeRecoveryBroker | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: protected service failed restart' }
  & $fixture --recover $service $ExpectedServiceSha256 $parts[2] $parts[3] 2>$null
  if ($LASTEXITCODE -eq 0) {
    throw 'DENIED: missing original kernel Job falsely accepted after service crash'
  }
  Write-Host 'PASS: service death detected by host; worker reaped; missing Job stays unrecoverable'
} finally {
  if ($child -ne [IntPtr]::Zero) { [void][D7DLossObserver]::CloseHandle($child) }
  if ($owner -and !$owner.HasExited) {
    Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue
  }
}
) {
          Write-Host $stage
        }
      }
      throw "DENIED: admission failed before service crash: $($owner.ExitCode)"
    }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $marker)) { throw 'DENIED: owner marker absent' }
  $parts=@(Get-Content -LiteralPath $marker)
  if ($parts.Count -ne 4 -or $parts[2] -notmatch '^job-[0-9a-f]{32}$' -or
      $parts[3] -notmatch '^[a-f0-9]{64}$' -or [int]$parts[0] -ne $owner.Id) {
    throw 'DENIED: malformed native marker'
  }
  $child=[D7DLossObserver]::OpenProcess(0x00100000,$false,[int]$parts[1])
  if ($child -eq [IntPtr]::Zero -or
      [D7DLossObserver]::WaitForSingleObject($child,0) -ne 258) {
    throw 'DENIED: no live contained child before service crash'
  }
  $svc=Get-CimInstance Win32_Service -Filter "Name='LuIdeRecoveryBroker'"
  if (!$svc -or $svc.State -ne 'Running' -or $svc.ProcessId -le 0) {
    throw 'DENIED: protected service not running'
  }
  # This kills the separate protected principal WITHOUT killing the host.
  Stop-Process -Id $svc.ProcessId -Force
  $deadline=[DateTime]::UtcNow.AddSeconds(12)
  while (!(Test-Path -LiteralPath $watchdog) -and [DateTime]::UtcNow -lt $deadline) {
    if ($owner.HasExited) { break }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $watchdog) -or
      (Get-Content -LiteralPath $watchdog -Raw) -ne 'service_lost_job_reaped') {
    throw 'DENIED: live owner did not detect service loss and kill its Job'
  }
  if ([D7DLossObserver]::WaitForSingleObject($child,5000) -ne 0) {
    throw 'DENIED: service-loss worker child survived'
  }
  [void]$owner.WaitForExit(5000)
  if (!$owner.HasExited -or $owner.ExitCode -ne 0) {
    throw 'DENIED: owner did not finish watchdog cleanup'
  }
  $oldName="Global\LuIdeD7-$($parts[2])-$($parts[3].Substring(0,16))"
  $old=[D7DLossObserver]::OpenJobObjectW(0x0004,$false,$oldName)
  if ($old -ne [IntPtr]::Zero) {
    [void][D7DLossObserver]::CloseHandle($old)
    throw 'DENIED: no trusted service should retain the old Job after crash'
  }
  # The service can restart, but a missing old kernel object is not a
  # trustworthy witness; the approved host must reject recovery.
  $deadline=[DateTime]::UtcNow.AddSeconds(10)
  while ((Get-Service LuIdeRecoveryBroker).Status -ne 'Stopped' -and
      [DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 100
  }
  & sc.exe start LuIdeRecoveryBroker | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: protected service failed restart' }
  & $fixture --recover $service $ExpectedServiceSha256 $parts[2] $parts[3] 2>$null
  if ($LASTEXITCODE -eq 0) {
    throw 'DENIED: missing original kernel Job falsely accepted after service crash'
  }
  Write-Host 'PASS: service death detected by host; worker reaped; missing Job stays unrecoverable'
} finally {
  if ($child -ne [IntPtr]::Zero) { [void][D7DLossObserver]::CloseHandle($child) }
  if ($owner -and !$owner.HasExited) {
    Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue
  }
}
