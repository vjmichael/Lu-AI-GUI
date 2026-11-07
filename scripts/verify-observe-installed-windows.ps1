# P06 independent interactive Windows installed-Lu smoke harness.
# This script runs ONLY on the user's real Windows desktop and never installs
# or downloads software. UI Automation is an independent OS observation, not
# a renderer-provided PASS claim. It cannot prove WGC, redaction or NSIS cycle.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$LuSha256,
  [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$WorkerSha256,
  [switch]$InspectObserveUi,
  [switch]$ExerciseObservationLifecycle,
  [switch]$ExerciseCaptureCancellation
)
$ErrorActionPreference='Stop'
if($ExerciseObservationLifecycle -and -not $InspectObserveUi){
  throw 'Lifecycle acceptance requires explicit -InspectObserveUi opt-in'
}
if($ExerciseCaptureCancellation -and -not $ExerciseObservationLifecycle){
  throw 'Capture worker reap trial requires explicit -ExerciseObservationLifecycle and -InspectObserveUi'
}
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
  schema='Lu-P06-installed-evidence-v2'
  observedAtUtc=[DateTime]::UtcNow.ToString('o')
  appSha256=$LuSha256.ToLowerInvariant()
  workerSha256=$WorkerSha256.ToLowerInvariant()
  installedBinaryDigest='PASS'
  nativeWindowLaunched='INCONCLUSIVE'
  uiAutomationWindowProcessAttested='NOT_TESTED'
  observeOffByDefault='NOT_TESTED'
  observeUiLifecycle='NOT_TESTED'
  captureWorkerOsReap='NOT_TESTED'
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
    # Independent OS UIA process identity must match reviewed launched Lu.
    if([int64]$root.Current.ProcessId -ne [int64]$child.Id){
      throw 'Observed native UIA window belongs to a different process'
    }
    $record.uiAutomationWindowProcessAttested='PASS'
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
    if($ExerciseObservationLifecycle){
      if($record.observeOffByDefault -ne 'PASS'){
        throw 'Select a locally owned existing chat before running the optional lifecycle'
      }
      function Find-ObservedButton([string]$buttonName){
        $query=[System.Windows.Automation.PropertyCondition]::new(
          [System.Windows.Automation.AutomationElement]::NameProperty,$buttonName)
        return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$query)
      }
      function Press-ObservedButton([string]$buttonName){
        $element=Find-ObservedButton $buttonName
        if($null -eq $element){return $false}
        try{
          $invoke=$element.GetCurrentPattern(
            [System.Windows.Automation.InvokePattern]::Pattern)
          $invoke.Invoke()
          return $true
        }catch{return $false}
      }
      function Await-ObservedButton([string]$buttonName,[int]$seconds){
        $deadline=[DateTime]::UtcNow.AddSeconds($seconds)
        do{
          if($null -ne (Find-ObservedButton $buttonName)){return $true}
          Start-Sleep -Milliseconds 300
        }while([DateTime]::UtcNow -lt $deadline)
        return $false
      }
      # For this optional physical Windows trial, the independent Win32
      # process table (not a renderer "worker ready" hint) establishes that
      # an EXACT installed hash-reviewed WGC worker was started by Lu.
      # No screenshot, window title, raw process arguments or frame pixel is
      # placed into this evidence receipt.
      function Get-OwnedCaptureWorkerPids {
        $found=@()
        $entries=@(Get-CimInstance -ClassName Win32_Process -Filter "Name='lu-observe-capture-worker.exe'" -ErrorAction Stop)
        foreach($entry in $entries){
          if([int64]$entry.ParentProcessId -ne [int64]$child.Id){continue}
          if([string]::IsNullOrWhiteSpace($entry.ExecutablePath)){
            throw 'OS cannot identify the native child image; no worker provenance accepted'
          }
          $actualPath=[IO.Path]::GetFullPath([string]$entry.ExecutablePath)
          if($actualPath -ine [IO.Path]::GetFullPath($worker)){
            throw 'Unreviewed WGC child process image; native worker identity mismatch'
          }
          $found+=([int64]$entry.ProcessId)
        }
        return $found
      }
      # Explicit interactive operator approval is still required in the
      # native Lu consent modal. The harness does NOT approve or dismiss it.
      if(-not (Press-ObservedButton 'Start observing Lu')){
        throw 'Native UI Automation could not invoke the Lu-only consent request'
      }
      if(-not (Await-ObservedButton 'Pause' 45)){
        $record.observeUiLifecycle='INCONCLUSIVE_NATIVE_CONSENT_NOT_APPROVED'
      }else{
        if(-not (Press-ObservedButton 'Pause') -or
          -not (Await-ObservedButton 'Resume' 8)){
          throw 'Actual installed Observe UI did not enter paused state'
        }
        if(-not (Press-ObservedButton 'Resume') -or
          -not (Await-ObservedButton 'Stop observation' 8)){
          throw 'Actual installed Observe UI did not resume'
        }
        $captureObserved=$false
        if($ExerciseCaptureCancellation){
          if(@(Get-OwnedCaptureWorkerPids).Count -ne 0){
            throw 'Capture worker was already running before the operator test'
          }
          # This invokes only the real Lu UI action; the distinct Lu-native
          # warning must be approved by the human, then leave the Windows
          # picker OPEN for the Stop/reap trial. Never automate approval.
          Write-Output 'Operator trial: approve native capture warning manually and leave Windows picker open.'
          if(-not (Press-ObservedButton 'Select window/display · capture one frame')){
            throw 'Installed Lu did not expose the native OS-picker request control'
          }
          $workerDeadline=[DateTime]::UtcNow.AddSeconds(60)
          $observedWorkerPids=@()
          do{
            $observedWorkerPids=@(Get-OwnedCaptureWorkerPids)
            if($observedWorkerPids.Count -gt 0){$captureObserved=$true;break}
            Start-Sleep -Milliseconds 300
          }while([DateTime]::UtcNow -lt $workerDeadline)
          if(-not $captureObserved){
            $record.captureWorkerOsReap='INCONCLUSIVE_NO_NATIVE_CHILD_OBSERVED'
          }else{
            $record.captureWorkerOsReap='PENDING_NATIVE_STOP'
          }
        }
        if(-not (Press-ObservedButton 'Stop observation') -or
          -not (Await-ObservedButton 'Start observing Lu' 8)){
          throw 'Actual installed Observe UI did not return to OFF'
        }
        if($ExerciseCaptureCancellation -and $captureObserved){
          $reapDeadline=[DateTime]::UtcNow.AddSeconds(15)
          do{
            if(@(Get-OwnedCaptureWorkerPids).Count -eq 0){break}
            Start-Sleep -Milliseconds 250
          }while([DateTime]::UtcNow -lt $reapDeadline)
          if(@(Get-OwnedCaptureWorkerPids).Count -gt 0){
            $record.captureWorkerOsReap='FAIL_WORKER_SURVIVED_NATIVE_STOP'
            throw 'Native WGC process survived Stop: no cancellation success or Guardian release accepted'
          }
          # Check original OS PIDs as well: a changed image name/parent
          # must not be accepted as proof of worker process cleanup.
          foreach($observedPid in $observedWorkerPids){
            $remaining=@(Get-CimInstance -ClassName Win32_Process -Filter ("ProcessId={0}" -f $observedPid) -ErrorAction Stop)
            if($remaining.Count -gt 0){
              $record.captureWorkerOsReap='FAIL_OBSERVED_OS_PROCESS_STILL_ACTIVE'
              throw 'Previously observed WGC child PID remains alive after Stop'
            }
          }
          $record.captureWorkerOsReap='PASS_OS_CHILD_REAP_ONLY'
        }
        # A reused HWND or replaced process after Start/Stop is not proof.
        $child.Refresh()
        if($child.HasExited -or [int64]$root.Current.ProcessId -ne [int64]$child.Id){
          throw 'Native Lu process or UIA ownership changed during test'
        }
        $record.observeUiLifecycle='PASS_UIA_VISIBLE_LIFECYCLE_ONLY'
      }
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
  Write-Output 'OS app/window and optional WGC child reaping do not prove picker selection, sensitive-region redaction, GUI test or NSIS acceptance.'
}
