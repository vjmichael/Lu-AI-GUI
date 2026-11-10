# Genuine installed Windows Tauri/WebView2 smoke; NO GGUF or tool effects.
# Intended only for a disposable, explicitly confirmed Windows test host.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$InstalledExe,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
  [Parameter(Mandatory=$true)][string]$EvidenceDir,
  [switch]$DisposableWindowsConfirmed
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not $DisposableWindowsConfirmed){
  throw 'Installed UI smoke requires explicitly confirmed disposable Windows'
}
if(-not(Test-Path -LiteralPath $InstalledExe -PathType Leaf) -or
   (Get-FileHash -LiteralPath $InstalledExe -Algorithm SHA256).Hash -ine $ExpectedSha256){
  throw 'Native installed executable SHA-256 differs from the reviewed build'
}
if(Test-Path -LiteralPath $EvidenceDir){throw 'UI evidence path must not already exist'}
$null=New-Item -ItemType Directory -Path $EvidenceDir
$report=[ordered]@{
  schema='lu-p0-real-windows-installed-ui-smoke-v1'
  installedExeSha256=$ExpectedSha256.ToLowerInvariant()
  nativeWindow='NOT_TESTED'
  uiAutomationProcessIdentity='NOT_TESTED'
  singleSettingsEntry='NOT_TESTED'
  independentSettingsWindow='NOT_TESTED'
  independentModelLibrary='NOT_TESTED'
  settingsScreenshot='NOT_TESTED'
  modelLibraryScreenshot='NOT_TESTED'
  genuineGgufInference='NOT_TESTED'
  activeGpuOffload='NOT_TESTED'
  result='NOT_ACCEPTED'
  errors=@()
}
$process=$null
function Wait-Element([System.Windows.Automation.AutomationElement]$window,[string]$label,[bool]$button=$false){
  $deadline=[DateTime]::UtcNow.AddSeconds(20)
  do{
    $items=$window.FindAll(
      [System.Windows.Automation.TreeScope]::Descendants,
      [System.Windows.Automation.Condition]::TrueCondition
    )
    foreach($item in $items){
      try{
        if($item.Current.Name -ne $label){continue}
        if($button -and $item.Current.ControlType -ne [System.Windows.Automation.ControlType]::Button){continue}
        return $item
      }catch{}
    }
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  throw "Installed WebView2 did not expose UI Automation element: $label"
}
function Invoke-UIA([System.Windows.Automation.AutomationElement]$element){
  $invoke=$element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
  if($null -eq $invoke){throw 'Genuine UIA invoke pattern absent'}
  $invoke.Invoke()
}
function Save-Visible-Desktop([string]$name){
  Add-Type -AssemblyName System.Windows.Forms
  Add-Type -AssemblyName System.Drawing
  $display=[System.Windows.Forms.Screen]::PrimaryScreen
  if($null -eq $display -or $display.Bounds.Width -lt 640 -or $display.Bounds.Height -lt 480){
    throw 'No adequately sized interactive Windows screen for screenshot'
  }
  $bmp=[System.Drawing.Bitmap]::new($display.Bounds.Width,$display.Bounds.Height)
  $g=[System.Drawing.Graphics]::FromImage($bmp)
  try{
    $g.CopyFromScreen($display.Bounds.Location,[System.Drawing.Point]::Empty,$display.Bounds.Size)
    $path=Join-Path $EvidenceDir $name
    $bmp.Save($path,[System.Drawing.Imaging.ImageFormat]::Png)
    if((Get-Item -LiteralPath $path).Length -lt 2000){throw 'Empty installed UI screenshot'}
    return $path
  }finally{$g.Dispose();$bmp.Dispose()}
}
try{
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
  $process=Start-Process -FilePath $InstalledExe -WorkingDirectory (Split-Path -Parent $InstalledExe) -PassThru
  $deadline=[DateTime]::UtcNow.AddSeconds(45)
  do{
    $process.Refresh()
    if($process.HasExited){throw 'Installed Lu exited before native window appeared'}
    if($process.MainWindowHandle -ne [IntPtr]::Zero){break}
    Start-Sleep -Milliseconds 250
  }while([DateTime]::UtcNow -lt $deadline)
  if($process.MainWindowHandle -eq [IntPtr]::Zero){throw 'Installed Lu never opened native GUI window'}
  $report.nativeWindow='PASS_WIN32_WINDOW_HANDLE'
  $window=[System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
  if($null -eq $window -or [int64]$window.Current.ProcessId -ne [int64]$process.Id){
    throw 'Genuine UIA window does not belong to the installed Lu process'
  }
  $report.uiAutomationProcessIdentity='PASS_EXACT_OS_PROCESS'
  $settings=Wait-Element $window 'Settings' $true
  $all=$window.FindAll(
    [System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition
  )
  $settingsButtons=0
  foreach($control in $all){
    try{
      if($control.Current.Name -eq 'Settings' -and
         $control.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button){
        $settingsButtons++
      }
    }catch{}
  }
  if($settingsButtons -ne 1){throw "Expected exactly one Settings button; observed $settingsButtons"}
  $report.singleSettingsEntry='PASS_ONLY_TOP_RIGHT_GEAR'
  Invoke-UIA $settings
  $null=Wait-Element $window 'Lu settings'
  $report.independentSettingsWindow='PASS_NATIVE_WEBVIEW_DIALOG'
  $report.settingsScreenshot=Save-Visible-Desktop 'installed-settings.png'
  Invoke-UIA (Wait-Element $window 'Close settings' $true)
  Invoke-UIA (Wait-Element $window 'Choose a model' $true)
  Invoke-UIA (Wait-Element $window 'Manage models' $true)
  $null=Wait-Element $window 'Model Library'
  $report.independentModelLibrary='PASS_NATIVE_WEBVIEW_DIALOG'
  $report.modelLibraryScreenshot=Save-Visible-Desktop 'installed-model-library.png'
  Invoke-UIA (Wait-Element $window 'Close model library' $true)
  $report.result='PASS_INSTALLED_NATIVE_WINDOW_AND_BOTH_DIALOGS'
}catch{
  $report.errors=@($report.errors)+@($_.Exception.Message)
  Write-Warning ('Genuine installed UI smoke is NOT ACCEPTED: '+$_.Exception.Message)
}finally{
  if($null -ne $process){
    try{
      $process.Refresh()
      if(-not $process.HasExited){
        if($process.CloseMainWindow()){
          if(-not $process.WaitForExit(15000)){$process.Kill($true)}
        }else{$process.Kill($true)}
        $null=$process.WaitForExit(10000)
      }
    }catch{
      $report.errors=@($report.errors)+@('Failed to reap owned GUI process: '+$_.Exception.Message)
      $report.result='NOT_ACCEPTED'
    }
  }
  $report|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $EvidenceDir 'p0-installed-ui.json') -Encoding UTF8
}
if($report.result -ne 'PASS_INSTALLED_NATIVE_WINDOW_AND_BOTH_DIALOGS'){exit 2}
exit 0
