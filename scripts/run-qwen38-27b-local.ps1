# Lu Current Development Preview — optional local Qwen3.8-27B runner.
# Does not modify Lu installation, bypass its sealed engine digest, or grant tools.
# Run in PowerShell on the user's Windows PC. No Docker or cloud inference.
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$ModelPath,
 [string]$ServerPath="$env:LOCALAPPDATA\Programs\LuCurrentPreview\lu-inference\llama-server.exe",
 [string]$MmprojPath="",
 [switch]$BuildVulkan,
 [int]$Context=8192,
 [int]$GpuLayers=99,
 [int]$Port=18972,
 [string]$EvidenceDirectory="$env:USERPROFILE\Documents\Lu-Qwen38-Local-Test"
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$upstream='e0f142d5ddc1461ce721644d92af33db23545105'
$repo='https://github.com/ggml-org/llama.cpp.git'
if($Context -lt 2048 -or $Context -gt 131072){throw "Context must be 2048..131072"}
if($GpuLayers -lt 0 -or $GpuLayers -gt 999){throw "Invalid GPU layer count"}
if($Port -lt 1024 -or $Port -gt 65535){throw "Invalid local port"}
$model=(Resolve-Path -LiteralPath $ModelPath -ErrorAction Stop).Path
if(-not $model.EndsWith('.gguf',[StringComparison]::OrdinalIgnoreCase)){throw 'Choose a .gguf model'}
$meta=Get-Item -LiteralPath $model
if($meta.Length -lt 1000000000){throw 'Model is unexpectedly small; verify exact 27B GGUF'}
$mmproj=$null
if($MmprojPath){
  $mmproj=(Resolve-Path -LiteralPath $MmprojPath -ErrorAction Stop).Path
  if(-not $mmproj.EndsWith('.gguf',[StringComparison]::OrdinalIgnoreCase)){throw 'MMProj must be a GGUF'}
}
New-Item -ItemType Directory -Force -Path $EvidenceDirectory|Out-Null
$evidence=[IO.Path]::GetFullPath($EvidenceDirectory)
if($BuildVulkan){
  if(-not $env:VULKAN_SDK -or -not (Test-Path -LiteralPath (Join-Path $env:VULKAN_SDK 'Bin\glslc.exe'))){
    throw 'Install the official Vulkan SDK, then reopen PowerShell (VULKAN_SDK must be set)'
  }
  foreach($cmd in @('git','cmake')){if(-not(Get-Command $cmd -ErrorAction SilentlyContinue)){throw "Missing $cmd"}}
  $sources=Join-Path $evidence 'pinned-llama-source'
  $build=Join-Path $evidence 'pinned-llama-vulkan-build'
  if(-not(Test-Path $sources)){
    New-Item -ItemType Directory -Force -Path $sources|Out-Null
    & git -C $sources init -q
    & git -C $sources remote add origin $repo
    & git -C $sources fetch --depth=1 origin $upstream
    if($LASTEXITCODE -ne 0){throw 'Failed to fetch pinned llama.cpp source'}
    & git -C $sources checkout --detach -q FETCH_HEAD
    if($LASTEXITCODE -ne 0){throw 'Failed to check out pinned llama.cpp source'}
  }
  if((& git -C $sources rev-parse HEAD).Trim() -cne $upstream){
    throw 'Pinned source mismatch; do not reuse unknown local engine'
  }
  # Use the installed MSVC CMake generator, no Docker or implicit MinGW.
  $vswhere=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio\Installer\vswhere.exe'
  if(-not (Test-Path $vswhere)){throw 'Install Visual Studio C++ Desktop Build Tools'}
  $vsPath=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath).Trim()
  $ver=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion).Trim()
  if(-not $vsPath){throw 'Native MSVC C++ toolchain missing'}
  $generator=switch([int]($ver.Split('.')[0])){
    17 {'Visual Studio 17 2022'}
    18 {'Visual Studio 18 2026'}
    default {throw 'Unsupported MSVC generator'}
  }
  & cmake -S $sources -B $build -G $generator -A x64 -DGGML_VULKAN=ON -DGGML_NATIVE=OFF -DLLAMA_CURL=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_SERVER=ON -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DCMAKE_POLICY_DEFAULT_CMP0091=NEW
  if($LASTEXITCODE -ne 0){throw 'Vulkan CMake configuration failed'}
  & cmake --build $build --config Release --target llama-server --parallel 4
  if($LASTEXITCODE -ne 0){throw 'Vulkan llama.cpp compile failed'}
  $server=@(Get-ChildItem -LiteralPath $build -Recurse -File -Filter 'llama-server.exe'|
    Where-Object{$_.FullName -match '(?i)[\\/]Release[\\/]'})
  if($server.Count -ne 1){throw 'Expected exactly one pinned Vulkan release engine'}
  $ServerPath=$server[0].FullName
}
$server=(Resolve-Path -LiteralPath $ServerPath -ErrorAction Stop).Path
if(-not $server.EndsWith('.exe',[StringComparison]::OrdinalIgnoreCase)){throw 'Engine must be a native .exe'}
$engineHash=(Get-FileHash -LiteralPath $server -Algorithm SHA256).Hash.ToLowerInvariant()
$modelHash=(Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash.ToLowerInvariant()
$mmprojHash=if($mmproj){(Get-FileHash -LiteralPath $mmproj -Algorithm SHA256).Hash.ToLowerInvariant()}else{$null}
Write-Host "Pinned compatible llama.cpp source recommended: $upstream"
Write-Host "Model: $([IO.Path]::GetFileName($model)) ($([math]::Round($meta.Length/1GB,2)) GiB)"
Write-Host "Server SHA256: $engineHash"
Write-Host "Model SHA256: $modelHash"
Write-Host "GPU layers requested: $GpuLayers; this does NOT prove GPU use."
$probe=Join-Path $evidence 'device-detection.txt'
& $server --list-devices *> $probe
# Some builds enumerate devices with nonzero exit. Evidence is preserved.
$devices=if(Test-Path $probe){Get-Content -LiteralPath $probe -Raw}else{''}
if($GpuLayers -gt 0 -and $devices -notmatch '(?i)vulkan|AMD|Radeon'){
  Write-Warning 'No GPU backend in detected devices. This is likely the CPU preview engine. Use -BuildVulkan with the Vulkan SDK to test AMD offload.'
}
$args=@('-m',$model,'--host','127.0.0.1','--port',[string]$Port,'-c',[string]$Context,'-ngl',[string]$GpuLayers,'--jinja','--cache-type-k','q8_0','--cache-type-v','q8_0','-fa','on')
if($mmproj){$args+=@('--mmproj',$mmproj)}
$out=Join-Path $evidence 'llama-server.stdout.log'
$err=Join-Path $evidence 'llama-server.stderr.log'
# Windows Start-Process joins ArgumentList into one command line. Quote
# paths containing spaces before subprocess creation; do not use Invoke-Expression.
$quotedArgs=@($args | ForEach-Object {
  $value=[string]$_
  if($value -match '[\s"]'){'"'+($value -replace '"','\\"')+'"'}else{$value}
})
$proc=Start-Process -FilePath $server -ArgumentList $quotedArgs -RedirectStandardOutput $out -RedirectStandardError $err -PassThru -NoNewWindow
try{
  $ready=$false
  for($i=0;$i -lt 120;$i++){
    $proc.Refresh()
    if($proc.HasExited){throw "llama-server exited $($proc.ExitCode). See $err"}
    try {
      $health=Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -Method Get -TimeoutSec 3
      if($health.status -eq 'ok'){$ready=$true;break}
    } catch{}
    Start-Sleep -Seconds 2
  }
  if(-not $ready){throw 'Model not ready; inspect server logs and GPU/VRAM status'}
  $payload=@{
    model='local';messages=@(@{role='user';content='In one sentence, state that this is a local Qwen3.8-27B response.'});
    stream=$false;max_tokens=96;temperature=0.1
  }|ConvertTo-Json -Depth 8
  $answer=Invoke-RestMethod -Uri "http://127.0.0.1:$Port/v1/chat/completions" -Method Post -ContentType 'application/json' -Body $payload -TimeoutSec 180
  $text=([string]$answer.choices[0].message.content)
  if([string]::IsNullOrWhiteSpace($text)){throw 'Model response absent or empty'}
  [ordered]@{
    schema='lu-local-qwen38-27b-physical-v1';timestamp=(Get-Date).ToUniversalTime().ToString('o')
    result='REAL_MODEL_RESPONSE_OBSERVED';modelFile=[IO.Path]::GetFileName($model)
    modelSha256=$modelHash;engineSha256=$engineHash;mmprojSha256=$mmprojHash
    requestedGpuLayers=$GpuLayers;gpuActual='CHECK_SERVER_LOG_NOT_AUTOMATICALLY_PROVEN'
    response=$text;nativeToolAndPlanExecution='NOT_TESTED'
    installedLuIntegration='NOT_TESTED_BY_STANDALONE_SERVER'
  }|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $evidence 'qwen38-local-result.json') -Encoding utf8
  Write-Host "Real local model response: $text"
  Write-Host "Evidence: $evidence"
  Write-Host 'Test Lu separately by importing this GGUF in Lu Current Preview; no unattended tool grants.'
}finally{
  if($proc -and -not $proc.HasExited){
    try{$proc.Kill($true);$proc.WaitForExit(10000)|Out-Null}catch{}
  }
}
