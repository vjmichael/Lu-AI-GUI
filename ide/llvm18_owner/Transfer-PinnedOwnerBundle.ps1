# Exact-source transport between two existing native CI workflows.
# This is artifact custody only, NOT a native debugger or release pass.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][ValidateSet('Export','Receive','SelfTest')][string]$Mode)
$ErrorActionPreference='Stop'
$llvmPin='3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff'
$head=(& git rev-parse HEAD).Trim()
if ($Mode -ne 'SelfTest' -and ($LASTEXITCODE -ne 0 -or $head -notmatch '^[0-9a-f]{40}$' -or
    $env:GITHUB_REPOSITORY -ne 'vjmichael/Lu-AI-GUI' -or
    $env:GITHUB_ACTIONS -ne 'true')) {
    throw 'DENIED: exact repository/source-bound CI transport required'
}
$patcherSha=(Get-FileHash -LiteralPath 'ide/llvm18_owner/apply_llvm18_owner_hook.py' -Algorithm SHA256).Hash.ToLowerInvariant()
$bundleName='lu-verified-owner-bundle'
$manifestName='lu-owner-llvm18-transport.json'
function Inventory([string]$root) {
    if (!(Test-Path -LiteralPath $root -PathType Container)) { throw 'DENIED: native bundle missing' }
    $rootItem=Get-Item -LiteralPath $root -Force
    if ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'DENIED: native artifact root reparse point'
    }
    $all=@(Get-ChildItem -LiteralPath $root -Recurse -Force)
    if ($all.Count -gt 1000 -or @($all | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count -ne 0) {
        throw 'DENIED: native artifact inventory bounds or reparse point'
    }
    $files=@($all | Where-Object { !$_.PSIsContainer })
    if ($files.Count -lt 4 -or $files.Count -gt 500) { throw 'DENIED: native artifact file count' }
    $total=0L
    $entries=@(foreach ($file in $files) {
        $relative=[IO.Path]::GetRelativePath($root,$file.FullName).Replace('\','/')
        if ($relative -notmatch '^[a-zA-Z0-9._/-]+$' -or $relative -match '(^|/)\.\.?(/|$)' -or $relative.StartsWith('/')) {
            throw 'DENIED: native artifact relative path'
        }
        if ($file.Length -le 0 -or $file.Length -gt 256MB) { throw 'DENIED: native artifact per-file bound' }
        $total += $file.Length
        [pscustomobject][ordered]@{path=$relative;bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    if ($total -gt 512MB) { throw 'DENIED: native artifact total bound' }
    $unique=@($entries | Group-Object -Property { $_.path.ToLowerInvariant() })
    if ($unique.Count -ne $entries.Count) { throw 'DENIED: native artifact case-folded alias' }
    return @($entries | Sort-Object -Property path)
}
# A ZIP's traversal order is not its identity. OrderedDictionary on export and
# PSCustomObject on JSON import can sort differently. Bind each exact ordinal
# path to its own byte length and hash instead of comparing positions.
function Assert-Inventory([object[]]$actual,[object[]]$expected) {
    if ($actual.Count -ne $expected.Count) { throw 'DENIED: native artifact file count changed' }
    $remaining=[System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    $folded=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in $actual) {
        if (!$folded.Add($entry.path)) { throw 'DENIED: native artifact case-folded alias' }
        $remaining.Add($entry.path,$entry)
    }
    foreach ($entry in $expected) {
        if ($entry.path -isnot [string] -or !$remaining.ContainsKey($entry.path)) {
            throw 'DENIED: native artifact missing, duplicate or changed path'
        }
        $measured=$remaining[$entry.path]
        if (($entry.bytes -isnot [int] -and $entry.bytes -isnot [long]) -or
            $entry.bytes -ne $measured.bytes) {
            throw 'DENIED: native artifact length changed'
        }
        if ($entry.sha256 -isnot [string] -or $entry.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            $entry.sha256 -cne $measured.sha256) {
            throw 'DENIED: native artifact SHA256 changed'
        }
        [void]$remaining.Remove($entry.path)
    }
    if ($remaining.Count -ne 0) { throw 'DENIED: native artifact unlisted paths' }
}
function RequireInventoryDenial([object[]]$actual,[object[]]$expected) {
    $denied=$false
    try { Assert-Inventory $actual $expected } catch { $denied=$true }
    if (!$denied) { throw 'DENIED: transport negative test accepted altered inventory' }
}
if ($Mode -eq 'SelfTest') {
    $a=[pscustomobject][ordered]@{path='bin/a.exe';bytes=12L;sha256=('a'*64)}
    $b=[pscustomobject][ordered]@{path='provenance/b.cpp';bytes=20L;sha256=('b'*64)}
    $actual=@($a,$b)
    $reordered=@((@($b,$a) | ConvertTo-Json -Depth 3 | ConvertFrom-Json))
    Assert-Inventory $actual $reordered
    RequireInventoryDenial $actual @($a)
    RequireInventoryDenial $actual @($a,$a)
    RequireInventoryDenial $actual @($a,[pscustomobject]@{path='Provenance/b.cpp';bytes=20L;sha256=('b'*64)})
    RequireInventoryDenial $actual @($a,[pscustomobject]@{path=$b.path;bytes=21L;sha256=$b.sha256})
    RequireInventoryDenial $actual @($a,[pscustomobject]@{path=$b.path;bytes=$b.bytes;sha256=('c'*64)})
    RequireInventoryDenial @($a,[pscustomobject]@{path='BIN/A.EXE';bytes=12L;sha256=$a.sha256}) @($a,$b)
    Write-Host 'LLVM18_EXACT_INVENTORY_ORDER_AND_TAMPER_SELFTEST=PASS_NOT_NATIVE_ACCEPTANCE'
    exit 0
}
function RequireBundle([string]$root) {
    # Inventory FIRST: no reparse file or root may be parsed or hashed.
    [void](Inventory $root)
    foreach ($relative in @('bin/lldb-dap.exe','bin/llvm-symbolizer.exe','debuggee/wo6-selftest.exe','provenance/Process.inc','provenance/DebuggerThread.cpp','provenance/ProgressEvent.h','fin04-owner-provenance.json')) {
        if (!(Test-Path -LiteralPath (Join-Path $root $relative) -PathType Leaf)) { throw 'DENIED: pinned native artifact incomplete' }
    }
    $source=Get-Content -LiteralPath (Join-Path $root 'provenance/Process.inc') -Raw
    if (!$source.Contains('LU_LLVM18_FIXED_DAP_IMAGE_LEAF_USED') -or
        !$source.Contains('Error != ERROR_ACCESS_DENIED') -or
        !$source.Contains('(!Dap && !Symbolizer)')) {
        throw 'DENIED: reviewed native startup repair absent from artifact'
    }
    $native=Get-Content -LiteralPath (Join-Path $root 'fin04-owner-provenance.json') -Raw | ConvertFrom-Json
    $binarySha=(Get-FileHash -LiteralPath (Join-Path $root 'bin/lldb-dap.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    $headerSha=(Get-FileHash -LiteralPath 'ide/llvm18_owner/LuIdeOwnerEvent.h' -Algorithm SHA256).Hash.ToLowerInvariant()
    $threadSha=(Get-FileHash -LiteralPath (Join-Path $root 'provenance/DebuggerThread.cpp') -Algorithm SHA256).Hash.ToLowerInvariant()
    $progressSha=(Get-FileHash -LiteralPath (Join-Path $root 'provenance/ProgressEvent.h') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($native.schema -cne 'lu.ide.fin04.first-party-lldb-owner.v1' -or
        $native.upstreamCommit -cne $llvmPin -or $native.patcherSha256 -cne $patcherSha -or
        $native.headerSha256 -cne $headerSha -or
        $native.patchedDebuggerThreadSha256 -cne $threadSha -or
        $native.patchedProgressEventHeaderSha256 -cne $progressSha -or
        $native.lldbDapExeSha256 -cne $binarySha -or
        $native.observerOnly -isnot [bool] -or !$native.observerOnly -or
        $native.productionDebuggerActions -isnot [bool] -or $native.productionDebuggerActions -or
        $native.replayAuthorized -isnot [bool] -or $native.replayAuthorized) {
        throw 'DENIED: first-party native provenance or observer policy mismatch'
    }
}
if ($Mode -eq 'Export') {
    $root=Join-Path $env:RUNNER_TEMP $bundleName
    RequireBundle $root
    $manifest=[ordered]@{schema='lu.ide.llvm18.transport.v1';repo=$env:GITHUB_REPOSITORY;head=$head;llvmCommit=$llvmPin;producerRun=$env:GITHUB_RUN_ID;producerAttempt=$env:GITHUB_RUN_ATTEMPT;patcherSha256=$patcherSha;files=@(Inventory $root)}
    [IO.File]::WriteAllText((Join-Path $env:RUNNER_TEMP $manifestName),($manifest | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    Write-Host 'LLVM18_EXACT_HEAD_NATIVE_TRANSPORT=EXPORTED_NOT_ACCEPTANCE'
    exit 0
}
if ([string]::IsNullOrWhiteSpace($env:GH_TOKEN)) { throw 'DENIED: only consumer job read-only Actions token may fetch' }
$branch=if ($env:GITHUB_EVENT_NAME -eq 'pull_request') { $env:GITHUB_HEAD_REF } else { $env:GITHUB_REF_NAME }
$workflow='ide-dev04-5d5-owner-event-native.yml'
$deadline=[DateTime]::UtcNow.AddMinutes(190)
$producer=$null
$artifact=$null
$round=0
while ([DateTime]::UtcNow -lt $deadline) {
    $round++
    $runsRaw=& gh api "repos/$env:GITHUB_REPOSITORY/actions/workflows/$workflow/runs?head_sha=$head&event=$env:GITHUB_EVENT_NAME&per_page=100"
    if ($LASTEXITCODE -ne 0) { throw 'DENIED: exact-head producer workflow lookup failed' }
    $runs=($runsRaw | ConvertFrom-Json)
    if ($runs.total_count -gt 100) { throw 'DENIED: ambiguous producer workflow pagination' }
    $matches=@($runs.workflow_runs | Where-Object { $_.head_sha -ceq $head -and $_.head_branch -ceq $branch -and $_.event -ceq $env:GITHUB_EVENT_NAME -and $_.head_repository.full_name -ceq $env:GITHUB_REPOSITORY -and $_.path -ceq ".github/workflows/$workflow" })
    if ($matches.Count -gt 1) { throw 'DENIED: multiple exact-head producer runs; no ambiguous artifact adoption' }
    if ($matches.Count -eq 1) {
        $producer=$matches[0]
        $name="dev045d5-native-bundle-$head-$($producer.run_attempt)"
        $raw=& gh api "repos/$env:GITHUB_REPOSITORY/actions/runs/$($producer.id)/artifacts?name=$name&per_page=100"
        if ($LASTEXITCODE -ne 0) { throw 'DENIED: producer artifact query failed' }
        $artifacts=$raw | ConvertFrom-Json
        $matching=@($artifacts.artifacts | Where-Object { $_.name -ceq $name -and !$_.expired })
        if ($matching.Count -gt 1) { throw 'DENIED: duplicate producer artifact name' }
        if ($matching.Count -eq 1) { $artifact=$matching[0]; break }
        if ($producer.status -eq 'completed') { throw 'DENIED: producer completed without verified candidate bundle' }
    }
    if (($round % 9) -eq 1) { Write-Host 'LLVM18_EXACT_HEAD_NATIVE_TRANSPORT=WAITING_FOR_ORIGINAL_PRODUCER' }
    Start-Sleep -Seconds 20
}
if (!$artifact -or !$producer) { throw 'DENIED: bounded original-producer artifact wait expired' }
$dest=Join-Path $env:RUNNER_TEMP 'lu-wo6-pinned-owner-intake'
if (Test-Path -LiteralPath $dest) { throw 'DENIED: native artifact destination already exists' }
New-Item -ItemType Directory -Path $dest | Out-Null
& gh run download "$($producer.id)" --repo $env:GITHUB_REPOSITORY --name $name --dir $dest
if ($LASTEXITCODE -ne 0) { throw 'DENIED: native candidate artifact download failed' }
$mp=Join-Path $dest $manifestName
$root=Join-Path $dest $bundleName
if (!(Test-Path -LiteralPath $mp -PathType Leaf)) { throw 'DENIED: native transport manifest invalid' }
$manifestMeta=Get-Item -LiteralPath $mp -Force
if ($manifestMeta.Length -gt 1MB -or $manifestMeta.Length -eq 0 -or
    ($manifestMeta.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'DENIED: native transport manifest invalid'
}
$manifest=Get-Content -LiteralPath $mp -Raw | ConvertFrom-Json
if ($manifest.schema -cne 'lu.ide.llvm18.transport.v1' -or $manifest.repo -cne $env:GITHUB_REPOSITORY -or
    $manifest.head -cne $head -or $manifest.llvmCommit -cne $llvmPin -or
    $manifest.producerRun -cne [string]$producer.id -or $manifest.producerAttempt -cne [string]$producer.run_attempt -or
    $manifest.patcherSha256 -cne $patcherSha) {
    throw 'DENIED: transported native candidate does not belong to exact reviewed source/run attempt'
}
RequireBundle $root
$actual=@(Inventory $root)
Assert-Inventory $actual @($manifest.files)
Add-Content $env:GITHUB_ENV "LU_IDE_WO6_DAP_BUNDLE=$root"
Write-Host 'LLVM18_EXACT_HEAD_NATIVE_TRANSPORT=VERIFIED_NOT_KERNEL_ACCEPTANCE'
