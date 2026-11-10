# Native Windows x64 current-source Lu development preview.
# Source-pinned Qwen3.8-compatible llama.cpp, dual CPU+Vulkan backends.
# This is NOT the historic alpha, production or AMD Vulkan acceptance.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-f0-9]{40}$')][string]$SourceSha,
  [Parameter(Mandatory=$true)][string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Native Windows build host required'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $repo
$originalConfig=$null
try{
  $head=(& git rev-parse HEAD).Trim()
  if($LASTEXITCODE -ne 0 -or $head -cne $SourceSha){throw "Source SHA mismatch: $head"}
  & node scripts/verify-source-tree.mjs
  if($LASTEXITCODE -ne 0){throw 'REPO-01 frozen source mismatch'}
  $out=[IO.Path]::GetFullPath($OutputDirectory)
  New-Item -Force -ItemType Directory $out|Out-Null
  $upstream='e0f142d5ddc1461ce721644d92af33db23545105'  # llama.cpp b12005 Qwen3.8 qwen35/MTP support
  $vendor=Join-Path $env:RUNNER_TEMP 'lu-current-llama-source'
  if(Test-Path $vendor){throw 'Pinned source directory pre-exists'}
  New-Item -ItemType Directory $vendor|Out-Null
  & git -C $vendor init -q
  if($LASTEXITCODE -ne 0){throw 'Git init failed'}
  & git -C $vendor remote add origin 'https://github.com/ggml-org/llama.cpp.git'
  & git -C $vendor fetch --depth=1 origin $upstream
  if($LASTEXITCODE -ne 0){throw 'Pinned upstream source fetch failed'}
  & git -C $vendor checkout --detach -q FETCH_HEAD
  if($LASTEXITCODE -ne 0 -or (& git -C $vendor rev-parse HEAD).Trim() -cne $upstream){
    throw 'Upstream exact commit verification failed'
  }
  # GitHub windows-2025 may carry VS 2026 (18.x), not VS 2022 (17.x).
  # Bind to the installed host MSVC toolchain, never silently substitute MinGW.
  $vswhere=(Get-Command vswhere.exe -ErrorAction SilentlyContinue).Source
  if(-not $vswhere){
    $vswhere=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio\Installer\vswhere.exe'
  }
  if(-not(Test-Path -LiteralPath $vswhere)){throw 'Native Visual Studio host discovery missing'}
  $vsArgs=@('-latest','-products','*','-requires',
    'Microsoft.VisualStudio.Component.VC.Tools.x86.x64')
  $vsPath=(& $vswhere @vsArgs -property installationPath).Trim()
  $vsVersion=(& $vswhere @vsArgs -property installationVersion).Trim()
  if($LASTEXITCODE -ne 0 -or -not(Test-Path -LiteralPath $vsPath)){
    throw 'No verified installed Visual Studio C++ workload'
  }
  $major=[int]($vsVersion.Split('.')[0])
  $generator=switch($major){
    17 {'Visual Studio 17 2022'}
    18 {'Visual Studio 18 2026'}
    default {throw "Unsupported host MSVC major $major"}
  }
  Write-Host "Pinned llama.cpp build: installed MSVC $vsVersion at $vsPath; CMake generator $generator"
  $engineBuild=Join-Path $env:RUNNER_TEMP 'lu-current-llama-build'
  $vulkanBuild=Join-Path $env:RUNNER_TEMP 'lu-current-vulkan-build'
  & cmake -S $vendor -B $engineBuild -G $generator -A x64 -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DCMAKE_POLICY_DEFAULT_CMP0091=NEW -DBUILD_SHARED_LIBS=OFF -DGGML_NATIVE=OFF -DGGML_VULKAN=OFF -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_APP=OFF -DLLAMA_BUILD_UI=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=ON
  if($LASTEXITCODE -ne 0){throw 'Pinned CPU llama.cpp CMake configure failed'}
  & cmake --build $engineBuild --config Release --target llama-server --parallel 4
  if($LASTEXITCODE -ne 0){throw 'Pinned CPU llama.cpp build failed'}
  $engines=@(Get-ChildItem -LiteralPath $engineBuild -Recurse -File -Filter 'llama-server.exe'|Where-Object {$_.FullName -match '(?i)[\\/]Release[\\/]'})
  if($engines.Count -ne 1){throw "Expected one Release llama-server.exe, found $($engines.Count)"}
  $engine=$engines[0]
  if($engine.Length -lt 1000000 -or $engine.Length -gt 250000000){throw 'Abnormal engine byte size'}
  # This package must contain a real launchable source-built server, not
  # a placeholder. A bounded --help launch does not load a GGUF or
  # establish GPU/tool correctness; it only verifies this engine can start.
  $probeOut=Join-Path $env:RUNNER_TEMP 'lu-01g-engine-help.stdout.txt'
  $probeErr=Join-Path $env:RUNNER_TEMP 'lu-01g-engine-help.stderr.txt'
  $probe=Start-Process -FilePath $engine.FullName -ArgumentList '--help' -PassThru -NoNewWindow -RedirectStandardOutput $probeOut -RedirectStandardError $probeErr
  if(-not $probe.WaitForExit(30000)){
    try{$probe.Kill($true);$null=$probe.WaitForExit(5000)}catch{}
    throw 'Pinned built llama-server.exe did not finish its --help self-test'
  }
  $probe.Refresh()
  if($probe.ExitCode -ne 0){
    throw 'Pinned built llama-server.exe did not pass its native startup-help test'
  }
  if((Get-Item $probeOut).Length -eq 0 -and (Get-Item $probeErr).Length -eq 0){
    throw 'Native llama.cpp engine produced no help/version text'
  }
  $engineHash=(Get-FileHash -LiteralPath $engine.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  $engineSize=[int64]$engine.Length
  $stage=Join-Path $repo 'lu-desktop/src-tauri/resources/lu-inference'
  New-Item -ItemType Directory -Force $stage|Out-Null
  Copy-Item -LiteralPath $engine.FullName -Destination (Join-Path $stage 'llama-server.exe') -Force
  $manifest=[ordered]@{
    schema='lu-inference-package-v1'; engineVariant='cpu'
    backendId='llama-cpu-qwen38-preview'
    release_status='current-source-development-preview-unsigned-not-final'
    upstreamCommit=$upstream; sourceCommit=$SourceSha
    files=@(@{name='llama-server.exe';sha256=$engineHash;bytes=$engineSize})
  }
  $manifestFile=Join-Path $stage 'manifest.json'
  $manifest|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $manifestFile -Encoding utf8
  if((Get-FileHash -LiteralPath (Join-Path $stage 'llama-server.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $engineHash){
    throw 'Staged engine SHA-256 verification failed'
  }
  # Vulkan is a separate source-built executable, not a different label
  # attached to the CPU binary. The actual driver/GPU is only verified on the
  # owner's Radeon PC. A source-pinned SDK must provide glslc and headers.
  $sdk=$env:VULKAN_SDK
  if(-not $sdk -or -not(Test-Path (Join-Path $sdk 'Bin\glslc.exe'))){
    $candidates=@(Get-ChildItem 'C:\VulkanSDK' -Directory -ErrorAction SilentlyContinue |
      Where-Object {Test-Path (Join-Path $_.FullName 'Bin\glslc.exe')})
    if($candidates.Count -ne 1){throw 'Pinned Vulkan SDK absent or ambiguous'}
    $sdk=$candidates[0].FullName
  }
  $env:VULKAN_SDK=$sdk
  $env:PATH=(Join-Path $sdk 'Bin')+';'+$env:PATH
  $glslc=Join-Path $sdk 'Bin\glslc.exe'
  & $glslc --version
  if($LASTEXITCODE -ne 0){throw 'Native Vulkan SDK shader compiler failed'}
  $sharedArgs=@('-S',$vendor,'-B',$vulkanBuild,'-G',$generator,'-A','x64',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',
    '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW','-DBUILD_SHARED_LIBS=OFF',
    '-DGGML_NATIVE=OFF','-DGGML_VULKAN=ON','-DLLAMA_CURL=OFF',
    '-DLLAMA_OPENSSL=OFF','-DLLAMA_BUILD_APP=OFF',
    '-DLLAMA_BUILD_UI=OFF','-DLLAMA_BUILD_TESTS=OFF',
    '-DLLAMA_BUILD_EXAMPLES=OFF','-DLLAMA_BUILD_SERVER=ON')
  & cmake @sharedArgs
  if($LASTEXITCODE -ne 0){throw 'Pinned Vulkan llama.cpp CMake configure failed'}
  & cmake --build $vulkanBuild --config Release --target llama-server --parallel 4
  if($LASTEXITCODE -ne 0){throw 'Pinned Vulkan llama.cpp compile failed'}
  $vkEngines=@(Get-ChildItem -LiteralPath $vulkanBuild -Recurse -File -Filter 'llama-server.exe' |
    Where-Object {$_.FullName -match '(?i)[\\/]Release[\\/]'})
  if($vkEngines.Count -ne 1){throw "Expected one Vulkan Release engine, found $($vkEngines.Count)"}
  $vkEngine=$vkEngines[0]
  if($vkEngine.Length -lt 1000000 -or $vkEngine.Length -gt 250000000){
    throw 'Abnormal Vulkan engine size'
  }
  $vkProbeOut=Join-Path $env:RUNNER_TEMP 'lu-vulkan-help.stdout.txt'
  $vkProbeErr=Join-Path $env:RUNNER_TEMP 'lu-vulkan-help.stderr.txt'
  $vkProbe=Start-Process -FilePath $vkEngine.FullName -ArgumentList '--help' -PassThru -NoNewWindow -RedirectStandardOutput $vkProbeOut -RedirectStandardError $vkProbeErr
  if(-not $vkProbe.WaitForExit(30000)){
    try{$vkProbe.Kill($true);$null=$vkProbe.WaitForExit(5000)}catch{}
    throw 'Source-built Vulkan llama-server.exe did not finish help probe'
  }
  $vkProbe.Refresh()
  if($vkProbe.ExitCode -ne 0 -or
    ((Get-Item $vkProbeOut).Length -eq 0 -and (Get-Item $vkProbeErr).Length -eq 0)){
    throw 'Vulkan engine failed bounded native launch'
  }
  $vkHash=(Get-FileHash -LiteralPath $vkEngine.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  $vkBytes=[int64]$vkEngine.Length
  if($vkHash -ceq $engineHash){throw 'CPU and Vulkan engine must be independent binaries'}
  Copy-Item -LiteralPath $vkEngine.FullName -Destination (Join-Path $stage 'llama-server-vulkan.exe') -Force
  if((Get-FileHash -LiteralPath (Join-Path $stage 'llama-server-vulkan.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $vkHash){
    throw 'Staged Vulkan engine digest mismatch'
  }
  $manifest.engineVariant='cpu-and-vulkan'
  $manifest.backendId='llama-dual-qwen38-preview'
  $manifest.files=@(
    @{name='llama-server.exe';sha256=$engineHash;bytes=$engineSize;backend='cpu'},
    @{name='llama-server-vulkan.exe';sha256=$vkHash;bytes=$vkBytes;backend='vulkan'}
  )
  $manifest|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $manifestFile -Encoding utf8
  Write-Host "Both CPU and Vulkan binaries built and SHA-verified; SDK: $sdk"
  # Ephemeral CI-only test identity. Never save modified config in Git.
  $confPath=Join-Path $repo 'lu-desktop/src-tauri/tauri.conf.json'
  $originalConfig=[IO.File]::ReadAllText($confPath)
  $conf=ConvertFrom-Json -InputObject $originalConfig -AsHashtable
  $conf.productName='Lu Current Development Preview'
  $conf.identifier='org.lu.current.devpreview'
  $conf.version='0.2.0-rc.1'
  $conf.app.windows[0].title='Lu Current Development Preview (Unsigned)'
  $conf.bundle.active=$false
  $conf.bundle.resources=@{
    'resources/lu-inference/llama-server.exe'='lu-inference/llama-server.exe'
    'resources/lu-inference/llama-server-vulkan.exe'='lu-inference/llama-server-vulkan.exe'
    'resources/lu-inference/manifest.json'='lu-inference/manifest.json'
    'resources/lu-native-codecs'='lu-native-codecs'
  }
  $conf|ConvertTo-Json -Depth 32|Set-Content -LiteralPath $confPath -Encoding utf8
  Push-Location (Join-Path $repo 'lu-desktop')
  try{
    if(Test-Path package-lock.json){& npm ci --no-audit --no-fund}
    else{& npm install --no-audit --no-fund}
    if($LASTEXITCODE -ne 0){throw 'npm install failed'}
    & npm run build
    if($LASTEXITCODE -ne 0){throw 'React bundle build failed'}
    & npm run check
    if($LASTEXITCODE -ne 0){throw 'React UI check failed'}
  } finally{Pop-Location}
  # Engine SHA and length are embedded into THIS TEST build at compile time.
  $env:LU_01G_PHYSICAL_CPU_ENGINE_SHA256=$engineHash
  $env:LU_01G_PHYSICAL_CPU_ENGINE_BYTES=[string]$engineSize
  $env:LU_PREVIEW_VULKAN_ENGINE_SHA256=$vkHash
  $env:LU_PREVIEW_VULKAN_ENGINE_BYTES=[string]$vkBytes
  # build.rs/tauri-build already embeds the one reviewed Common Controls v6
  # Win32 manifest. Linking a second custom .res provably breaks MSVC with
  # CVT1100 duplicate RT_MANIFEST / LNK1123. The PE oracle below is mandatory.
  & cargo rustc --release --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --features physical_win_cpu_eval --bin lu-desktop
  if($LASTEXITCODE -ne 0){throw 'Current exact-source Windows Rust build failed'}
  $app=Join-Path $repo 'lu-desktop/src-tauri/target/release/lu-desktop.exe'
  if(-not(Test-Path -LiteralPath $app -PathType Leaf)){throw 'Compiled Lu desktop EXE missing'}
  & python lu-desktop/scripts/verify_win_v6_manifest.py $app
  if($LASTEXITCODE -ne 0){throw 'Windows x64 GUI Common Controls v6 PE verification failed'}
  $appHash=(Get-FileHash -LiteralPath $app -Algorithm SHA256).Hash.ToLowerInvariant()
  $build=[ordered]@{
    schema='lu-current-preview-NSIS-source-pinned-v1'
    scope='EXACT_SOURCE_BUILD_PHYSICAL_ACCEPTANCE_PENDING'
    sourceSha=$SourceSha;sourceRepo='vjmichael/Lu-AI-GUI'
    variant='physical_win_cpu_eval';appIdentity='org.lu.current.devpreview'
    appSha256=$appHash;appBytes=[int64](Get-Item $app).Length
    engineSha256=$engineHash;engineBytes=$engineSize
    vulkanEngineSha256=$vkHash;vulkanEngineBytes=$vkBytes
    engineUpstreamCommit=$upstream;engineBackend='two-source-built-cpu-and-vulkan-gpu-unproven'
    engineDigestEmbeddedIntoApp=$true;modelBundled=$false
    installedUiAcceptance='NOT_TESTED'
    realGgufBrokerAcceptance='NOT_TESTED'
    windowsJobObjectDescendantRecovery='NOT_TESTED'
  }
  $provenance=Join-Path $out 'BUILD_PROVENANCE.json'
  $build|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $provenance -Encoding utf8
  $installer=Join-Path $out ("Lu-Current-Preview-NSIS-"+$SourceSha.Substring(0,12)+".exe")
  $nsi=Join-Path $repo 'lu-desktop/packaging/NSIS_CURRENT_DEV_PREVIEW.nsi'
  $readme=Join-Path $repo 'lu-desktop/packaging/README_CURRENT_DEV_PREVIEW.txt'
  $makensis=Get-Command makensis.exe -ErrorAction SilentlyContinue
  if($null -eq $makensis){
    $candidate=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'NSIS\makensis.exe'
    if(-not(Test-Path $candidate)){throw 'Pinned NSIS makensis.exe missing'}
    $compiler=$candidate
  }else{$compiler=$makensis.Source}
  & $compiler "-DLU_APP_EXE=$app" "-DLU_ENGINE_EXE=$(Join-Path $stage 'llama-server.exe')" "-DLU_VULKAN_ENGINE_EXE=$(Join-Path $stage 'llama-server-vulkan.exe')" "-DLU_ENGINE_MANIFEST=$manifestFile" "-DLU_TEST_README=$readme" "-DLU_CODECS_DIR=$(Join-Path $repo 'lu-desktop/src-tauri/resources/lu-native-codecs')" "-DLU_BUILD_PROVENANCE=$provenance" "-DLU_INSTALLER_OUTPUT=$installer" $nsi
  if($LASTEXITCODE -ne 0 -or -not(Test-Path $installer)){throw 'NSIS failed; no distributable artifact'}
  $installerHash=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
  [ordered]@{
    schema='lu-current-preview-nsis-sha256-v1'
    sourceSha=$SourceSha;installerName=(Split-Path -Leaf $installer)
    installerSha256=$installerHash;installerBytes=[int64](Get-Item $installer).Length
    appSha256=$appHash;engineSha256=$engineHash;vulkanEngineSha256=$vkHash
    genuineInstallerProduced=$true;physicallyInstalledAndTested=$false
    nativeUiEvidence='NOT_TESTED';realModelToolEvidence='NOT_TESTED'
    gpuVulkanEvidence='VULKAN_ENGINE_BUILT_GPU_OFFLOAD_NOT_TESTED';qwen38SupportPinnedEngine=$upstream
  }|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $out 'installer-checksums.json') -Encoding utf8
  Write-Host "NSIS CREATED: $installer"
  Write-Host "SOURCE SHA: $SourceSha; INSTALLER SHA-256: $installerHash"
} finally{
  if($null -ne $originalConfig){
    [IO.File]::WriteAllText((Join-Path $repo 'lu-desktop/src-tauri/tauri.conf.json'),$originalConfig)
  }
  Pop-Location
}
