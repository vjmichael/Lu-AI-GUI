# LU-IPC-01D-R3 — native Windows Lu IDE installed-publisher enrollment.
# This requires an ALREADY INSTALLED, authentic Authenticode-signed Lu IDE
# executable and independent, explicitly approved SHA-256 identity pins.
# Never derives an approval pin from the untrusted installed executable.
# NO code-signing private key, IPC credential or claimed manifest is stored.
#
# Default = VERIFY ONLY. -Apply requires elevated trusted installer context
# and creates ONE protected ProgramData enrollment. No replacement, migration,
# network download, silent approval, self-sign, test surrogate or fallback.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][string]$ExpectedInstallRoot,
  [Parameter(Mandatory=$true)][string]$ApprovedImageSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedSignerCertificateSha256,
  [Parameter(Mandatory=$true)][string]$ApprovedPackageSha256,
  [switch]$Apply
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Windows host required for native installed package proof'}

function Refuse([string]$Reason){throw ("LU_IPC_R3_REFUSED: "+$Reason)}
function Canonical([string]$Path){
  if(-not [IO.Path]::IsPathRooted($Path)){Refuse 'Relative file or installation root'}
  if($Path.IndexOf([char]0) -ge 0){Refuse 'NUL in path'}
  return [IO.Path]::GetFullPath($Path).TrimEnd('\')
}
function ApprovedHash([string]$Value,[string]$Purpose){
  if($Value -cnotmatch '^[0-9a-f]{64}$' -or $Value -ceq ('0'*64)){
    Refuse "Missing or invalid independent SHA256: $Purpose"
  }
  return $Value
}
function NativeSid([string]$Account){
  return ([Security.Principal.NTAccount]::new($Account)).Translate(
    [Security.Principal.SecurityIdentifier]).Value
}
function InspectPath([string]$Path,[bool]$MustBeInstallerOnly){
  $item=Get-Item -LiteralPath $Path -Force -ErrorAction Stop
  if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
    Refuse "Reparse-point/Junction in installed path: $Path"
  }
  $security=Get-Acl -LiteralPath $Path -ErrorAction Stop
  $allowOwner=@('S-1-5-18','S-1-5-32-544',
    'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464') # TrustedInstaller
  $owner=NativeSid $security.Owner
  if($allowOwner -cnotcontains $owner){
    Refuse "Untrusted installed path owner: $Path"
  }
  if($MustBeInstallerOnly -and -not $security.AreAccessRulesProtected){
    Refuse "Installer enrollment DACL inherits permissions: $Path"
  }
  $system=$false
  $admins=$false
  $rules=$security.GetAccessRules($true,$true,
    [Security.Principal.SecurityIdentifier])
  foreach($rule in $rules){
    $sid=$rule.IdentityReference.Value
    $mask=[uint32]([int64]$rule.FileSystemRights -band 0xffffffffL)
    # Rights permitting writing, deletion, DACL changes or takeover. Generic
    # all/write bits and FILE-specific bits are included. Do not allow
    # broad inherited or indirect modification rights to non-custodians.
    $dangerous=[uint32]0x500D0156
    $canWrite=(($mask -band $dangerous) -ne 0)
    if($rule.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow){
      if($MustBeInstallerOnly){
        if($rule.IsInherited -or $sid -cnotin @('S-1-5-18','S-1-5-32-544')){
          Refuse "Untrusted enrollment ACE: $Path"
        }
        if($sid -ceq 'S-1-5-18'){$system=$true}
        if($sid -ceq 'S-1-5-32-544'){$admins=$true}
      } elseif($canWrite -and $allowOwner -cnotcontains $sid){
        Refuse "Non-custodian has write access to installed image path: $Path"
      }
    } elseif($MustBeInstallerOnly){
      Refuse "Unexpected installer enrollment ACL type: $Path"
    }
  }
  if($MustBeInstallerOnly -and -not ($system -and $admins)){
    Refuse "Incomplete installer ACL: $Path"
  }
}
function VerifySourceTree([string]$Image,[string]$Root,[string]$ProgramFiles){
  $programPrefix=$ProgramFiles+'\'
  $rootPrefix=$Root+'\'
  if(-not $Root.StartsWith($programPrefix,[StringComparison]::OrdinalIgnoreCase)){
    Refuse 'Installed Lu IDE root must be beneath Program Files'
  }
  if(-not $Image.StartsWith($rootPrefix,[StringComparison]::OrdinalIgnoreCase)){
    Refuse 'Signed Lu IDE executable is outside approved install root'
  }
  # Do not silently accept junctions or user-writable ancestors.
  $current=$Image
  while($true){
    InspectPath $current $false
    if($current.Equals($ProgramFiles,[StringComparison]::OrdinalIgnoreCase)){break}
    $parent=[IO.Directory]::GetParent($current)
    if($null -eq $parent){Refuse 'Unexpected installation path root'}
    $current=$parent.FullName.TrimEnd('\')
  }
}
function SecureAcl(){
  # Explicit SYSTEM + built-in Administrators, no inherited ACEs, owner BA.
  return 'O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)'
}
function EnsureOwnedDirectory([string]$Path){
  if(Test-Path -LiteralPath $Path){
    InspectPath $Path $true
    if(-not (Get-Item -LiteralPath $Path).PSIsContainer){Refuse "Not directory: $Path"}
    return
  }
  $sd=[Security.AccessControl.DirectorySecurity]::new()
  $sd.SetSecurityDescriptorSddlForm((SecureAcl))
  # This overload supplies security at creation, not after an unprotected
  # time window in a broadly writable ProgramData parent directory.
  [IO.Directory]::CreateDirectory($Path,$sd)|Out-Null
  InspectPath $Path $true
}
$exe=Canonical $InstalledExe
$root=Canonical $ExpectedInstallRoot
$exeHash=ApprovedHash $ApprovedImageSha256 'Lu IDE executable'
$certHash=ApprovedHash $ApprovedSignerCertificateSha256 'Lu IDE publisher certificate'
$pkgHash=ApprovedHash $ApprovedPackageSha256 'approved manifest'
if(-not (Test-Path -LiteralPath $exe -PathType Leaf)){
  Refuse 'Installed signed Lu IDE executable missing'
}
$programFilesValue=(Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion').ProgramFilesDir
$programFiles=Canonical $programFilesValue
VerifySourceTree $exe $root $programFiles
$signature=Get-AuthenticodeSignature -LiteralPath $exe
if($null -eq $signature -or
  $signature.Status -ne [Management.Automation.SignatureStatus]::Valid -or
  $null -eq $signature.SignerCertificate -or
  $signature.SignatureType -ne [Management.Automation.SignatureType]::Authenticode){
  Refuse 'Installed Lu IDE has no valid embedded Authenticode publisher signature'
}
$actualFileHash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
if($actualFileHash -cne $exeHash){Refuse 'Installed image digest differs from approval'}
$sha=[Security.Cryptography.SHA256]::Create()
try{
  $actualSignerHash=([BitConverter]::ToString(
    $sha.ComputeHash($signature.SignerCertificate.RawData))).Replace('-','').ToLowerInvariant()
}finally{$sha.Dispose()}
if($actualSignerHash -cne $certHash){Refuse 'Signer DER SHA256 differs from approved publisher'}
# Stable full path must equal the installer's expected canonical identity.
if(-not $exe.Equals((Canonical $signature.Path),
    [StringComparison]::OrdinalIgnoreCase)){
  Refuse 'Signature API resolved another file'
}
if(-not $Apply){
  Write-Output 'LU_IPC_R3_PREFLIGHT_PASS: genuine installed signed executable, independent publisher and image pins, protected location'
  Write-Output 'No enrollment was created, no credential was read or generated, and no Lu Core permission was granted'
  return
}
$who=[Security.Principal.WindowsPrincipal]::new(
  [Security.Principal.WindowsIdentity]::GetCurrent())
if(-not $who.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
  Refuse 'Creating protected enrollment requires elevated installer context'
}
$programData=Canonical $env:ProgramData
$lu=Join-Path $programData 'Lu'
$ipc=Join-Path $lu 'IPC'
$folder=Join-Path $ipc 'enrollments'
EnsureOwnedDirectory $lu
EnsureOwnedDirectory $ipc
EnsureOwnedDirectory $folder
$destination=Join-Path $folder 'lu.ide.json'
if(Test-Path -LiteralPath $destination){
  Refuse 'Enrollment already installed: refuse overwrite/rollback; use reviewed upgrade protocol'
}
$record=[ordered]@{
  schema='lu.win.dlc-enrollment.v1'
  module_id='lu.ide'
  role=1
  installed_executable_path=$exe
  image_sha256=$exeHash
  signer_certificate_sha256=$certHash
  package_sha256=$pkgHash
}
$json=($record | ConvertTo-Json -Compress -Depth 3)
$staging=Join-Path $folder ('.lu.ide.'+[guid]::NewGuid().ToString('N')+'.tmp')
try{
  # Parent is protected at creation, and enrollment file explicitly has
  # SYSTEM/BA-only DACL before the atomic no-overwrite rename.
  $encoded=New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($staging,$json,$encoded)
  $fs=[Security.AccessControl.FileSecurity]::new()
  $fs.SetSecurityDescriptorSddlForm((SecureAcl))
  Set-Acl -LiteralPath $staging -AclObject $fs
  InspectPath $staging $true
  [IO.File]::Move($staging,$destination) # fails if destination already exists
  InspectPath $destination $true
}finally{
  if(Test-Path -LiteralPath $staging){Remove-Item -LiteralPath $staging -Force}
}
Write-Output 'LU_IPC_R3_PROTECTED_RECORD_CREATED: lu.ide'
Write-Output 'No HMAC secret was written. A separately authorized OS credential broker and signed Lu IDE runtime are still required'
