# Lu Observe & Learn — Windows build path for one packaged, pinned WGC worker.
# CI/release engineering only; this does not ask the user to build Lu locally.
# Run in a trusted Windows build job with Rust 1.98, Node 22, Tauri CLI v2,
# full Windows SDK and NSIS toolchain. No unverified downloaded worker.
[CmdletBinding()]
param(
  [switch]$BuildInstaller,
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
  if($ExpectedWorkerSha256 -cnotmatch '^[0-9a-fA-F]{64}
$repo=Split-Path -Parent $PSScriptRoot
$workerManifest=Join-Path $repo 'src-tauri/lu-observe-capture-worker/Cargo.toml'
$workerExe=Join-Path $repo 'src-tauri/lu-observe-capture-worker/target/release/lu-observe-capture-worker.exe'
$desktopDir=Join-Path $repo 'lu-desktop'
$tauriDir=Join-Path $desktopDir 'src-tauri'
$stage=Join-Path $tauriDir 'resources/lu-observe-capture-worker.exe'
$overlay=Join-Path $tauriDir 'tauri.observe.capture.conf.json'

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
  # --config is RFC7396 merge-patched into the baseline Tauri config,
  # including the original inference DLLs and all other bundle resources.
  & cargo tauri build --ci --config $overlay
  if($LASTEXITCODE -ne 0){throw 'Lu NSIS build with capture worker failed'}
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
){
    throw 'Expected worker SHA-256 from trusted CI artifact is required'
  }
  $dir=(Resolve-Path -LiteralPath $VerifyInstalledDirectory -ErrorAction Stop).Path
  $checks=@{
    'Lu.exe'=$null
    'lu-observe-capture-worker.exe'=$ExpectedWorkerSha256
  }
  if($ExpectedResearchSha256){
    if($ExpectedResearchSha256 -cnotmatch '^[0-9a-fA-F]{64}
$repo=Split-Path -Parent $PSScriptRoot
$workerManifest=Join-Path $repo 'src-tauri/lu-observe-capture-worker/Cargo.toml'
$workerExe=Join-Path $repo 'src-tauri/lu-observe-capture-worker/target/release/lu-observe-capture-worker.exe'
$desktopDir=Join-Path $repo 'lu-desktop'
$tauriDir=Join-Path $desktopDir 'src-tauri'
$stage=Join-Path $tauriDir 'resources/lu-observe-capture-worker.exe'
$overlay=Join-Path $tauriDir 'tauri.observe.capture.conf.json'

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
  # --config is RFC7396 merge-patched into the baseline Tauri config,
  # including the original inference DLLs and all other bundle resources.
  & cargo tauri build --ci --config $overlay
  if($LASTEXITCODE -ne 0){throw 'Lu NSIS build with capture worker failed'}
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
){
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
  # --config is RFC7396 merge-patched into the baseline Tauri config,
  # including the original inference DLLs and all other bundle resources.
  & cargo tauri build --ci --config $overlay
  if($LASTEXITCODE -ne 0){throw 'Lu NSIS build with capture worker failed'}
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
