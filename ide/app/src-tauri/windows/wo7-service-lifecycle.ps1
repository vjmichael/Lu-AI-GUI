# WO7-C protected LocalService lifecycle. Installed NSIS per-machine ONLY.
# This is not an administrative repair tool for arbitrary services or paths.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidateSet('Install','Remove','Inspect')][string]$Operation,
  [Parameter(Mandatory=$true)][string]$InstallRoot
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$Name='LuIdeRecoveryBroker'
$ExeRel='resources\recovery\windows_recovery_service.exe'
$PolicyRel='resources\wo7-service-policy.json'
$Sc=Join-Path $env:SystemRoot 'System32\sc.exe'

function Deny([string]$reason) { throw "DENIED: WO7 protected service: $reason" }
function Sc([string[]]$Arguments) {
  $output=& $Sc @Arguments 2>&1
  if ($LASTEXITCODE -ne 0) { Deny ("sc_" + $Arguments[0] + "_failed") }
  return $output
}
function Confirm-Administrator {
  $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
  $principal=New-Object Security.Principal.WindowsPrincipal($identity)
  if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Deny 'elevated_per_machine_installer_required'
  }
}
function Assert-Immutable([string]$path) {
  $current=[IO.Path]::GetFullPath($path)
  while ($current -and (Test-Path -LiteralPath $current)) {
    $item=Get-Item -LiteralPath $current -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
      Deny 'reparse_point_in_installation'
    }
    $parent=[IO.Directory]::GetParent($current)
    if ($null -eq $parent) { break }
    $current=$parent.FullName
  }
}
function Check-ProtectedAcl([string]$path) {
  $acl=Get-Acl -LiteralPath $path
  foreach ($ace in $acl.Access) {
    if ($ace.AccessControlType -ne 'Allow') { continue }
    $sid=$ace.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($sid -notin @('S-1-1-0','S-1-5-11','S-1-5-32-545')) { continue }
    $write=[Security.AccessControl.FileSystemRights]::Write -bor
      [Security.AccessControl.FileSystemRights]::Modify -bor
      [Security.AccessControl.FileSystemRights]::FullControl -bor
      [Security.AccessControl.FileSystemRights]::Delete
    if (($ace.FileSystemRights -band $write) -ne 0) {
      Deny 'standard_users_can_modify_installation'
    }
  }
}
function Get-TrustedPolicy([string]$root) {
  $policyFile=Join-Path $root $PolicyRel
  if (!(Test-Path -LiteralPath $policyFile -PathType Leaf)) { Deny 'release_policy_missing' }
  Assert-Immutable $policyFile
  $policy=Get-Content -LiteralPath $policyFile -Raw | ConvertFrom-Json
  if ($policy.schema -ne 'lu.ide.wo7.service-policy.v1' -or $policy.releaseApproved -ne $true) {
    Deny 'release_policy_unapproved'
  }
  if ($policy.serviceSha256 -notmatch '^[a-fA-F0-9]{64}$' -or
      $policy.signerThumbprint -notmatch '^[a-fA-F0-9]{40}$') {
    Deny 'release_policy_pin_missing'
  }
  return $policy
}
function Confirm-TrustedService([string]$root, $policy) {
  $exe=Join-Path $root $ExeRel
  if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { Deny 'service_binary_missing' }
  Assert-Immutable $exe
  foreach ($directory in @($root,(Join-Path $root 'resources'),(Join-Path $root 'resources\recovery'))) {
    Check-ProtectedAcl $directory
  }
  Check-ProtectedAcl $exe
  $hash=(Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash
  if ($hash -ine $policy.serviceSha256) { Deny 'service_digest_mismatch' }
  $signature=Get-AuthenticodeSignature -LiteralPath $exe
  if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate -or
      $signature.SignerCertificate.Thumbprint -ine $policy.signerThumbprint) {
    Deny 'service_signature_not_trusted'
  }
  return $exe
}
function CurrentService {
  return Get-CimInstance -ClassName Win32_Service -Filter "Name='LuIdeRecoveryBroker'" -ErrorAction Stop
}
function Confirm-OurService($service,[string]$exe) {
  if ($null -eq $service) { Deny 'protected_service_absent' }
  if ($service.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE')) {
    Deny 'service_principal_changed'
  }
  # SCM Win32_Service.PathName may add enclosing quotes, not arguments.
  if ($service.PathName.Trim('"') -ine $exe) { Deny 'service_image_path_mismatch' }
  if ($service.ServiceType -ne 'Own Process') { Deny 'service_type_changed' }
}
try {
  if ($env:OS -ne 'Windows_NT') { Deny 'windows_required' }
  Confirm-Administrator
  $root=[IO.Path]::GetFullPath($InstallRoot).TrimEnd('\')
  $accepted=@(
    (Join-Path $env:ProgramFiles 'Lu IDE')
  )
  $programFilesX86=[Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
  if ($programFilesX86) { $accepted+=Join-Path $programFilesX86 'Lu IDE' }
  if (!($accepted | Where-Object { $root -ieq [IO.Path]::GetFullPath($_).TrimEnd('\') })) {
    Deny 'only_fixed_program_files_lu_ide_installation_allowed'
  }
  Assert-Immutable $root
  $policy=Get-TrustedPolicy $root
  $serviceExe=Confirm-TrustedService $root $policy
  $existing=CurrentService
  switch ($Operation) {
    'Install' {
      if ($null -ne $existing) { Deny 'existing_service_requires_explicit_upgrade' }
      $quotedImage='"' + $serviceExe + '"'
      try {
        Sc @('create',$Name,('binPath= ' + $quotedImage),'start= demand',
              'type= own','obj= NT AUTHORITY\LocalService','DisplayName= Lu IDE Protected Recovery Broker') | Out-Null
        Sc @('sidtype',$Name,'restricted') | Out-Null
        # SYSTEM and Administrators can manage service. Never grant Everyone
        # or AppContainer wildcard control of a protected recovery service.
        Sc @('sdset',$Name,'D:(A;;CCDCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRRC;;;BA)') | Out-Null
        Confirm-OurService (CurrentService) $serviceExe
        Write-Output 'WO7_PROTECTED_SERVICE_INSTALLED_AND_IDENTITY_VERIFIED'
      } catch {
        # Fail closed and attempt rollback; do not suppress the original error.
        & $Sc delete $Name | Out-Null
        throw
      }
    }
    'Inspect' {
      Confirm-OurService $existing $serviceExe
      Write-Output 'WO7_PROTECTED_SERVICE_IDENTITY_VERIFIED'
    }
    'Remove' {
      Confirm-OurService $existing $serviceExe
      Sc @('stop',$Name) | Out-Null
      $deadline=(Get-Date).AddSeconds(25)
      do {
        Start-Sleep -Milliseconds 400
        $current=CurrentService
        if ($null -ne $current -and $current.State -eq 'Stopped') { break }
      } while ((Get-Date) -lt $deadline)
      if ($null -eq $current -or $current.State -ne 'Stopped') { Deny 'service_not_stopped' }
      Sc @('delete',$Name) | Out-Null
      $deadline=(Get-Date).AddSeconds(20)
      do { Start-Sleep -Milliseconds 400; $current=CurrentService }
      while ($null -ne $current -and (Get-Date) -lt $deadline)
      if ($null -ne $current) { Deny 'service_removal_not_observed' }
      Write-Output 'WO7_PROTECTED_SERVICE_REMOVED'
    }
  }
} catch {
  Write-Error "WO7_SERVICE_LIFECYCLE_DENIED"
  exit 1
}
