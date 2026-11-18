# Lu P06 — build and optionally package the hash-pinned native WGC worker.
# CI/release engineering only; no arbitrary internet downloads or dummy engine.
# -VerifyInstalledDirectory is a READ-ONLY binary preflight, NOT a WGC test.
[CmdletBinding()]
param(
  [switch]$BuildInstaller,
  [switch]$IncludeResearch,
  [string]$VerifyInstalledDirectory,
  [string]$ExpectedLuSha256,
  [string]$ExpectedWorkerSha256,
  [string]$ExpectedResearchSha256
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Is-Sha([string]$value){
  return $value -cmatch '^[a-fA-F0-9]{64}$'
}
function Assert-RegularFile([string]$name){
  $item=Get-Item -LiteralPath $name -Force -ErrorAction Stop
  if($item.PSIsContainer -or (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){
    throw 'A normal installed file is required (no symlinks or reparse points)'
  }
  return $item
}
function Verify-ExactDigest([string]$name,[string]$digest){
  $null=Assert-RegularFile $name
  if(-not (Is-Sha $digest)){throw 'A complete trusted SHA-256 digest is required'}
  $actual=(Get-FileHash -Algorithm SHA256 -LiteralPath $name).Hash.ToLowerInvariant()
  if($actual -cne $digest.ToLowerInvariant()){
    throw 'Trusted installed/staged file SHA-256 did not match'
  }
  return $actual
}
if($VerifyInstalledDirectory){
  if($BuildInstaller -or $IncludeResearch){
    throw 'Read-only installed verification cannot compile, stage or install software'
  }
  if(-not (Is-Sha $ExpectedLuSha256) -or -not (Is-Sha $ExpectedWorkerSha256)){
    throw 'Reviewed exact Lu.exe and WGC worker SHA-256 digests are mandatory'
  }
  $dir=(Resolve-Path -LiteralPath $VerifyInstalledDirectory -ErrorAction Stop).Path
  $verified=@()
  foreach($entry in @(
    @{name='Lu.exe';expected=$ExpectedLuSha256},
    @{name='lu-observe-capture-worker.exe';expected=$ExpectedWorkerSha256}
  )){
    $candidate=Join-Path $dir $entry.name
    $item=Assert-RegularFile $candidate
    $digest=Verify-ExactDigest $candidate $entry.expected
    $verified+=@{name=$entry.name;sha256=$digest;bytes=$item.Length}
  }
  if($ExpectedResearchSha256){
    $candidate=Join-Path $dir 'lu-research/curl.exe'
    $item=Assert-RegularFile $candidate
    $digest=Verify-ExactDigest $candidate $ExpectedResearchSha256
    $verified+=@{name='lu-research/curl.exe';sha256=$digest;bytes=$item.Length}
  }
  [pscustomobject]@{
    schema=2
    stage='installed-binary-preflight'
    verifiedBinaryDigests='PASS'
    nativeGuiLaunch='NOT_TESTED'
    wgcPicker='NOT_TESTED'
    sensitiveRedaction='NOT_TESTED'
    installedNsisCycle='NOT_TESTED'
    modelVision='NOT_TESTED'
    files=$verified
  } | ConvertTo-Json -Depth 6
  return
}
if($ExpectedLuSha256){throw '-ExpectedLuSha256 is for installed-binary verification only'}
if($IncludeResearch -and -not $BuildInstaller){
  throw 'Research resource combination requires an actual NSIS installer build'
}
if($ExpectedWorkerSha256 -and -not (Is-Sha $ExpectedWorkerSha256)){
  throw 'Expected Windows WGC worker SHA-256 is invalid'
}
if($ExpectedResearchSha256 -and -not (Is-Sha $ExpectedResearchSha256)){
  throw 'Expected Research binary SHA-256 is invalid'
}
if(-not (Get-Command cargo -ErrorAction SilentlyContinue)){
  throw 'Trusted Windows Cargo/Rust toolchain required'
}
if(-not (Get-Command npm -ErrorAction SilentlyContinue)){
  throw 'Node/npm is required for the native Lu frontend'
}
$repo=Split-Path -Parent $PSScriptRoot
$desktop=Join-Path $repo 'lu-desktop'
$tauri=Join-Path $desktop 'src-tauri'
$manifest=Join-Path $repo 'src-tauri/lu-observe-capture-worker/Cargo.toml'
$worker=Join-Path $repo 'src-tauri/lu-observe-capture-worker/target/release/lu-observe-capture-worker.exe'
$staged=Join-Path $tauri 'resources/lu-observe-capture-worker.exe'
$overlay=Join-Path $tauri 'tauri.observe.capture.conf.json'
if($env:OS -ne 'Windows_NT'){throw 'Native WGC worker build requires Windows'}
$null=Assert-RegularFile $manifest
$null=Assert-RegularFile $overlay
# No default Docker, shell-installed helper, fake hash, or downloaded worker.
& cargo build --release --manifest-path $manifest
if($LASTEXITCODE -ne 0){throw 'Native Windows WGC worker build failed'}
$null=Assert-RegularFile $worker
$digest=(Get-FileHash -Algorithm SHA256 -LiteralPath $worker).Hash.ToLowerInvariant()
if($ExpectedWorkerSha256 -and $digest -cne $ExpectedWorkerSha256.ToLowerInvariant()){
  throw 'Built worker differs from the independently reviewed worker digest'
}
New-Item -ItemType Directory -Path (Split-Path -Parent $staged) -Force | Out-Null
Copy-Item -LiteralPath $worker -Destination $staged -Force
$null=Verify-ExactDigest $staged $digest
$env:LU_OBSERVE_CAPTURE_WORKER_SHA256=$digest
Push-Location $desktop
try{
  # This project has no checked-in npm package-lock.json. npm ci cannot run.
  & npm install --no-package-lock --ignore-scripts --no-audit --no-fund
  if($LASTEXITCODE -ne 0){throw 'Lu frontend dependencies unavailable'}
  & npm run build
  if($LASTEXITCODE -ne 0){throw 'Lu actual React frontend build failed'}
}finally{Pop-Location}
$temporary=$null
if($BuildInstaller){
  $bundleOverlay=$overlay
  try{
    if($IncludeResearch){
      $researchScript=Join-Path $repo 'scripts/stage-research-windows.ps1'
      $null=Assert-RegularFile $researchScript
      & $researchScript
      if($LASTEXITCODE -and $LASTEXITCODE -ne 0){
        throw 'Reviewed Research runtime staging failed'
      }
      $researchOverlay=Join-Path $tauri 'tauri.research.conf.json'
      $capture=Get-Content -LiteralPath $overlay -Raw | ConvertFrom-Json
      $research=Get-Content -LiteralPath $researchOverlay -Raw | ConvertFrom-Json
      foreach($property in $research.bundle.resources.PSObject.Properties){
        if($capture.bundle.resources.PSObject.Properties[$property.Name]){
          throw 'Duplicate destination resource in combined WGC/Research overlay'
        }
        $capture.bundle.resources |
          Add-Member -MemberType NoteProperty -Name $property.Name -Value $property.Value
      }
      $temporary=Join-Path $tauri 'tauri.observe.research.generated.conf.json'
      $capture|ConvertTo-Json -Depth 30 |
        Set-Content -LiteralPath $temporary -Encoding UTF8
      $bundleOverlay=$temporary
    }
    Push-Location $desktop
    try{
      # The actual NSIS packaging toolchain and pinned inference resources
      # must exist independently; no inert placeholders are acceptable.
      & cargo tauri build --ci --config $bundleOverlay
      if($LASTEXITCODE -ne 0){throw 'Actual NSIS packaging build failed'}
    }finally{Pop-Location}
  }finally{
    if($temporary -and (Test-Path -LiteralPath $temporary)){
      Remove-Item -LiteralPath $temporary -Force
    }
  }
}else{
  & cargo build --release --locked --manifest-path (Join-Path $tauri 'Cargo.toml')
  if($LASTEXITCODE -ne 0){throw 'Lu native desktop compile failed'}
  $app=Join-Path $tauri 'target/release/lu-desktop.exe'
  $null=Assert-RegularFile $app
  $destination=Join-Path (Split-Path -Parent $app) 'lu-observe-capture-worker.exe'
  Copy-Item -LiteralPath $worker -Destination $destination -Force
  $null=Verify-ExactDigest $destination $digest
}
Write-Output ("Compiled Windows WGC worker, verified SHA-256: "+$digest)
Write-Output 'This build and binary check DO NOT prove installed/interactive WGC, GUI or model acceptance.'
