# P06 independent interactive Windows installed-Lu smoke harness.
# This script runs ONLY on the user's real Windows desktop and never installs
# or downloads software. UI Automation is an independent OS observation, not
# a renderer-provided PASS claim. It cannot prove WGC, redaction or NSIS cycle.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$LuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$WorkerSha256,
  [switch]$InspectObserveUi
)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Physical interactive Windows desktop required'}
function Verify-Binary([string]$Path,[string]$Digest) {
  if(-not [IO.Path]::IsPathRooted($Path)){throw 'Must supply an absolute installed binary path'}
  $item=Get-Item -LiteralPath $Path -Force -ErrorAction Stop
  if($item.PSIsContainer -or (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'Ordinary installed binary required; no reparse-point substitution'
  }
  $actual=(Get-FileHash -Algorithm SHA256 -LiteralPath $item.FullName).Hash.ToLowerInvariant()
  if($actual -cne $Digest.ToLowerInvariant()){throw 'Installed binary does not match reviewed build SHA-256'}
  return $item.FullName
}
$app=Verify-Binary $InstalledExe $LuSha256
if([IO.Path]::GetFileName($app) -ne 'Lu.exe'){
  throw 'Use the installed native Lu.exe, not a browser, test dummy or renamed helper'
}
$worker=Verify-Binary (Join-Path (Split-Path -Parent $app) 'lu-observe-capture-worker.exe') $WorkerSha256
$record=[ordered]@{
  schema='Lu-P06-installed-evidence-v1'
  observedAtUtc=[DateTime]::UtcNow.ToString('o')
  appSha256=$LuSha256.ToLowerInvariant()
  workerSha256=$WorkerSha256.ToLowerInvariant()
  installedBinaryDigest='PASS'
  nativeWindowLaunched='INCONCLUSIVE'
  observeOffByDefault='NOT_TESTED'
  wgcPickerAndCancellation='NOT_TESTED'
  foregroundOsSourceInstance='NOT_TESTED'
  sensitiveRegionRedaction='NOT_TESTED'
  nsisInstallUninstallCycle='NOT_TESTED'
  visionOrGuiTestAcceptance='NOT_TESTED'
}
$child=$null
try {
  # A second Lu instance is intentionally disallowed by the existing
  # DesktopOwner. Close any previously running instance before this test.
  $child=Start-Process -FilePath $app -PassThru -ErrorAction Stop
  $until=[DateTime]::UtcNow.AddSeconds(30)
  do {
    Start-Sleep -Milliseconds 350
    $child.Refresh()
    if($child.HasExited){throw 'Installed Lu exited before exposing its native desktop window'}
    if($child.MainWindowHandle -ne [IntPtr]::Zero){break}
  } while([DateTime]::UtcNow -lt $until)
  if($child.MainWindowHandle -eq [IntPtr]::Zero){
    throw 'Installed Lu did not create a native top-level window'
  }
  $record.nativeWindowLaunched='PASS'
  if($InspectObserveUi){
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $root=[System.Windows.Automation.AutomationElement]::FromHandle($child.MainWindowHandle)
    if($null -eq $root){throw 'Windows UI Automation could not attest the Lu window'}
    $scope=[System.Windows.Automation.TreeScope]::Descendants
    $name=[System.Windows.Automation.AutomationElement]::NameProperty
    $start=$root.FindFirst($scope,
      ([System.Windows.Automation.PropertyCondition]::new(
        $name,'Start observing Lu')))
    $off=$root.FindFirst($scope,
      ([System.Windows.Automation.PropertyCondition]::new($name,'Off')))
    # UI controls exist only after selecting an existing conversation.
    # No browser event or app-provided boolean substitutes for Windows UIA.
    if($null -ne $start -and $null -ne $off){
      $record.observeOffByDefault='PASS'
    } else {
      $record.observeOffByDefault='INCONCLUSIVE_SELECT_EXISTING_CHAT'
    }
  }
} catch {
  if($record.nativeWindowLaunched -ne 'PASS'){$record.nativeWindowLaunched='FAIL'}
  $record.failureCategory='NATIVE_INSTALLED_LAUNCH_OR_OS_UIA'
  throw
} finally {
  if($null -ne $child){
    $child.Refresh()
    if(-not $child.HasExited){
      Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
      try{$child.WaitForExit(10000)}catch{}
    }
  }
  $evidence=Join-Path ([IO.Path]::GetTempPath()) (
    'lu-p06-installed-'+[Guid]::NewGuid().ToString('N')+'.json')
  ($record|ConvertTo-Json -Depth 6)|Set-Content -LiteralPath $evidence -Encoding UTF8 -NoNewline
  Write-Output "P06 native installed smoke receipt: $evidence"
  Write-Output 'OS app/window verification is not WGC, redaction, GUI test or NSIS acceptance.'
}
