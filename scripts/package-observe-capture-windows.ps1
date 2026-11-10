# Lu Observe & Learn — Windows build path for one packaged, pinned WGC worker.
# CI/release engineering only; this does not ask the user to build Lu locally.
# Run in a trusted Windows build job with Rust 1.98, Node 22, Tauri CLI v2,
# full Windows SDK and NSIS toolchain. No unverified downloaded worker.
[CmdletBinding()]
param(
  [switch]$BuildInstaller,
  [switch]$IncludeResearch,
  [string]$VerifyInstalledDirectory,
  [string]$ExpectedWorkerSha256,
  [string]$ExpectedResearchSha256
)

Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'

# Native installed-app preflight. Never compiles or stages a binary. This is
# a prerequisite to a separately witnessed, interactive picker/cancel test,
# NOT proof of a captured frame or verified Browser/Research functionality.
if($VerifyInstalledDirectory){
  if($BuildInstaller){throw 'Installed verification cannot build an installer'}
  if($ExpectedWorkerSha256 -cnotmatch '^[0-9a-fA-F]{64}$'){
    throw 'Expected worker SHA-256 from trusted CI artifact is required'
  }
  $dir=(Resolve-Path -LiteralPath $VerifyInstalledDirectory -ErrorAction Stop).Path
  $checks=@{
    'Lu.exe'=$null
    'lu-observe-capture-worker.exe'=$ExpectedWorkerSha256
  }
  if($ExpectedResearchSha256){
    if($ExpectedResearchSha256 -cnotmatch '^[0-9a-fA-F]{64}$'){
      throw 'Invalid trusted Research client SHA-256'
    }
    $checks['lu-research/curl.exe']=$ExpectedResearchSha256
  }
  $verified=@()
  foreach($name in ($checks.Keys | Sort-Object)){
    $candidate=Join-Path $dir $name
    $item=Get-Item -LiteralPath $candidate -ErrorAction Stop
    if($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){
      throw "Installed entry is not a regular file: $name"
    }
    $digest=(Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash.ToLowerInvariant()
    if($checks[$name] -and $digest -cne $checks[$name].ToLowerInvariant()){
      throw "Installed binary differs from trusted build: $name"
    }
    $verified+=@{ name=$name;sha256=$digest;bytes=$item.Length }
  }
  [pscustomobject]@{
    schema=1
    stage='installed-binary-preflight'
    status='PASS'
    captureFrameVerified=$false
    pickerCancellationVerified=$false
    desktopBrowserVerified=$false
    researchNetworkVerified=$false
    installDirectory=$dir
    files=$verified
  } | ConvertTo-Json -Depth 5
  return
}
$repo=Split-Path -Parent $PSScriptRoot
$workerManifest=Join-Path $repo 'src-tauri/lu-observe-capture-worker/Cargo.toml'
$workerExe=Join-Path $repo 'src-tauri/lu-observe-capture-worker/target/release/lu-observe-capture-worker.exe'
$desktopDir=Join-Path $repo 'lu-desktop'
$tauriDir=Join-Path $desktopDir 'src-tauri'
$stage=Join-Path $tauriDir 'resources/lu-observe-capture-worker.exe'
$overlay=Join-Path $tauriDir 'tauri.observe.capture.conf.json'

if($IncludeResearch -and -not $BuildInstaller){
  throw 'Research installer integration requires -BuildInstaller'
}
if(-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw 'A trusted Windows Rust/Cargo toolchain is required'
}
if(-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  throw 'Node/npm is required to build the Lu desktop'
}

# Build the actual WGC picker/D3D11 frame worker. Always use a pinned manifest.
& cargo build --release --manifest-path $workerManifest
if($LASTEXITCODE -ne 0){throw 'Windows Graphics Capture worker compile failed'}
if(-not (Test-Path -LiteralPath $workerExe -PathType Leaf)){
  throw 'Compiled Windows capture worker missing'
}

# Native host embeds this digest. Native supervisor independently hashes the
# installed executable before launching. A mismatch must fail closed.
$digest=(Get-FileHash -LiteralPath $workerExe -Algorithm SHA256).Hash.ToLowerInvariant()
if($digest -notmatch '^[0-9a-f]{64}$'){throw 'Invalid capture worker digest'}
New-Item -ItemType Directory -Path (Split-Path -Parent $stage) -Force | Out-Null
Copy-Item -LiteralPath $workerExe -Destination $stage -Force
$stagedDigest=(Get-FileHash -LiteralPath $stage -Algorithm SHA256).Hash.ToLowerInvariant()
if($stagedDigest -cne $digest){throw 'Staged capture worker digest mismatch'}
$env:LU_OBSERVE_CAPTURE_WORKER_SHA256=$digest

Push-Location $desktopDir
try {
  & npm ci
  if($LASTEXITCODE -ne 0){throw 'Lu dependencies could not be restored'}
  & npm run build
  if($LASTEXITCODE -ne 0){throw 'Lu frontend build failed'}
}
finally {Pop-Location}

if($BuildInstaller){
  # Tauri CLI must run from the actual lu-desktop project root. Running it
  # from the repository root resolves the wrong src-tauri directory.
  # Build a single opt-in overlay so WGC and Research can ship together.
  $bundleOverlay=$overlay
  $temporaryOverlay=$null
  try {
    if($IncludeResearch){
      & (Join-Path $repo 'scripts/stage-research-windows.ps1')
      if($LASTEXITCODE -and $LASTEXITCODE -ne 0){
        throw 'Trusted Research HTTPS client staging failed'
      }
      $researchOverlay=Join-Path $tauriDir 'tauri.research.conf.json'
      $captureConfig=Get-Content -LiteralPath $overlay -Raw | ConvertFrom-Json
      $researchConfig=Get-Content -LiteralPath $researchOverlay -Raw | ConvertFrom-Json
      foreach($property in $researchConfig.bundle.resources.PSObject.Properties){
        if($captureConfig.bundle.resources.PSObject.Properties[$property.Name]){
          throw 'Duplicate resource destination in combined installer'
        }
        $captureConfig.bundle.resources |
          Add-Member -MemberType NoteProperty -Name $property.Name -Value $property.Value
      }
      $temporaryOverlay=Join-Path $tauriDir 'tauri.observe.research.generated.conf.json'
      $captureConfig | ConvertTo-Json -Depth 10 |
        Set-Content -LiteralPath $temporaryOverlay -Encoding utf8
      $bundleOverlay=$temporaryOverlay
    }
    Push-Location $desktopDir
    try {
      # --config merge-patches resource mappings into the baseline Lu NSIS
      # bundle. This path builds the actual desktop app, not test placeholders.
      & cargo tauri build --ci --config $bundleOverlay
      if($LASTEXITCODE -ne 0){throw 'Lu NSIS build with trusted resources failed'}
    } finally {Pop-Location}
  } finally {
    if($temporaryOverlay -and (Test-Path -LiteralPath $temporaryOverlay)){
      Remove-Item -LiteralPath $temporaryOverlay -Force
    }
  }
}else{
  & cargo build --release --locked --manifest-path (Join-Path $tauriDir 'Cargo.toml')
  if($LASTEXITCODE -ne 0){throw 'Lu desktop native host build failed'}
  # Non-installer desktop binary has to be next to the pinned worker.
  $appExe=Join-Path $tauriDir 'target/release/lu-desktop.exe'
  if(Test-Path -LiteralPath $appExe -PathType Leaf){
    Copy-Item -LiteralPath $workerExe -Destination (Join-Path (Split-Path -Parent $appExe) 'lu-observe-capture-worker.exe') -Force
  }
}
Write-Output ("Lu Windows Graphics Capture build complete. Verified worker SHA-256: "+$digest)
Write-Output 'This build path does not by itself prove physical Windows picker/GUI acceptance.'
