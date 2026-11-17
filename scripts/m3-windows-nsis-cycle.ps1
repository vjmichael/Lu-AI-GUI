# M3 NSIS cycle: genuine installer/uninstaller execution on a disposable
# Windows account/VM. Not a UI automation or GGUF inference acceptance.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedLuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedEngineSha256,
  [string]$ExpectedVulkanSha256='',
  [string]$ExpectedResearchSha256='',
  [string]$ExpectedBrowserManifestSha256='',
  [string]$ProfileIdentifier='org.lu.agent01g.devtest',
  [Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9._-]+\.exe$')][string]$UninstallerName,
  [switch]$DisposableWindowsConfirmed,
  [switch]$ExerciseInstalledGui,
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
  vulkanEngineSha256=$ExpectedVulkanSha256
  installedVulkanEngine='NOT_TESTED'
  installedResearchClient='NOT_TESTED'
  installedReviewedBrowser='NOT_TESTED'
  windowsEnvironment='OPERATOR_CONFIRMED_DISPOSABLE'
  installer='NOT_TESTED'
  installedApp='NOT_TESTED'
  installedRealEngine='NOT_TESTED'
  uninstaller='NOT_TESTED'
  packageRemoval='NOT_TESTED'
  activeModelWorkersReaped='NOT_TESTED'
  webViewUiAndModelInference='NOT_TESTED'
  installedUiSmoke='NOT_TESTED'
  installationStateUncertain=$false
  profilePreservation='NOT_TESTED'
  removalObservedAfterMs=0
  postUninstallResidue=@()
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
  if($ProfileIdentifier -cnotmatch '^org\.lu\.[A-Za-z0-9._-]+$'){throw 'Invalid isolated Lu profile identity'}
  $profile=Join-Path $env:APPDATA $ProfileIdentifier
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
  $vulkanEngine=$null
  if($ExpectedVulkanSha256){
    if($ExpectedVulkanSha256 -notmatch '^[0-9a-fA-F]{64}$'){throw 'Invalid reviewed Vulkan SHA256'}
    $vulkanEngine=Verified-Exe (Join-Path $target 'lu-inference\llama-server-vulkan.exe') $ExpectedVulkanSha256
    if($vulkanEngine -ieq $engine){throw 'CPU/Vulkan paths must be distinct'}
    $record.installedVulkanEngine='PASS_SHA256'
  }
  $researchExe=$null
  if($ExpectedResearchSha256){
    if($ExpectedResearchSha256 -notmatch '^[0-9a-fA-F]{64}$'){throw 'Invalid signed research client digest'}
    $researchExe=Verified-Exe (Join-Path $target 'lu-research\curl.exe') $ExpectedResearchSha256
    $record.installedResearchClient='PASS_SHA256'
  }
  if($ExpectedBrowserManifestSha256){
    $stage='installed-reviewed-browser'
    if($ExpectedBrowserManifestSha256 -cnotmatch '^[0-9a-fA-F]{64}$'){throw 'Invalid browser SHA'}
    $root=Join-Path $target 'lu-browser'
    $r=Get-Item -LiteralPath $root -Force -ErrorAction Stop
    if(-not $r.PSIsContainer -or ($r.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Untrusted browser directory'}
    $mp=Join-Path $root 'manifest.json'
    $mi=Get-Item -LiteralPath $mp -Force -ErrorAction Stop
    if($mi.PSIsContainer -or ($mi.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Untrusted browser manifest'}
    if((Get-FileHash -LiteralPath $mp -Algorithm SHA256).Hash.ToLowerInvariant() -cne $ExpectedBrowserManifestSha256.ToLowerInvariant()){
      throw 'Installed Browser manifest differs from independently approved SHA'
    }
    $m=Get-Content -LiteralPath $mp -Raw|ConvertFrom-Json
    if($m.schema -ne 1 -or $m.runtime -cne 'lu-browser' -or $m.status -cne 'reviewed' -or
       $m.platform -cne 'win32' -or $m.arch -cne 'x64' -or
       $m.playwrightVersion -cne '1.64.1' -or @($m.files).Count -lt 100){
      throw 'Invalid reviewed browser metadata'
    }
    $seen=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    # Windows can return a physical installed file with path casing/formatting
    # different from the manifest's slash-separated form. Independently bind
    # each approved byte-verified manifest entry to ONE installed file identity.
    $verifiedPaths=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($f in @($m.files)){
      $rel=[string]$f.path
      if($rel -cnotmatch '^[a-zA-Z0-9._ -]+(/[a-zA-Z0-9._ -]+)*$' -or
         -not $seen.Add($rel) -or ([string]$f.sha256) -cnotmatch '^[a-f0-9]{64}$' -or
         ([int64]$f.bytes) -lt 0){throw 'Untrusted browser relative path/hash/size'}
      $parts=$rel.Split('/')
      foreach($part in $parts){if($part -eq '.' -or $part -eq '..' -or $part -cne $part.Trim()){throw 'Unsafe browser segment'}}
      $dir=$root
      for($i=0;$i -lt ($parts.Count-1);$i++){
        $dir=Join-Path $dir $parts[$i]
        $di=Get-Item -LiteralPath $dir -Force -ErrorAction Stop
        if(-not $di.PSIsContainer -or ($di.Attributes -band [IO.FileAttributes]::ReparsePoint)){
          throw 'Installed Browser symlink in reviewed component parent'
        }
      }
      $fp=Join-Path $root ($parts -join '\')
      $fi=Get-Item -LiteralPath $fp -Force -ErrorAction Stop
      if($fi.PSIsContainer -or ($fi.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
         [int64]$fi.Length -ne [int64]$f.bytes -or
         (Get-FileHash -LiteralPath $fp -Algorithm SHA256).Hash.ToLowerInvariant() -cne $f.sha256){
        throw 'Installed Browser component hash/size/file-kind mismatch'
      }
      if(-not $verifiedPaths.Add($fi.FullName)){
        throw 'Installed Browser duplicate physical component'
      }
    }
    if($verifiedPaths.Count -ne $seen.Count -or $verifiedPaths.Contains($mi.FullName)){
      throw 'Installed Browser physical inventory alias'
    }
    foreach($need in @('chrome.exe','node.exe','lu_browser_worker.mjs',
                        'node_modules/playwright/index.mjs','node_modules/playwright-core/index.mjs')){
      if(-not $seen.Contains($need)){throw 'Installed Browser entrypoint missing'}
    }
    $actual=@(Get-ChildItem -LiteralPath $root -Recurse -Force -File)
    if($actual.Count -ne ($seen.Count+1)){throw 'Installed Browser file count mismatch'}
    foreach($item in $actual){
      if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Untrusted Browser file link'}
      # Every manifest file was SHA-256/size/kind verified above, and the
      # installed full paths were proven unique. Check the actual on-disk
      # inventory against those verified identities rather than re-parsing
      # Win32 path spelling into a separate untrusted relative-path string.
      if($item.FullName -ine $mi.FullName -and -not $verifiedPaths.Contains($item.FullName)){
        $relative=[IO.Path]::GetRelativePath($root,$item.FullName)
        throw ('Installed Browser unexpected file: '+$relative)
      }
    }
    & (Join-Path $root 'node.exe') --version
    if($LASTEXITCODE -ne 0){throw 'Real reviewed browser Node executable failed'}
    $record.installedReviewedBrowser='PASS_EXACT_BROWSER_FILES_AND_REAL_BUNDLED_NODE'
  }
  if($ExerciseInstalledGui){
    $stage='installed-ui'
    $uiDir=Join-Path $EvidenceDir 'p0-installed-ui'
    $pwsh=(Get-Process -Id $PID).Path
    $start=[Diagnostics.ProcessStartInfo]::new($pwsh)
    $start.UseShellExecute=$false
    foreach($arg in @('-NoProfile','-File',(Join-Path $PSScriptRoot 'p0-windows-installed-gui-smoke.ps1'),
      '-InstalledExe',$app,'-ExpectedSha256',$ExpectedLuSha256,
      '-EvidenceDir',$uiDir,'-DisposableWindowsConfirmed')){
      [void]$start.ArgumentList.Add([string]$arg)
    }
    try{
      $child=[Diagnostics.Process]::Start($start)
      if($null -eq $child){throw 'UI evidence child failed to start'}
      if(-not $child.WaitForExit(180000)){
        $child.Kill($true)
        $null=$child.WaitForExit(10000)
        throw 'Installed UI smoke exceeded 180 seconds'
      }
      $record.installedUiSmoke=if($child.ExitCode -eq 0){
        'PASS_INSTALLED_UNIFIED_TABS'
      }else{'FAIL_INSTALLED_UI_EVIDENCE'}
    }catch{
      $record.installedUiSmoke='FAIL_INSTALLED_UI_WORKER'
      $record.errors=@($record.errors)+@('UI smoke: '+$_.Exception.Message)
    }
  }
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
  # NSIS can schedule final file and directory cleanup after the parent
  # uninstaller exits. Observe a bounded 30 seconds without deleting anything
  # from the test host or weakening the final absence oracle.
  $watchPaths=@($app,$engine,$uninstaller)
  if($vulkanEngine){$watchPaths+=@($vulkanEngine)}
  if($researchExe){$watchPaths+=@($researchExe)}
  $timer=[Diagnostics.Stopwatch]::StartNew()
  $residue=@()
  do{
    $residue=@()
    foreach($candidate in $watchPaths){
      if(Test-Path -LiteralPath $candidate){$residue+=('file:'+ $candidate)}
    }
    if(Test-Path -LiteralPath $target){$residue+=('install-dir:'+ $target)}
    foreach($proc in @(Get-Process -Name 'Lu','lu-desktop' -ErrorAction SilentlyContinue)){
      $residue+=('process:'+ $proc.ProcessName + ':' + $proc.Id)
    }
    if($residue.Count -eq 0){break}
    Start-Sleep -Milliseconds 250
  }while($timer.Elapsed.TotalSeconds -lt 30)
  $timer.Stop()
  $record.removalObservedAfterMs=[int]$timer.ElapsedMilliseconds
  $record.postUninstallResidue=@($residue)
  # Independently witness profile preservation even if removal fails.
  # Neither result substitutes for the other.
  if((Test-Path -LiteralPath $marker -PathType Leaf) -and
    (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash -ceq $markerSha){
    $record.profilePreservation='PASS_UNTOUCHED_APPDATA_SENTINEL'
  }else{
    $record.profilePreservation='FAIL_APPDATA_SENTINEL_MISSING_OR_MODIFIED'
  }
  if($residue.Count -ne 0){
    $record.packageRemoval='FAIL_POST_UNINSTALL_RESIDUE'
    throw ('NSIS uninstall left: '+($residue -join '; '))
  }
  $record.packageRemoval='PASS_BINARIES_ABSENT'
  if($record.profilePreservation -ne 'PASS_UNTOUCHED_APPDATA_SENTINEL'){
    throw 'NSIS uninstall altered isolated AppData sentinel'
  }
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
if($ExerciseInstalledGui -and $record.installedUiSmoke -ne 'PASS_INSTALLED_UNIFIED_TABS'){exit 3}
# Without explicit success, a prior native command's LASTEXITCODE survives
# the script call and falsely marks a verified lifecycle receipt as failure.
exit 0
