#requires -Version 7.2
<#
Lu IDE D7D: explicit, elevated installation of the isolated recovery service.
NO service is installed by compiling the repository or running the IDE.
A release must provide independently approved SHA-256 pins AND Authenticode.
CI's unsigned fixture override only works on hosted GitHub Actions runners.
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$ServiceExecutable,
  [Parameter(Mandatory)][string]$ExpectedServiceSha256,
  [Parameter(Mandatory)][string]$ApprovedHostExecutable,
  [Parameter(Mandatory)][string]$ExpectedHostSha256,
  [switch]$CiAcceptanceUnsigned,
  [switch]$Start
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$serviceName='LuIdeRecoveryBroker'
if ($env:OS -ne 'Windows_NT') { throw 'DENIED: Windows-only service' }
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw 'DENIED: elevated installation is mandatory'
}
foreach ($hash in @($ExpectedServiceSha256,$ExpectedHostSha256)) {
  if ($hash -notmatch '^[0-9a-fA-F]{64}$') { throw 'DENIED: exact externally reviewed hashes required' }
}
$serviceExe=[IO.Path]::GetFullPath($ServiceExecutable)
$hostExe=[IO.Path]::GetFullPath($ApprovedHostExecutable)
foreach ($item in @($serviceExe,$hostExe)) {
  if (!(Test-Path -LiteralPath $item -PathType Leaf)) { throw "DENIED: missing $item" }
  if ((Get-Item -LiteralPath $item -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw 'DENIED: executable reparse point'
  }
}
$observedService=(Get-FileHash -LiteralPath $serviceExe -Algorithm SHA256).Hash
$observedHost=(Get-FileHash -LiteralPath $hostExe -Algorithm SHA256).Hash
if ($observedService -ine $ExpectedServiceSha256 -or $observedHost -ine $ExpectedHostSha256) {
  throw 'DENIED: installed service/host executable did not match reviewed pin'
}
$allowUnsigned=$CiAcceptanceUnsigned -and $env:GITHUB_ACTIONS -eq 'true' -and
    $env:CI -eq 'true' -and ![string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)
if ($CiAcceptanceUnsigned -and !$allowUnsigned) {
  throw 'DENIED: unsigned acceptance only on disposable GitHub Actions runner'
}
if (!$allowUnsigned) {
  foreach ($item in @($serviceExe,$hostExe)) {
    $sig=Get-AuthenticodeSignature -LiteralPath $item
    if ($sig.Status -ne 'Valid') { throw "DENIED: Authenticode invalid for $item" }
  }
}
if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
  throw 'DENIED: service already exists; use an explicit separately reviewed upgrade'
}
$programFiles=[Environment]::GetFolderPath('ProgramFiles')
if (!$allowUnsigned -and (!$serviceExe.StartsWith($programFiles+'\',
  [StringComparison]::OrdinalIgnoreCase) -or
  !$hostExe.StartsWith($programFiles+'\', [StringComparison]::OrdinalIgnoreCase))) {
  throw 'DENIED: release executables must be installed under Program Files'
}
$programData=[Environment]::GetFolderPath('CommonApplicationData')
$root=Join-Path $programData 'LuIDERecoveryBroker'
if (Test-Path -LiteralPath $root) { throw 'DENIED: existing broker evidence directory; do not adopt unknown ACLs' }
$serviceSid='NT SERVICE\LuIdeRecoveryBroker'
$system='NT AUTHORITY\SYSTEM'
$created=$false
try {
  # The account is NOT LocalSystem. The restricted service SID protects
  # its private evidence; interactive Users have no ACL grants.
  # PowerShell's automatic $args must never be used for native command
  # splatting. In the Windows 2022 CI run, sc.exe saw malformed arguments
  # (1639) rather than a CreateServiceW denial.
  # Supply each sc option name and value as separate native argv entries.
  # For Program Files paths, preserve the quotes in the service registry
  # ImagePath; on CI's no-space path, avoid unneeded literal quote layers.
  $binaryPathArgument=if ($serviceExe.Contains(' ')) {
    '"' + $serviceExe + '"'
  } else { $serviceExe }
  $scArgs=@('create',$serviceName,'binPath=',$binaryPathArgument,
    'type=','own','start=','demand','obj=','NT AUTHORITY\LocalService')
  # Preserve the native SCM error code and Win32 message. Hiding this
  # output previously reduced a real Windows 2022 failure to an opaque
  # "SCM service creation failed" even though the pinned build succeeded.
  $scOutput=@(& sc.exe @scArgs 2>&1 | ForEach-Object { [string]$_ })
  $scExit=$LASTEXITCODE
  if ($scExit -ne 0) {
    throw ("DENIED: SCM service creation failed (exit={0}): {1}" -f
      $scExit, ($scOutput -join ' | '))
  }
  $created=$true
  & sc.exe sidtype $serviceName restricted | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: restricted service SID installation failed' }
  & sc.exe privs $serviceName 'SeChangeNotifyPrivilege/SeImpersonatePrivilege' | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: minimal service privilege configuration failed' }
  New-Item -ItemType Directory -Path $root -Force | Out-Null
  # Create and protect config before removing interactive write rights
  # from the parent directory; do not rely on owner privilege to write data.
  $settings=[ordered]@{
    schema='lu.ide.windows.recovery-service.v1'
    serviceName=$serviceName
    expectedServiceSha256=$ExpectedServiceSha256.ToLowerInvariant()
    expectedHostSha256=$ExpectedHostSha256.ToLowerInvariant()
    expectedHostExecutable=$hostExe
    evidenceRoot=$root
    endpoint='\\.\pipe\LuIdeRecoveryBroker.v1'
    acceptanceUnsigned=[bool]$allowUnsigned
  }
  $config=Join-Path $root 'service.json'
  $settings|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $config -Encoding utf8 -NoNewline
  & icacls.exe $config /inheritance:r /grant:r ($system+':F') ($serviceSid+':M') | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: protected config ACL failed' }
  $allowed=@($system,$serviceSid)
  $configAcl=Get-Acl -LiteralPath $config
  if (!$configAcl.AreAccessRulesProtected -or
      @($configAcl.Access|Where-Object {
        $_.AccessControlType -ne 'Allow' -or $_.IdentityReference.Value -notin $allowed
      }).Count) {
    throw 'DENIED: service configuration ACL not private'
  }
  & icacls.exe $root /inheritance:r /grant:r (
      $system+':(OI)(CI)F') ($serviceSid+':(OI)(CI)M') | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: private evidence DACL provisioning failed' }
  $acl=Get-Acl -LiteralPath $root
  if (!$acl.AreAccessRulesProtected) { throw 'DENIED: evidence directory inherits parent ACL' }
  foreach ($entry in $acl.Access) {
    if ($entry.AccessControlType -ne 'Allow' -or $entry.IdentityReference.Value -notin $allowed) {
      throw "DENIED: unexpected evidence ACE: $($entry.IdentityReference.Value)"
    }
  }
  foreach ($name in $allowed) {
    if (!(@($acl.Access|ForEach-Object {$_.IdentityReference.Value}) -contains $name)) {
      throw "DENIED: missing private evidence ACE $name"
    }
  }
  # Restrict service management to SYSTEM and Administrators. This changes
  # the service object DACL, not the named pipe or evidence directory DACLs.
  $scmSddl='D:P(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCLCSWRPWPDTLOCRRC;;;BA)'
  & sc.exe sdset $serviceName $scmSddl | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: service management DACL failed' }
  if ($Start) {
    & sc.exe start $serviceName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'DENIED: restricted recovery service failed to start' }
  }
  Write-Host "SERVICE=$serviceName"
  Write-Host "EVIDENCE_ROOT=$root"
  Write-Host 'ACCEPTANCE=service identity and ACL policy provisioned, native hostile tests still required'
} catch {
  if ($created) { & sc.exe stop $serviceName 2>$null | Out-Null }
  if ($created) { & sc.exe delete $serviceName 2>$null | Out-Null }
  if (Test-Path -LiteralPath $root) {
    # Do not automatically delete evidence after a partial deployment.
    Write-Warning "Partial protected evidence directory left for explicit inspection: $root"
  }
  throw
}
