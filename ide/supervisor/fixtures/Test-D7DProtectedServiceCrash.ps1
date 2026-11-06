#requires -Version 7.2
# D7D external crash observer: fixed native owner/child, real protected SCM
# service and kernel Job evidence. Protected journal is NOT readable by host.
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$FixtureExecutable,
  [Parameter(Mandatory)][string]$ServiceExecutable,
  [Parameter(Mandatory)][string]$ExpectedServiceSha256
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: Windows native acceptance required' }
if ($ExpectedServiceSha256 -notmatch '^[0-9a-fA-F]{64}$') {
  throw 'DENIED: reviewed exact service binary SHA-256 required'
}
$fixture=(Resolve-Path -LiteralPath $FixtureExecutable).Path
$service=(Resolve-Path -LiteralPath $ServiceExecutable).Path
$root=Join-Path $env:RUNNER_TEMP ('d7d-crash-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$marker=Join-Path $root 'owner.marker'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class D7DProtectedObserver {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern IntPtr OpenJobObjectW(uint access, bool inherit, string name);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool QueryInformationJobObject(IntPtr job, int infoClass,
      out BasicAccounting info, int length, IntPtr returned);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr handle);
  [StructLayout(LayoutKind.Sequential)]
  public struct BasicAccounting {
    public long TotalUserTime;
    public long TotalKernelTime;
    public uint TotalPageFaultCount;
    public uint TotalProcesses;
    public uint ActiveProcesses;
    public uint TotalTerminatedProcesses;
  }
}
'@
$owner=$null
$child=[IntPtr]::Zero
$job=[IntPtr]::Zero
try {
  $args=@('--owner',('"' + $service + '"'),$ExpectedServiceSha256,('"' + $marker + '"'))
  $owner=Start-Process -FilePath $fixture -ArgumentList $args -PassThru -WindowStyle Hidden
  $deadline=[DateTime]::UtcNow.AddSeconds(25)
  while (!(Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $deadline) {
    if ($owner.HasExited) { throw "DENIED: owner failed admission: $($owner.ExitCode)" }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $marker)) { throw 'DENIED: service READY absent' }
  $lines=@(Get-Content -LiteralPath $marker)
  if ($lines.Count -ne 4) { throw 'DENIED: malformed owner Job marker' }
  $ownerPid=[int]$lines[0]
  $childPid=[int]$lines[1]
  $jobId=$lines[2]
  $plan=$lines[3]
  if ($ownerPid -ne $owner.Id -or $childPid -le 0 -or
      $jobId -notmatch '^job-[0-9a-f]{32}$' -or
      $plan -notmatch '^[a-f0-9]{64}$') {
    throw 'DENIED: inconsistent marker/job/plan'
  }
  $child=[D7DProtectedObserver]::OpenProcess(0x00100000,$false,$childPid)
  if ($child -eq [IntPtr]::Zero -or
      [D7DProtectedObserver]::WaitForSingleObject($child,0) -ne 258) {
    throw 'DENIED: child was not alive before host crash'
  }
  # Hostile pre-crash: even the approved host binary cannot release a live Job.
  & $fixture --recover $service $ExpectedServiceSha256 $jobId $plan 2>$null
  if ($LASTEXITCODE -eq 0) { throw 'DENIED: service released a live Job prematurely' }
  if ([D7DProtectedObserver]::WaitForSingleObject($child,0) -ne 258) {
    throw 'DENIED: premature recovery killed or released live worker'
  }
  Stop-Process -Id $owner.Id -Force
  [void]$owner.WaitForExit(8000)
  if (!$owner.HasExited) { throw 'DENIED: owner survived injected crash' }
  if ([D7DProtectedObserver]::WaitForSingleObject($child,12000) -ne 0) {
    throw 'DENIED: independent service failed to terminate Job child'
  }
  # DO NOT hold a Job handle before the crash: otherwise this observer could
  # keep the Job alive and falsely certify service ownership.
  $jobName="Global\LuIdeD7-$jobId-$($plan.Substring(0,16))"
  $job=[D7DProtectedObserver]::OpenJobObjectW(0x0004,$false,$jobName)
  if ($job -eq [IntPtr]::Zero) {
    throw 'DENIED: original named Job disappeared; service keeper not proven live'
  }
  $accounting=New-Object 'D7DProtectedObserver+BasicAccounting'
  $size=[Runtime.InteropServices.Marshal]::SizeOf($accounting)
  if (![D7DProtectedObserver]::QueryInformationJobObject(
      $job,1,[ref]$accounting,$size,[IntPtr]::Zero) -or
      $accounting.ActiveProcesses -ne 0) {
    throw 'DENIED: retained Job is not empty'
  }
  # A restarted native executable opens its own kernel handle and asks the
  # SCM service to fsync recovered_release before returning.
  & $fixture --recover $service $ExpectedServiceSha256 $jobId $plan
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: protected recovery ACK not durable' }
  [void][D7DProtectedObserver]::CloseHandle($job)
  $job=[IntPtr]::Zero
  Start-Sleep -Milliseconds 300
  $unexpected=[D7DProtectedObserver]::OpenJobObjectW(0x0004,$false,$jobName)
  if ($unexpected -ne [IntPtr]::Zero) {
    [void][D7DProtectedObserver]::CloseHandle($unexpected)
    throw 'DENIED: service did not release recovered original Job'
  }
  Write-Host 'PASS: host-crash -> separate service Job kill -> child exit -> retained empty original Job -> authenticated recovery ACK -> Job release'
} finally {
  if ($job -ne [IntPtr]::Zero) { [void][D7DProtectedObserver]::CloseHandle($job) }
  if ($child -ne [IntPtr]::Zero) { [void][D7DProtectedObserver]::CloseHandle($child) }
  if ($owner -and !$owner.HasExited) {
    Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue
  }
}
