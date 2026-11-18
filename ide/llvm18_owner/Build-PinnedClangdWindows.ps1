# CI-only exact source rebuild. Never use the ambient clangd or widen ACLs.
$ErrorActionPreference='Stop'
$pin='87f0227cb60147a26a1eeb4fb06e3b505e9c7261'
$root=Join-Path $env:RUNNER_TEMP 'lu-clangd20-source'
$out=Join-Path $env:RUNNER_TEMP 'lu-clangd20-build'
$bundle=Join-Path $env:RUNNER_TEMP 'e2-clangd-bundle'
git -c core.autocrlf=false clone --depth 1 --filter=blob:none --branch llvmorg-20.1.8 --single-branch https://github.com/llvm/llvm-project.git $root
if ($LASTEXITCODE -ne 0 -or ((& git -C $root rev-parse HEAD).Trim() -ne $pin)) { throw 'DENIED: clangd source pin mismatch' }
python ide/llvm18_owner/apply_clangd20_windows_startup.py $root
if ($LASTEXITCODE -ne 0) { throw 'DENIED: clangd exact startup overlay failed' }
$changed=@(& git -C $root diff --name-only)
if ($LASTEXITCODE -ne 0 -or $changed.Count -ne 1 -or $changed[0] -cne 'llvm/lib/Support/Windows/Process.inc') { throw 'DENIED: clangd source delta not isolated' }
$flags=@('-S',(Join-Path $root 'llvm'),'-B',$out,'-G','Visual Studio 17 2022','-A','x64','-DLLVM_ENABLE_PROJECTS=clang;clang-tools-extra','-DLLVM_TARGETS_TO_BUILD=X86','-DLLVM_ENABLE_ZLIB=OFF','-DLLVM_ENABLE_ZSTD=OFF','-DLLVM_ENABLE_LIBXML2=OFF','-DLLVM_INCLUDE_TESTS=OFF','-DCLANG_INCLUDE_TESTS=OFF','-DCLANG_TOOLS_EXTRA_INCLUDE_TESTS=OFF')
& cmake @flags
if ($LASTEXITCODE -ne 0) { throw 'DENIED: clangd source configuration failed' }
& cmake --build $out --config Release --target clangd llvm-readobj clang-resource-headers --parallel 2
if ($LASTEXITCODE -ne 0) { throw 'DENIED: source-pinned clangd build failed' }
$bin=Join-Path $out 'Release/bin'
if (!(Test-Path (Join-Path $bin 'clangd.exe'))) { $bin=Join-Path $out 'bin/Release' }
if (!(Test-Path (Join-Path $bin 'clangd.exe') -PathType Leaf) -or !(Test-Path (Join-Path $bin 'llvm-readobj.exe') -PathType Leaf)) { throw 'DENIED: native clangd or import reader missing' }
if (Test-Path -LiteralPath $bundle) { throw 'DENIED: clangd bundle must be fresh' }
New-Item -ItemType Directory -Path (Join-Path $bundle 'bin') | Out-Null
Copy-Item -LiteralPath (Join-Path $bin 'clangd.exe') -Destination (Join-Path $bundle 'bin')
Get-ChildItem -LiteralPath $bin -Filter '*.dll' -File | Copy-Item -Destination (Join-Path $bundle 'bin')
# clangd resolves built-in headers relative to its own executable. Copy only
# the build's exact major-version resource directory, never ambient PATH files.
$resource=Join-Path (Split-Path $bin -Parent) 'lib/clang/20'
if (!(Test-Path (Join-Path $resource 'include/stddef.h') -PathType Leaf)) { $resource=Join-Path $out 'lib/clang/20' }
if (!(Test-Path (Join-Path $resource 'include/stddef.h') -PathType Leaf)) { throw 'DENIED: clangd built-in headers missing' }
New-Item -ItemType Directory -Force -Path (Join-Path $bundle 'lib/clang') | Out-Null
Copy-Item -LiteralPath $resource -Destination (Join-Path $bundle 'lib/clang/20') -Recurse
$sha={ param([string]$p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }
$receipt=[ordered]@{schema='lu.ide.clangd20-ci-source.v1';upstreamCommit=$pin;patchedProcessSha256=(& $sha (Join-Path $root 'llvm/lib/Support/Windows/Process.inc'));patcherSha256=(& $sha 'ide/llvm18_owner/apply_clangd20_windows_startup.py');clangdSha256=(& $sha (Join-Path $bundle 'bin/clangd.exe'));releaseApproved=$false;nativeAccepted=$false}
[IO.File]::WriteAllText((Join-Path $bundle 'clangd-source-provenance.json'),($receipt | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
Add-Content -LiteralPath $env:GITHUB_ENV -Value ('LU_IDE_E2_PINNED_LSP_BUNDLE='+$bundle)
Add-Content -LiteralPath $env:GITHUB_ENV -Value ('LU_IDE_E2_PINNED_READOBJ='+(Join-Path $bin 'llvm-readobj.exe'))
Write-Host 'E2_SOURCE_PINNED_CLANGD_BUILD=PASS_NOT_NATIVE_ACCEPTANCE'
