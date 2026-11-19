# Read-only acceptance-harness regression: recorded UIA provider shape.
# This does not launch Lu, approve anything, or stand in for the native test.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$harness=Join-Path $PSScriptRoot 'cp05e5-windows-installed-gguf.ps1'
$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile($harness,[ref]$tokens,[ref]$errors)
if($errors.Count -ne 0){throw "Installed witness syntax rejected: $errors"}
$definitions=@($ast.FindAll({
  param($n)
  $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
    $n.Name -ceq 'Test-ConsentControlIdentity'
},$true))
if($definitions.Count -ne 1){throw 'Expected exactly one consent identity predicate'}
# Load only this checked-in pure predicate, never run the installed witness.
. ([scriptblock]::Create($definitions[0].Extent.Text))
function New-Fixture([string]$name='Allow once',[string]$id='CommandButton_1000',
  [string]$kind='ControlType.Pane',[string]$class='CCPushButton',
  [int]$owner=8084,[long]$handle=66178,[bool]$enabled=$true,[bool]$offscreen=$false){
  [pscustomobject]@{
    Name=$name;AutomationId=$id;ClassName=$class
    ControlType=[pscustomobject]@{ProgrammaticName=$kind}
    ProcessId=$owner;NativeWindowHandle=$handle;IsEnabled=$enabled;IsOffscreen=$offscreen
  }
}
function Expect([bool]$actual,[bool]$wanted,[string]$label){
  if($actual -ne $wanted){throw "Consent identity fixture mismatch: $label"}
  Write-Output ("PASS_CP05_CONSENT_IDENTITY: "+$label)
  $script:passed++
}
$script:passed=0
# Original failed-run receipt 14481458024, exact native Pane names/IDs/class.
Expect (Test-ConsentControlIdentity (New-Fixture) 'Allow once' 8084) $true 'original allow pane'
Expect (Test-ConsentControlIdentity (New-Fixture -name 'Cancel' -id 'CommandButton_1001' -handle 66182) 'Cancel' 8084) $true 'original cancel pane'
Expect (Test-ConsentControlIdentity (New-Fixture -kind 'ControlType.Button') 'Allow once' 8084) $true 'same native command normal button'
Expect (Test-ConsentControlIdentity (New-Fixture -id 'CommandButton_1001') 'Allow once' 8084) $false 'opposite command id'
Expect (Test-ConsentControlIdentity (New-Fixture -name 'Cancel') 'Allow once' 8084) $false 'opposite label'
Expect (Test-ConsentControlIdentity (New-Fixture -name 'allow once') 'Allow once' 8084) $false 'nonexact label'
Expect (Test-ConsentControlIdentity (New-Fixture -class 'Element') 'Allow once' 8084) $false 'generic pane'
Expect (Test-ConsentControlIdentity (New-Fixture -kind 'ControlType.Text') 'Allow once' 8084) $false 'noncommand type'
Expect (Test-ConsentControlIdentity (New-Fixture -owner 8085) 'Allow once' 8084) $false 'foreign process'
Expect (Test-ConsentControlIdentity (New-Fixture -handle 0) 'Allow once' 8084) $false 'missing HWND'
Expect (Test-ConsentControlIdentity (New-Fixture -enabled $false) 'Allow once' 8084) $false 'disabled command'
Expect (Test-ConsentControlIdentity (New-Fixture -offscreen $true) 'Allow once' 8084) $false 'offscreen command'
Expect (Test-ConsentControlIdentity (New-Fixture) 'Approve' 8084) $false 'unrecognized action'
if($script:passed -ne 13){throw 'Consent fixture inventory incomplete'}
Write-Output 'CP05_NATIVE_CONSENT_IDENTITY_FIXTURES_PASS=13'
