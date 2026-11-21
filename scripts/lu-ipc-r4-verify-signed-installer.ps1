# LU-IPC-01D-R4 — independent publisher admission for an already built
# signed installer AND its installed signed Lu IDE DLC.
#
# This script DOES NOT build, sign, install, trust or approve an artifact.
# Source-controlled code and the installed binary cannot approve themselves.
# It rejects missing/unapproved/unsigned installers before delegating to
# R3's protected installed Lu IDE enrollment preflight. -Apply requires a
# separately authorized elevated installer, and R3 refuses overwrite.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$SignedInstaller,
  [Parameter(Mandatory=$true)][string]$ApprovedInstallerSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedInstallerSignerCertificateSha256,
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][string]$ExpectedInstallRoot,
  [Parameter(Mandatory=$true)][string]$ApprovedImageSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedSignerCertificateSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedPackageSha256,
  [Parameter(Mandatory=$true)][string]$PackageManifestPath,
  [switch]$Apply
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Refuse([string]$Why){ throw ('LU_IPC_R4_REFUSED: '+$Why) }
if($env:OS -ne 'Windows_NT'){Refuse 'native Windows acceptance required'}
function ExactHash([string]$Hash,[string]$Name){
  if($Hash -cnotmatch '^[0-9a-f]{64}$' -or $Hash -ceq ('0'*64)){
    Refuse "Missing independent approved $Name"
  }
  return $Hash
}
$expected=ExactHash $ApprovedInstallerSha256 'installer file SHA256'
$cert=ExactHash $ApprovedInstallerSignerCertificateSha256 'installer publisher certificate SHA256'
$source=ExactHash $ApprovedImageSha256 'Lu IDE image SHA256'
$ideSigner=ExactHash $ApprovedSignerCertificateSha256 'Lu IDE certificate SHA256'
$package=ExactHash $ApprovedPackageSha256 'Lu IDE package SHA256'
if(-not [IO.Path]::IsPathRooted($SignedInstaller)){
  Refuse 'Installer path must be fully qualified'
}
$installer=[IO.Path]::GetFullPath($SignedInstaller)
if(-not (Test-Path -LiteralPath $installer -PathType Leaf)){
  Refuse 'Approved signed installer executable missing'
}
$file=Get-Item -LiteralPath $installer -Force
if(($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
  Refuse 'Installer path may not be a link/reparse point'
}
$actual=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
if($actual -cne $expected){ Refuse 'Installer image differs from independent approval' }
$signature=Get-AuthenticodeSignature -LiteralPath $installer
if($null -eq $signature -or
   $signature.Status -ne [Management.Automation.SignatureStatus]::Valid -or
   $null -eq $signature.SignerCertificate -or
   $signature.SignatureType -ne [Management.Automation.SignatureType]::Authenticode){
  Refuse 'Installer is not authentically embedded-signed and trusted'
}
$hasher=[Security.Cryptography.SHA256]::Create()
try{
  $actualCert=([BitConverter]::ToString(
    $hasher.ComputeHash($signature.SignerCertificate.RawData))).Replace('-','').ToLowerInvariant()
}finally{$hasher.Dispose()}
if($actualCert -cne $cert){Refuse 'Installer certificate differs from independent approval'}
# Bind the package identity to the exact installed manifest bytes rather
# than accepting an arbitrary 64-digit claim. It must be a protected file
# within the same approved Lu IDE Program Files installation root.
if(-not [IO.Path]::IsPathRooted($PackageManifestPath) -or
   -not [IO.Path]::IsPathRooted($ExpectedInstallRoot)){
  Refuse 'Manifest and install root must have absolute paths'
}
$manifest=[IO.Path]::GetFullPath($PackageManifestPath)
$root=[IO.Path]::GetFullPath($ExpectedInstallRoot).TrimEnd('\')
if(-not $manifest.StartsWith(($root+'\'),[StringComparison]::OrdinalIgnoreCase)){
  Refuse 'Installed package manifest is outside the approved installation root'
}
if(-not (Test-Path -LiteralPath $manifest -PathType Leaf)){
  Refuse 'Approved installed package manifest missing'
}
# Verify EVERY path component beneath the approved installation root,
# not just the manifest leaf: a user-writable child directory or junction
# would otherwise permit a protected-looking manifest to be substituted.
$installers=@('S-1-5-18','S-1-5-32-544',
  'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
$current=$manifest
while($true){
  $item=Get-Item -LiteralPath $current -Force
  if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
    Refuse 'Reparse/junction in installed manifest ancestry'
  }
  $acl=Get-Acl -LiteralPath $current
  $owner=([Security.Principal.NTAccount]::new($acl.Owner)).Translate(
    [Security.Principal.SecurityIdentifier]).Value
  if($installers -cnotcontains $owner){
    Refuse 'Installed manifest ancestry is not installer owned'
  }
  foreach($ace in $acl.GetAccessRules($true,$true,
    [Security.Principal.SecurityIdentifier])){
    $sid=$ace.IdentityReference.Value
    $rights=[uint32]([int64]$ace.FileSystemRights -band 0xffffffffL)
    $writeMask=[uint32]0x500D0156
    if($ace.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow -and
       (($rights -band $writeMask) -ne 0) -and
       $installers -cnotcontains $sid){
      Refuse 'Writable installed manifest ancestor cannot authorize DLC'
    }
  }
  if($current.Equals($root,[StringComparison]::OrdinalIgnoreCase)){break}
  $parent=[IO.Directory]::GetParent($current)
  if($null -eq $parent){Refuse 'Manifest escaped approved install root'}
  $current=$parent.FullName.TrimEnd('\')
  if(-not $current.Equals($root,[StringComparison]::OrdinalIgnoreCase) -and
     -not $current.StartsWith(($root+'\'),[StringComparison]::OrdinalIgnoreCase)){
    Refuse 'Manifest escaped approved install root'
  }
}
$actualPackage=(Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash.ToLowerInvariant()
if($actualPackage -cne $package){Refuse 'Installed package manifest digest differs from approval'}
# This verifies an installed state; never executes the installer or treats its
# mere signature as proof that it installed the approved Lu IDE payload.
$preflight=Join-Path $PSScriptRoot 'lu-ipc-r3-stage-installed-ide.ps1'
$arguments=@{
  InstalledExe=$InstalledExe
  ExpectedInstallRoot=$ExpectedInstallRoot
  ApprovedImageSha256=$source
  ApprovedSignerCertificateSha256=$ideSigner
  ApprovedPackageSha256=$package
}
if($Apply){
  $arguments.Apply=$true
}
& $preflight @arguments
Write-Output 'LU_IPC_R4_SIGNED_INSTALLER_AND_IDE_PREFLIGHT_PASS'
Write-Output 'This is not a physical installed Lu IDE proof or broker-session acceptance receipt'
