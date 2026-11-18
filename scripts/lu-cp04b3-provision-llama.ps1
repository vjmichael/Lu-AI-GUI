# Native Windows development acceptance only. Reuses the E1-P07 exact upstream
# source and model digest; no NSIS, provider evidence or browser trust promotion.
param([Parameter(Mandatory=$true)][string]$OutputDir)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Requires native Windows x64'}
if(Test-Path -LiteralPath $OutputDir){throw 'Output directory already exists'}
$root=(New-Item -ItemType Directory -Path $OutputDir).FullName
$work=Join-Path ([IO.Path]::GetTempPath()) ('lu-cp04b3-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work|Out-Null
try {
  $source=Join-Path $work 'source';$build=Join-Path $work 'build'
  $expectedSource='2bc46baa9207d098d03741ac394e7faaf1a955b6'
  git clone --depth 1 --branch b12143 https://github.com/ggml-org/llama.cpp.git $source
  if($LASTEXITCODE -ne 0){throw 'llama.cpp acquisition failed'}
  $actual=(git -C $source rev-parse HEAD).Trim()
  if($LASTEXITCODE -ne 0 -or $actual -cne $expectedSource){throw 'llama.cpp source mismatch'}
  $vswhere=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
  if(-not(Test-Path -LiteralPath $vswhere)){throw 'Missing Visual Studio inventory'}
  $version=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion).Trim()
  if($LASTEXITCODE -ne 0 -or -not $version){throw 'No C++ toolchain'}
  $generator=switch([int]($version.Split('.')[0])){17 {'Visual Studio 17 2022'} 18 {'Visual Studio 18 2026'} default {throw 'Unsupported Visual Studio generator'}}
  cmake -S $source -B $build -G $generator -A x64 -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DCMAKE_POLICY_DEFAULT_CMP0091=NEW -DBUILD_SHARED_LIBS=OFF -DGGML_NATIVE=OFF -DGGML_VULKAN=OFF -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_APP=OFF -DLLAMA_BUILD_UI=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=ON
  if($LASTEXITCODE -ne 0){throw 'llama.cpp configure failed'}
  cmake --build $build --config Release --target llama-server --parallel 3
  if($LASTEXITCODE -ne 0){throw 'llama.cpp build failed'}
  $engines=@(Get-ChildItem -LiteralPath (Join-Path $build 'bin') -Recurse -File -Filter 'llama-server.exe')
  if($engines.Count -ne 1 -or $engines[0].Length -lt 1000000){throw 'Expected exactly one genuine built server'}
  $engine=Join-Path $root 'llama-server.exe';Copy-Item -LiteralPath $engines[0].FullName -Destination $engine
  # Preserve any build-emitted adjacent DLLs, no arbitrary system DLL copies.
  Get-ChildItem -LiteralPath $engines[0].DirectoryName -File -Filter '*.dll'|ForEach-Object{Copy-Item -LiteralPath $_.FullName -Destination $root}
  $model=Join-Path $root 'Qwen3-0.6B.Q4_K_M.gguf'
  $expectedModel='7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d'
  curl.exe -fL --retry 3 --connect-timeout 30 --max-time 600 'https://huggingface.co/QuantFactory/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B.Q4_K_M.gguf' -o $model
  if($LASTEXITCODE -ne 0){throw 'GGUF download failed'}
  $modelHash=(Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash.ToLowerInvariant()
  if($modelHash -cne $expectedModel){throw 'Pinned GGUF mismatch'}
  $hash=(Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash.ToLowerInvariant()
  $files=@(Get-ChildItem -LiteralPath $root -File|Sort-Object Name|ForEach-Object{@{path=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}})
  @{schema='lu-cp04b3-engine-inputs-v1';sourceSha=$actual;engineSha256=$hash;modelSha256=$modelHash;files=$files;scope='development-only-not-browser-approved'}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $root 'inputs.json') -Encoding utf8
  python scripts/lu-cp04b3-engine-smoke.py --engine $engine --model $model --engine-sha256 $hash --model-sha256 $expectedModel --evidence (Join-Path $root 'smoke.json')
  if($LASTEXITCODE -ne 0){throw 'Genuine GGUF smoke failed'}
} finally {Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue}
