#requires -Version 7.2
<#
Independent native Windows Job owner-crash witness.
This is an adversarial OS fixture, not a Lu IDE cancellation receipt.
The observer holds kernel process handles BEFORE killing the process which
owns JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE; it then witnesses both child
and grandchild termination, rather than trusting a PID lookup afterward.
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$ProbeExe,
  [Parameter(Mandatory=$true)][string]$ReceiptPath
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class LuD7KernelWitness {
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool CloseHandle(IntPtr handle);
}
'@
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: native Windows is required' }
if (!(Test-Path -LiteralPath $ProbeExe -PathType Leaf)) { throw 'DENIED: fixed probe unavailable' }
if (Test-Path -LiteralPath $ReceiptPath) { throw 'DENIED: observer receipt already exists' }
$probe=[IO.Path]::GetFullPath($ProbeExe)
$process=$null
$children=New-Object 'System.Collections.Generic.List[object]'
$marker=$null
$stderr=Join-Path ([IO.Path]::GetTempPath()) ('lu-d7b-native-crash-stderr-'+[guid]::NewGuid().ToString('N')+'.txt')
try {
  # Keep the native reason if the fixed owner fails before its marker. A
  # failing process is never interpreted as a successful kill-on-close proof.
  $process=Start-Process -FilePath $probe -ArgumentList '--fixture-external-crash' -PassThru -WindowStyle Hidden -RedirectStandardError $stderr
  $marker=Join-Path ([IO.Path]::GetTempPath()) "lu-ide-d7b-crash-$($process.Id).txt"
  $deadline=[DateTime]::UtcNow.AddSeconds(20)
  while (!(Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $deadline) {
    if ($process.HasExited) {
      $nativeError=if (Test-Path -LiteralPath $stderr) {
        ((Get-Content -LiteralPath $stderr -Tail 6 -ErrorAction SilentlyContinue) -join ' ')
      } else { 'stderr unavailable' }
      if ($nativeError.Length -gt 1500) { $nativeError=$nativeError.Substring(0,1500) }
      throw "DENIED: Job owner exited before witness admission; exit=$($process.ExitCode); native=$nativeError"
    }
    Start-Sleep -Milliseconds 50
  }
  if (!(Test-Path -LiteralPath $marker)) { throw 'DENIED: witness did not observe live Job tree' }
  $lines=@(Get-Content -LiteralPath $marker)
  if ($lines.Count -ne 3) { throw 'DENIED: malformed Job tree observation' }
  $owner=[int]$lines[0]
  $child=[int]$lines[1]
  $grandchild=[int]$lines[2]
  if ($owner -ne $process.Id -or $child -le 0 -or $grandchild -le 0 -or
      $child -eq $grandchild -or $child -eq $owner -or $grandchild -eq $owner) {
    throw 'DENIED: invalid or mismatched process identities'
  }
  # SYNCHRONIZE: both native handles must open and be unsignaled while
  # the parent Job owner is alive. These are not PID-only postmortem checks.
  foreach ($observedChildPid in @($child,$grandchild)) {
    $handle=[LuD7KernelWitness]::OpenProcess(0x00100000,$false,$observedChildPid)
    if ($handle -eq [IntPtr]::Zero) {
      throw "DENIED: child $observedChildPid could not be independently opened"
    }
    $children.Add([pscustomobject]@{Pid=$observedChildPid;Handle=$handle})
    if ([LuD7KernelWitness]::WaitForSingleObject($handle,0) -ne 258) {
      throw "DENIED: child $observedChildPid not live before owner crash"
    }
  }
  $crashAt=[DateTime]::UtcNow
  Stop-Process -Id $owner -Force -ErrorAction Stop
  $process.WaitForExit(10000) | Out-Null
  if (!$process.HasExited) { throw 'DENIED: owner not terminated' }
  $results=@()
  foreach ($item in $children) {
    $kernelWait=[LuD7KernelWitness]::WaitForSingleObject($item.Handle,10000)
    if ($kernelWait -ne 0) { throw "DENIED: descendant $($item.Pid) survived owner crash ($kernelWait)" }
    $results+=([pscustomobject]@{pid=$item.Pid;kernelWait='WAIT_OBJECT_0'})
  }
  # A witness is written *after* all OS kernel waits passed; no
  # success receipt is emitted on an unverified or partial termination.
  $receipt=[ordered]@{
    schema='lu.ide.os-job-crash-witness.v1'
    acceptanceFixtureOnly=$true
    ownerPid=$owner
    crashUtc=$crashAt.ToString('o')
    observedUtc=[DateTime]::UtcNow.ToString('o')
    executableSha256=(Get-FileHash -LiteralPath $probe -Algorithm SHA256).Hash.ToLowerInvariant()
    processHandlesOpenedBeforeCrash=$true
    children=$results
    observation='both_descendants_kernel_signaled'
    claim='OS fixture only; no authenticated D7 worker recovery'
  }
  $receipt|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $ReceiptPath -NoNewline -Encoding utf8
  Write-Host 'PASS: independent observer verified parent-crash descendant reaping'
  Write-Host "WITNESS: $ReceiptPath"
} finally {
  foreach ($item in $children) {
    [void][LuD7KernelWitness]::CloseHandle($item.Handle)
  }
  if ($process -and !$process.HasExited) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
  }
  if ($marker) { Remove-Item -LiteralPath $marker -Force -ErrorAction SilentlyContinue }
  if ($stderr) { Remove-Item -LiteralPath $stderr -Force -ErrorAction SilentlyContinue }
}
