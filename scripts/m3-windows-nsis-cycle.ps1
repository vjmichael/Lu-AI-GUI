# M3 NSIS cycle: genuine installer/uninstaller execution on a disposable
# Windows account/VM. Not a UI automation or GGUF inference acceptance.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedLuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedEngineSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9._-]+\.exe$')][string]$UninstallerName,
  [switch]$DisposableWindowsConfirmed,
  [ValidateRange(30,1800)][int]$TimeoutSeconds=480,
  [string]$EvidenceDir=(Join-Path $env:TEMP ('Lu-M3-nsis-'+[guid]::NewGuid().ToString('N')))
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
  throw 'A clean disposable native Windows environment and explicit consent are mandatory'
}
if($UninstallerName -in @('Lu.exe','lu-desktop.exe','llama-server.exe')){
  throw 'Refuse unsafe uninstaller identity'
}
function Verified-Exe([string]$path,[string]$hash){
  if(-not [IO.Path]::IsPathRooted($path)){throw 'Expected reviewed absolute EXE path'}
  $file=Get-Item -LiteralPath $path -Force -ErrorAction Stop
  if($file.PSIsContainer -or $file.Extension -ine '.exe' -or
    (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'Expected ordinary executable, no symlink or reparse'
  }
  if((Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -ine $hash){
    throw 'Reviewed executable SHA-256 mismatch'
  }
  return $file.FullName
}
$installer=Verified-Exe $InstallerPath $InstallerSha256
if(Test-Path -LiteralPath $EvidenceDir){throw 'Evidence output must be a new directory'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$target=Join-Path $env:TEMP ('Lu-M3-disposable-'+[guid]::NewGuid().ToString('N'))
$record=[ordered]@{
  schema='lu-m3-physical-nsis-v1'
  installerSha256=$InstallerSha256.ToLowerInvariant()
  luSha256=$ExpectedLuSha256.ToLowerInvariant()
  engineSha256=$ExpectedEngineSha256.ToLowerInvariant()
  windowsEnvironment='OPERATOR_CONFIRMED_DISPOSABLE'
  installer='NOT_TESTED'
  installedApp='NOT_TESTED'
  installedRealEngine='NOT_TESTED'
  uninstaller='NOT_TESTED'
  packageRemoval='NOT_TESTED'
  activeModelWorkersReaped='NOT_TESTED'
  webViewUiAndModelInference='NOT_TESTED'
  installationStateUncertain=$false
  profilePreservation='NOT_TESTED'
  versionedRollback='NOT_TESTED_REQUIRES_TWO_DISTINCT_INSTALLERS'
  failureStage=$null
  errors=@()
}
$stage='preflight'
function Invoke-Reviewed-Child([string]$path,[string[]]$arguments) {
  $p=Start-Process -FilePath $path -ArgumentList $arguments -PassThru -ErrorAction Stop
  if(-not $p.WaitForExit($TimeoutSeconds*1000)){
    try{$p.Kill($true);$null=$p.WaitForExit(10000)}catch{}
    throw 'Reviewed installer process exceeded bounded timeout; installation state uncertain'
  }
  $p.Refresh()
  if($p.ExitCode -ne 0){throw ('Reviewed NSIS exit code '+$p.ExitCode)}
}
try{
  $existing=@(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue)
  if($existing.Count -gt 0){throw 'Another Lu instance is running; refuse collision'}
  if(Test-Path -LiteralPath $target){throw 'Disposable target unexpectedly exists'}
  # New isolated profile sentinel: the uninstaller must NEVER erase creator
  # SQLite/models in AppData. This does not launch GGUF or prove actual
  # historical profile migrations. Refuse an existing profile in CI.
  $profile=Join-Path $env:APPDATA 'org.lu.agent01g.devtest'
  if(Test-Path -LiteralPath $profile){throw 'Refuse preexisting test profile'}
  $null=New-Item -ItemType Directory -Path $profile
  $marker=Join-Path $profile 'lifecycle-retain-this-data.txt'
  $nonce=[guid]::NewGuid().ToString('N')
  [IO.File]::WriteAllText($marker,$nonce)
  $markerSha=(Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash
  $stage='install'
  Invoke-Reviewed-Child $installer @('/S',('/D='+$target))
  $record.installer='PASS_REAL_NATIVE_PROCESS_EXIT'
  $app=$null
  foreach($name in @('Lu.exe','lu-desktop.exe')){
    $candidate=Join-Path $target $name
    if(Test-Path -LiteralPath $candidate -PathType Leaf){
      $app=Verified-Exe $candidate $ExpectedLuSha256
      break
    }
  }
  if(-not $app){throw 'Reviewed installed Lu executable was not placed in disposable directory'}
  $record.installedApp='PASS_SHA256'
  $engine=Verified-Exe (Join-Path $target 'lu-inference\llama-server.exe') $ExpectedEngineSha256
  $record.installedRealEngine='PASS_SHA256'
  $stage='uninstall'
  $uninstaller=Join-Path $target $UninstallerName
  if(-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)){
    throw 'Expected actual installed uninstaller is absent'
  }
  if($uninstaller -ieq $app -or $uninstaller -ieq $engine){
    throw 'Uninstaller aliases a reviewed installed binary'
  }
  $u=Get-Item -LiteralPath $uninstaller -Force
  if(($u.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){
    throw 'Uninstaller is a reparse point'
  }
  Invoke-Reviewed-Child $uninstaller @('/S')
  $record.uninstaller='PASS_REAL_NATIVE_PROCESS_EXIT'
  if((Test-Path -LiteralPath $app) -or (Test-Path -LiteralPath $engine) -or
    @(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue).Count -gt 0){
    throw 'Installed program or live Lu process survived uninstall'
  }
  $record.packageRemoval='PASS_BINARIES_ABSENT'
  if(-not(Test-Path -LiteralPath $marker -PathType Leaf) -or
    (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash -cne $markerSha){
    throw 'NSIS uninstall altered isolated AppData sentinel'
  }
  $record.profilePreservation='PASS_UNTOUCHED_APPDATA_SENTINEL'
  $record.activeModelWorkersReaped='INCONCLUSIVE_NO_MODEL_LAUNCH_IN_THIS_CYCLE'
}catch{
  $record.failureStage=$stage
  $record.errors=@($record.errors)+@($_.Exception.Message)
  $record.installationStateUncertain=($stage -ne 'preflight')
  Write-Warning ('Real M3 NSIS cycle is NOT ACCEPTED: '+$_.Exception.Message)
}finally{
  $receipt=Join-Path $EvidenceDir 'm3-nsis-cycle.json'
  $record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $receipt -Encoding UTF8
  Write-Output ('Genuine NSIS lifecycle receipt: '+$receipt)
  Write-Output 'No cleanup of uncertain installation state is attempted'
}
if($record.packageRemoval -ne 'PASS_BINARIES_ABSENT'){exit 2}
