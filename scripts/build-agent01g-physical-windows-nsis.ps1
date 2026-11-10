# Native Windows x64 only. Exact head, source-pinned llama.cpp CPU engine.
# Unsigned, isolated physical-acceptance installer. NEVER a Lu alpha release.
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
  $upstream='7fe450e19305b828c199d602c23a8337aaa1f03b'
  $vendor=Join-Path $env:RUNNER_TEMP 'lu-01g-llama-source'
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
  $engineBuild=Join-Path $env:RUNNER_TEMP 'lu-01g-llama-build'
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
    backendId='llama-cpu-physical-eval'
    release_status='unsigned-test-only-not-alpha'
    upstreamCommit=$upstream; sourceCommit=$SourceSha
    files=@(@{name='llama-server.exe';sha256=$engineHash;bytes=$engineSize})
  }
  $manifestFile=Join-Path $stage 'manifest.json'
  $manifest|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $manifestFile -Encoding utf8
  if((Get-FileHash -LiteralPath (Join-Path $stage 'llama-server.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $engineHash){
    throw 'Staged engine SHA-256 verification failed'
  }
  # Ephemeral CI-only test identity. Never save modified config in Git.
  $confPath=Join-Path $repo 'lu-desktop/src-tauri/tauri.conf.json'
  $originalConfig=[IO.File]::ReadAllText($confPath)
  $conf=ConvertFrom-Json -InputObject $originalConfig -AsHashtable
  $conf.productName='Lu AGENT-01G Physical Windows CPU Test'
  $conf.identifier='org.lu.agent01g.devtest'
  $conf.app.windows[0].title='Lu AGENT-01G CPU TEST (NOT ALPHA)'
  $conf.bundle.active=$false
  $conf.bundle.resources=@{
    'resources/lu-inference/llama-server.exe'='lu-inference/llama-server.exe'
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
  $manifestRes=Join-Path $env:RUNNER_TEMP 'lu-01g-common-controls-v6.res'
  & python lu-desktop/scripts/build_win_manifest_res.py lu-desktop/scripts/lu-desktop.manifest $manifestRes
  if($LASTEXITCODE -ne 0){throw 'Windows manifest resource compilation failed'}
  & cargo rustc --release --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --features physical_win_cpu_eval --bin lu-desktop -- -C 'link-arg=/manifest:no' -C "link-arg=$manifestRes"
  if($LASTEXITCODE -ne 0){throw 'Current exact-source Windows Rust build failed'}
  $app=Join-Path $repo 'lu-desktop/src-tauri/target/release/lu-desktop.exe'
  if(-not(Test-Path -LiteralPath $app -PathType Leaf)){throw 'Compiled Lu desktop EXE missing'}
  & python lu-desktop/scripts/verify_win_v6_manifest.py $app
  if($LASTEXITCODE -ne 0){throw 'Windows x64 GUI Common Controls v6 PE verification failed'}
  $appHash=(Get-FileHash -LiteralPath $app -Algorithm SHA256).Hash.ToLowerInvariant()
  $build=[ordered]@{
    schema='lu-agent01g-physical-windows-cpu-installer-v1'
    scope='EXACT_SOURCE_BUILD_PHYSICAL_ACCEPTANCE_PENDING'
    sourceSha=$SourceSha;sourceRepo='vjmichael/Lu-AI-GUI'
    variant='physical_win_cpu_eval';appIdentity='org.lu.agent01g.devtest'
    appSha256=$appHash;appBytes=[int64](Get-Item $app).Length
    engineSha256=$engineHash;engineBytes=$engineSize
    engineUpstreamCommit=$upstream;engineBackend='cpu-no-vulkan-proof'
    engineDigestEmbeddedIntoApp=$true;modelBundled=$false
    installedUiAcceptance='NOT_TESTED'
    realGgufBrokerAcceptance='NOT_TESTED'
    windowsJobObjectDescendantRecovery='NOT_TESTED'
  }
  $provenance=Join-Path $out 'BUILD_PROVENANCE.json'
  $build|ConvertTo-Json -Depth 7|Set-Content -LiteralPath $provenance -Encoding utf8
  $installer=Join-Path $out ("Lu-AGENT01G-Physical-Windows-CPU-Test-"+$SourceSha.Substring(0,12)+".exe")
  $nsi=Join-Path $repo 'lu-desktop/packaging/NSIS_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.nsi'
  $readme=Join-Path $repo 'lu-desktop/packaging/README_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.txt'
  $makensis=Get-Command makensis.exe -ErrorAction SilentlyContinue
  if($null -eq $makensis){
    $candidate=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'NSIS\makensis.exe'
    if(-not(Test-Path $candidate)){throw 'Pinned NSIS makensis.exe missing'}
    $compiler=$candidate
  }else{$compiler=$makensis.Source}
  & $compiler "-DLU_APP_EXE=$app" "-DLU_ENGINE_EXE=$(Join-Path $stage 'llama-server.exe')" "-DLU_ENGINE_MANIFEST=$manifestFile" "-DLU_TEST_README=$readme" "-DLU_BUILD_PROVENANCE=$provenance" "-DLU_INSTALLER_OUTPUT=$installer" $nsi
  if($LASTEXITCODE -ne 0 -or -not(Test-Path $installer)){throw 'NSIS failed; no distributable artifact'}
  $installerHash=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
  [ordered]@{
    schema='lu-agent01g-built-nsis-sha256-v1'
    sourceSha=$SourceSha;installerName=(Split-Path -Leaf $installer)
    installerSha256=$installerHash;installerBytes=[int64](Get-Item $installer).Length
    appSha256=$appHash;engineSha256=$engineHash
    genuineInstallerProduced=$true;physicallyInstalledAndTested=$false
    nativeUiEvidence='NOT_TESTED';realModelToolEvidence='NOT_TESTED'
    gpuVulkanEvidence='NOT_TESTED_CPU_BUILD'
  }|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $out 'installer-checksums.json') -Encoding utf8
  Write-Host "NSIS CREATED: $installer"
  Write-Host "SOURCE SHA: $SourceSha; INSTALLER SHA-256: $installerHash"
} finally{
  if($null -ne $originalConfig){
    [IO.File]::WriteAllText((Join-Path $repo 'lu-desktop/src-tauri/tauri.conf.json'),$originalConfig)
  }
  Pop-Location
}
