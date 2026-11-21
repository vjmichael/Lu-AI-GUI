# LU-IPC-01D-R4-R3-R3-R1 -- read-only physical-Windows SCM admission preflight.
# This NEVER installs/reconfigures a service, issues a credential, approves a
# signer, or claims original Job or tool-executor custody. Approval hashes
# MUST come from independently reviewed, signed installer records.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledServiceExe,
  [Parameter(Mandatory=$true)][string]$ApprovedServiceImageSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedServiceSignerCertSha256,
  [Parameter(Mandatory=$true)][string]$ExpectedInstallRoot
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Refuse([string]$Why){throw ('LU_IPC_R5_SCM_REFUSED: '+$Why)}
if($env:OS -ne 'Windows_NT'){Refuse 'Physical/native Windows required'}
function Pin([string]$Value,[string]$Name){
  if($Value -cnotmatch '^[0-9a-f]{64}$' -or $Value -ceq ('0'*64)){
    Refuse ('Missing independent approved '+$Name)
  }
  return $Value
}
$expectedImage=Pin $ApprovedServiceImageSha256 'SCM image SHA256'
$expectedSigner=Pin $ApprovedServiceSignerCertSha256 'SCM signer certificate'
if(-not [IO.Path]::IsPathRooted($InstalledServiceExe) -or
   -not [IO.Path]::IsPathRooted($ExpectedInstallRoot)){
  Refuse 'Service image and install root must be absolute'
}
$exe=[IO.Path]::GetFullPath($InstalledServiceExe)
$root=[IO.Path]::GetFullPath($ExpectedInstallRoot).TrimEnd('\')
if(-not $exe.StartsWith(($root+'\'),[StringComparison]::OrdinalIgnoreCase)){
  Refuse 'SCM service executable is outside the approved install root'
}
if(-not (Test-Path -LiteralPath $exe -PathType Leaf)){
  Refuse 'Approved SCM executable is absent'
}
# Installer-owned ancestors are checked to prevent junction / writable
# directory substitution after a successful Authenticode signature check.
$trusted=@('S-1-5-18','S-1-5-32-544',
  'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
$walk=$exe
while($true){
  $item=Get-Item -LiteralPath $walk -Force
  if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
    Refuse 'Reparse point in installed service image ancestry'
  }
  $acl=Get-Acl -LiteralPath $walk
  $owner=([Security.Principal.NTAccount]::new($acl.Owner)).Translate(
    [Security.Principal.SecurityIdentifier]).Value
  if($trusted -cnotcontains $owner){Refuse 'SCM executable ancestry is not installer-owned'}
  foreach($ace in $acl.GetAccessRules($true,$true,
    [Security.Principal.SecurityIdentifier])){
    $sid=$ace.IdentityReference.Value
    $rights=[uint32]([int64]$ace.FileSystemRights -band 0xffffffffL)
    $writes=[uint32]0x500D0156
    if($ace.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow -and
       (($rights -band $writes) -ne 0) -and $trusted -cnotcontains $sid){
      Refuse 'Untrusted write access in installed SCM image ancestry'
    }
  }
  if($walk.Equals($root,[StringComparison]::OrdinalIgnoreCase)){break}
  $parent=[IO.Directory]::GetParent($walk)
  if($null -eq $parent){Refuse 'Escaped install root'}
  $walk=$parent.FullName.TrimEnd('\')
  if(-not $walk.Equals($root,[StringComparison]::OrdinalIgnoreCase) -and
     -not $walk.StartsWith(($root+'\'),[StringComparison]::OrdinalIgnoreCase)){
    Refuse 'Escaped install root'
  }
}
$actualImage=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
if($actualImage -cne $expectedImage){Refuse 'Image hash differs from independent pin'}
$signature=Get-AuthenticodeSignature -LiteralPath $exe
if($null -eq $signature -or
   $signature.Status -ne [Management.Automation.SignatureStatus]::Valid -or
   $null -eq $signature.SignerCertificate -or
   $signature.SignatureType -ne [Management.Automation.SignatureType]::Authenticode){
  Refuse 'Installed SCM image has no trusted embedded Authenticode signature'
}
$hash=[Security.Cryptography.SHA256]::Create()
try{
  $actualSigner=([BitConverter]::ToString(
    $hash.ComputeHash($signature.SignerCertificate.RawData))).Replace('-','').ToLowerInvariant()
}finally{$hash.Dispose()}
if($actualSigner -cne $expectedSigner){Refuse 'SCM signing cert differs from approval'}
# SCM process observations are independent of the installed binary's claims.
$service=Get-CimInstance -ClassName Win32_Service -Filter "Name='LuCoreIpcSupervisor'"
if($null -eq $service -or $service.State -cne 'Running' -or
   [uint32]$service.ProcessId -eq 0 -or
   $service.StartMode -cne 'Auto' -or
   $service.ServiceType -notlike '*Own Process*' -or
   $service.ServiceType -like '*Share Process*'){
  Refuse 'Approved dedicated automatic SCM service is not running'
}
if($service.StartName -notin @('LocalSystem','NT AUTHORITY\LocalSystem')){
  Refuse 'Installed SCM service is not configured as LocalSystem'
}
if($service.PathName -cne $exe -and $service.PathName -cne ('"'+$exe+'"')){
  Refuse 'SCM ImagePath differs from approved executable'
}
$pidValue=[uint32]$service.ProcessId
$observed=Get-CimInstance -ClassName Win32_Process -Filter ("ProcessId="+$pidValue)
if($null -eq $observed -or
   -not $observed.ExecutablePath.Equals($exe,[StringComparison]::OrdinalIgnoreCase) -or
   $null -eq $observed.CreationDate){
  Refuse 'Service process image or creation epoch cannot be verified'
}
$owner=Invoke-CimMethod -InputObject $observed -MethodName GetOwnerSid
if($owner.ReturnValue -ne 0 -or $owner.Sid -cne 'S-1-5-18'){
  Refuse 'Live SCM process has no independent LocalSystem token witness'
}
$recheck=Get-CimInstance -ClassName Win32_Service -Filter "Name='LuCoreIpcSupervisor'"
$processAgain=Get-CimInstance -ClassName Win32_Process -Filter ("ProcessId="+$pidValue)
if($null -eq $recheck -or $null -eq $processAgain -or
   $recheck.State -cne 'Running' -or
   [uint32]$recheck.ProcessId -ne $pidValue -or
   $processAgain.CreationDate -ne $observed.CreationDate){
  Refuse 'SCM process generation changed during inspection'
}
Write-Output 'LU_IPC_R5_SCM_SIGNED_RUNNING_PREFLIGHT_PASS'
Write-Output 'This does not verify retained worker Jobs, SYSTEM broker credential rotation, restart receipts or physical execution acceptance'
