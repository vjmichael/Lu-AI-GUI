$ErrorActionPreference='Stop'
$expected='3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff'
$root=Join-Path $env:RUNNER_TEMP 'lu-llvm18-owner-source'
git clone --depth 1 --filter=blob:none --branch llvmorg-18.1.8 --single-branch https://github.com/llvm/llvm-project.git $root
if ($LASTEXITCODE -ne 0) { throw 'DENIED: LLVM source checkout failed' }
if ((& git -C $root rev-parse HEAD).Trim() -ne $expected) { throw 'DENIED: LLVM source commit mismatch' }
python ide/llvm18_owner/apply_llvm18_owner_hook.py $root
if ($LASTEXITCODE -ne 0) { throw 'DENIED: LLVM owner hook overlay failed' }
$out=Join-Path $env:RUNNER_TEMP 'lu-llvm18-owner-native'
$flags=@('-S',(Join-Path $root 'llvm'),'-B',$out,'-G','Visual Studio 17 2022','-A','x64','-DLLVM_ENABLE_PROJECTS=clang;lldb','-DLLVM_TARGETS_TO_BUILD=X86','-DLLDB_ENABLE_PYTHON=OFF','-DLLDB_ENABLE_CURSES=OFF','-DLLVM_ENABLE_ZLIB=OFF','-DLLVM_ENABLE_LIBXML2=OFF','-DLLVM_INCLUDE_TESTS=OFF','-DLLDB_INCLUDE_TESTS=OFF')
& cmake @flags
if ($LASTEXITCODE -ne 0) { throw 'DENIED: actual Windows LLVM configuration failed' }
& cmake --build $out --config Release --target lldb-dap llvm-symbolizer clang --parallel 2
if ($LASTEXITCODE -ne 0) { throw 'DENIED: actual patched Windows LLDB build failed' }
$bin=Join-Path $out 'Release\bin'
if (!(Test-Path (Join-Path $bin 'lldb-dap.exe'))) { $bin=Join-Path $out 'bin\Release' }
if (!(Test-Path (Join-Path $bin 'lldb-dap.exe'))) { throw 'DENIED: linked patched LLDB-DAP missing' }
$bundle=Join-Path $env:RUNNER_TEMP 'lu-verified-owner-bundle'
New-Item -ItemType Directory -Force (Join-Path $bundle 'bin') | Out-Null
New-Item -ItemType Directory -Force (Join-Path $bundle 'debuggee') | Out-Null
Copy-Item (Join-Path $bin 'lldb-dap.exe') (Join-Path $bundle 'bin\lldb-dap.exe')
Copy-Item (Join-Path $bin 'llvm-symbolizer.exe') (Join-Path $bundle 'bin\llvm-symbolizer.exe')
Get-ChildItem $bin -Filter '*.dll' -File | Copy-Item -Destination (Join-Path $bundle 'bin')
& (Join-Path $bin 'clang.exe') -g -O0 -o (Join-Path $bundle 'debuggee\wo6-selftest.exe') ide/supervisor/fixtures/debug/wo6_selftest.c
if ($LASTEXITCODE -ne 0) { throw 'DENIED: actual first-party fixed debuggee build failed' }
# FIN-04R first-party provenance for an opt-in READ-ONLY Windows owner
# observer. This inventory is NOT an installed/kernel acceptance receipt;
# the independent native OS gate remains mandatory.
$patched=Join-Path $root 'lldb/source/Plugins/Process/Windows/Common/DebuggerThread.cpp'
# Prior native MSVC job 130020178608 failed C2039/C3861 because the pinned
# ProgressEvent.h uses std::chrono::system_clock without a direct <chrono>
# include. Require the exact anchored repair and record its source digest.
$progress=Join-Path $root 'lldb/tools/lldb-dap/ProgressEvent.h'
$progressText=Get-Content -LiteralPath $progress -Raw
if (!$progressText.Contains('#include <chrono>') -or
    !$progressText.Contains('std::chrono::system_clock::now()')) {
  throw 'DENIED: pinned LLVM DAP chrono compile repair absent'
}
$provenanceDirectory=Join-Path $bundle 'provenance'
New-Item -ItemType Directory -Force $provenanceDirectory | Out-Null
Copy-Item -LiteralPath $patched -Destination (Join-Path $provenanceDirectory 'DebuggerThread.cpp')
$sha256={ param([string]$p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }
$ownerProvenance=[ordered]@{
  schema='lu.ide.fin04.first-party-lldb-owner.v1'
  upstreamCommit=$expected
  observerOnly=$true
  productionDebuggerActions=$false
  replayAuthorized=$false
  headerSha256=(& $sha256 'ide/llvm18_owner/LuIdeOwnerEvent.h')
  patcherSha256=(& $sha256 'ide/llvm18_owner/apply_llvm18_owner_hook.py')
  patchedDebuggerThreadSha256=(& $sha256 $patched)
  lldbDapExeSha256=(& $sha256 (Join-Path $bin 'lldb-dap.exe'))
}
# Windows PowerShell 5's -Encoding UTF8 emits a BOM, which serde_json
# rejects as a provenance prefix. Emit explicitly BOM-free UTF-8.
$provenanceFile=Join-Path $bundle 'fin04-owner-provenance.json'
$utf8NoBom=New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText(
  $provenanceFile,($ownerProvenance | ConvertTo-Json -Depth 3),$utf8NoBom
)
$lock=[ordered]@{schema='lu-ide-d5-ci-only-v1'; llvmCommit=$expected; lldbSha256=(Get-FileHash (Join-Path $bin 'lldb-dap.exe') -Algorithm SHA256).Hash; sourceSha256=(Get-FileHash (Join-Path $root 'lldb/source/Plugins/Process/Windows/Common/DebuggerThread.cpp') -Algorithm SHA256).Hash; releaseApproved=$false}
$lock | ConvertTo-Json | Set-Content (Join-Path $env:RUNNER_TEMP 'lu-owner-llvm18-lock.json')
Add-Content $env:GITHUB_ENV "LU_IDE_WO6_DAP_BUNDLE=$bundle"
Write-Host 'DEV045D5_NATIVE_PATCHED_LLDB_COMPILE=PASS_NOT_INSTALLED_ACCEPTANCE'
