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
$lock=[ordered]@{schema='lu-ide-d5-ci-only-v1'; llvmCommit=$expected; lldbSha256=(Get-FileHash (Join-Path $bin 'lldb-dap.exe') -Algorithm SHA256).Hash; sourceSha256=(Get-FileHash (Join-Path $root 'lldb/source/Plugins/Process/Windows/Common/DebuggerThread.cpp') -Algorithm SHA256).Hash; releaseApproved=$false}
$lock | ConvertTo-Json | Set-Content (Join-Path $env:RUNNER_TEMP 'lu-owner-llvm18-lock.json')
Add-Content $env:GITHUB_ENV "LU_IDE_WO6_DAP_BUNDLE=$bundle"
Write-Host 'DEV045D5_NATIVE_PATCHED_LLDB_COMPILE=PASS_NOT_INSTALLED_ACCEPTANCE'
