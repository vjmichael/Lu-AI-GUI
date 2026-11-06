#requires -Version 7.2
# Independent D7C broker crash fixture: open kernel handles before host kill.
# The witness is OS observation, not proof of a different Windows principal.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$OwnerFixture,
  [Parameter(Mandatory=$true)][string]$BrokerExecutable,
  [Parameter(Mandatory=$true)][string]$ReceiptDirectory
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class D7CNativeWitness {
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
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern IntPtr OpenEventW(uint access, bool inherit, string name);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool SetEvent(IntPtr handle);
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
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: native Windows required' }
$exe=[IO.Path]::GetFullPath($OwnerFixture)
$broker=[IO.Path]::GetFullPath($BrokerExecutable)
$root=[IO.Path]::GetFullPath($ReceiptDirectory)
foreach ($file in @($exe,$broker)) {
  if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "DENIED: $file missing" }
}
if (Test-Path -LiteralPath $root) { throw 'DENIED: exclusive recovery test root required' }
New-Item -ItemType Directory -Path $root | Out-Null
$marker=Join-Path $root 'owner.marker'
$brokerRoot=Join-Path $root 'broker-ledger'
New-Item -ItemType Directory -Path $brokerRoot | Out-Null
$hash=(Get-FileHash -LiteralPath $broker -Algorithm SHA256).Hash.ToLowerInvariant()
$owner=$null
$childHandle=[IntPtr]::Zero
$jobHandle=[IntPtr]::Zero
try {
  $arguments = @('--owner', ('"' + $broker + '"'), $hash,
                 ('"' + $brokerRoot + '"'), ('"' + $marker + '"'))
  $owner=Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -WindowStyle Hidden
  $deadline=[DateTime]::UtcNow.AddSeconds(25)
  while (!(Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $deadline) {
    if ($owner.HasExited) { throw "DENIED: broker owner failed before admission: $($owner.ExitCode)" }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $marker)) { throw 'DENIED: broker owner never admitted worker' }
  $lines=@(Get-Content -LiteralPath $marker)
  if ($lines.Count -ne 4) { throw 'DENIED: malformed Job marker' }
  $originalPid=[int]$lines[0]
  $childPid=[int]$lines[1]
  $jobId=$lines[2]
  $plan=$lines[3]
  if ($originalPid -ne $owner.Id -or $childPid -le 0 -or
      $jobId -notmatch '^job-[0-9a-f]{32}$' -or $plan -notmatch '^[0-9a-f]{64}$') {
    throw 'DENIED: inconsistent owner/worker/plan'
  }
  $childHandle=[D7CNativeWitness]::OpenProcess(0x00100000,$false,$childPid)
  if ($childHandle -eq [IntPtr]::Zero -or
      [D7CNativeWitness]::WaitForSingleObject($childHandle,0) -ne 258) {
    throw 'DENIED: no live worker kernel handle before crash'
  }
  Stop-Process -Id $owner.Id -Force
  [void]$owner.WaitForExit(8000)
  if (!$owner.HasExited) { throw 'DENIED: owner survived injected crash' }
  if ([D7CNativeWitness]::WaitForSingleObject($childHandle,10000) -ne 0) {
    throw 'DENIED: independent broker failed to reap child'
  }
  $jobName = "Local\LuIdeD7-$jobId-$($plan.Substring(0,16))"
  $jobHandle=[D7CNativeWitness]::OpenJobObjectW(0x0004,$false,$jobName)
  if ($jobHandle -eq [IntPtr]::Zero) {
    throw 'DENIED: named Job vanished; independent keeper not live'
  }
  $accounting=New-Object 'D7CNativeWitness+BasicAccounting'
  $size=[Runtime.InteropServices.Marshal]::SizeOf($accounting)
  if (![D7CNativeWitness]::QueryInformationJobObject(
      $jobHandle,1,[ref]$accounting,$size,[IntPtr]::Zero) -or
      $accounting.ActiveProcesses -ne 0) {
    throw 'DENIED: retained kernel Job is not empty'
  }
  $journal=Join-Path $brokerRoot "broker-$jobId-$($plan.Substring(0,16)).jsonl"
  $deadline=[DateTime]::UtcNow.AddSeconds(10)
  $states=@()
  do {
    if (Test-Path -LiteralPath $journal) {
      $states=@(Get-Content -LiteralPath $journal | Where-Object { $_ } |
          ForEach-Object { ($_ | ConvertFrom-Json).body.state })
    }
    if ($states -contains 'tree_empty') { break }
    Start-Sleep -Milliseconds 100
  } while ([DateTime]::UtcNow -lt $deadline)
  if ($states.Count -lt 4 -or
      ($states[0..3] -join ',') -ne 'ready,owner_exited,kill_requested,tree_empty') {
    throw "DENIED: durable broker receipts absent or misordered: $states"
  }
  & $exe --audit "$brokerRoot" "$jobId" "$plan"
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: native hash chain witness verification failed' }
  # The kernel Job is confirmed empty and the independent audit succeeded.
  # Release this test-only keeper explicitly so CI cannot leak a 15-minute
  # background process. A production supervisor sends this only after its
  # own durable Cancelled recovery receipt.
  $releaseName="Local\LuIdeD7BrokerRelease-$jobId-$($plan.Substring(0,16))"
  $release=[D7CNativeWitness]::OpenEventW(0x0002,$false,$releaseName)
  if ($release -eq [IntPtr]::Zero) { throw 'DENIED: broker recovery-release event missing' }
  try {
    if (![D7CNativeWitness]::SetEvent($release)) {
      throw 'DENIED: native recovery-release signal rejected'
    }
  } finally { [void][D7CNativeWitness]::CloseHandle($release) }
  $released=$false
  $limit=[DateTime]::UtcNow.AddSeconds(8)
  while ([DateTime]::UtcNow -lt $limit) {
    if ((Get-Content -LiteralPath $journal -Raw) -match '"state":"recovered_release"') {
      $released=$true
      break
    }
    Start-Sleep -Milliseconds 50
  }
  if (!$released) { throw 'DENIED: independent keeper did not fsync recovery release' }
  Write-Host 'PASS: independent keeper remained live, reaped tree, held named Job, fsynced evidence, and released after verification'
} finally {
  if ($jobHandle -ne [IntPtr]::Zero) { [void][D7CNativeWitness]::CloseHandle($jobHandle) }
  if ($childHandle -ne [IntPtr]::Zero) { [void][D7CNativeWitness]::CloseHandle($childHandle) }
  if ($owner -and !$owner.HasExited) {
    Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue
  }
}
