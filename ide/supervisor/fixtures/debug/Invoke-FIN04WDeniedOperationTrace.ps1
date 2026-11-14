# FIN-04W disposable Windows CI only: independently observe OS FILE/REGISTRY
# denials made by the real LLDB-DAP child. No injection, sandbox modification,
# debugger authorization, service policy edit, or production distribution.
param(
    [Parameter(Mandatory=$true)][ValidateSet('Start','Collect')][string]$Phase,
    [Parameter(Mandatory=$true)][string]$Root
)
$ErrorActionPreference='Stop'
function Write-Status([string]$s) { Write-Host "FIN04W_PROCMON_DIAGNOSTIC=$s" }
$rootFull=[IO.Path]::GetFullPath($Root)
$tempFull=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
if (!$rootFull.StartsWith($tempFull,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'DENIED: operation trace must be under disposable runner temp'
}
New-Item -ItemType Directory -Force -Path $rootFull | Out-Null
$binary=Join-Path $rootFull 'procmon\Procmon64.exe'
$pml=Join-Path $rootFull 'observe.pml'
$csv=Join-Path $rootFull 'observe.csv'
$report=Join-Path $rootFull 'denied-operation-summary.json'
if ($Phase -eq 'Start') {
    try {
        $zip=Join-Path $rootFull 'ProcessMonitor.zip'
        Invoke-WebRequest -Uri 'https://download.sysinternals.com/files/ProcessMonitor.zip' -OutFile $zip -UseBasicParsing
        Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $rootFull 'procmon') -Force
        if (!(Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'signed_procmon64_missing' }
        $sig=Get-AuthenticodeSignature -LiteralPath $binary
        if ($sig.Status -ne 'Valid' -or !$sig.SignerCertificate -or
            $sig.SignerCertificate.Subject -notmatch 'Microsoft') {
            throw 'untrusted_procmon_signature'
        }
        $sha=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
        $metadata=@{schema='lu.ide.fin04w.signed_process_monitor.v1';authenticode='valid_microsoft';
                    monitor_sha256=$sha;original_native_security_tests_unchanged=$true}
        $metadata|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $rootFull 'monitor-provenance.json')
        $args=@('/AcceptEula','/Quiet','/Minimized','/NoFilter','/BackingFile',('"' + $pml + '"'))
        $null=Start-Process -FilePath $binary -ArgumentList $args -PassThru -WindowStyle Hidden
        Start-Sleep -Seconds 3
        if (!(Get-Process -Name 'Procmon64' -ErrorAction SilentlyContinue)) {
            throw 'monitor_process_not_running'
        }
        Write-Status 'SIGNED_KERNEL_TRACE_STARTED_NOT_AUTHORITY'
    } catch {
        # A disposable runner may refuse the signed Sysinternals driver.
        # The unchanged original native security test proceeds and still
        # determines acceptance. Do not count a missing trace as a pass.
        Write-Status 'UNAVAILABLE_NO_TRACE_NO_ACCEPTANCE'
    }
    exit 0
}
$summary=[ordered]@{
    schema='lu.ide.fin04w.denied_windows_op.v1'
    source_sha=$env:GITHUB_SHA
    original_security_tests_unchanged=$true
    evidence='UNAVAILABLE'
    observer='Microsoft-signed Sysinternals Process Monitor'
    denial_count=0
    denied_operations=@()
    caveat='A Procmon file/registry result is the kernel operation, not automatically its calling Win32 API; correlate stack before source patch'
}
try {
    if (!(Test-Path -LiteralPath $binary -PathType Leaf) -or
        !(Test-Path -LiteralPath $pml -PathType Leaf)) {
        $summary.evidence='NO_SIGNED_KERNEL_TRACE'
    } else {
        $stop=Start-Process -FilePath $binary -ArgumentList '/Terminate','/Quiet' -PassThru -WindowStyle Hidden
        if (!$stop.WaitForExit(30000)) { try{$stop.Kill()}catch{};throw 'monitor_stop_timeout' }
        $file=Get-Item -LiteralPath $pml
        # Stay bounded and never upload raw trace: it includes unrelated
        # CI process details, environment and path names.
        if ($file.Length -gt 100MB -or $file.Length -eq 0) {
            $summary.evidence='PML_SIZE_OUT_OF_BOUNDS'
        } else {
            $convert=Start-Process -FilePath $binary -ArgumentList @(
                '/AcceptEula','/Quiet','/OpenLog',('"' + $pml + '"'),'/SaveAs',('"' + $csv + '"')
            ) -PassThru -WindowStyle Hidden
            if (!$convert.WaitForExit(120000)) {try{$convert.Kill()}catch{};throw 'monitor_csv_timeout'}
            if (!(Test-Path -LiteralPath $csv -PathType Leaf)) { throw 'missing_csv' }
            $seen=0
            $recorded=New-Object System.Collections.Generic.List[object]
            foreach ($item in (Import-Csv -LiteralPath $csv)) {
                if ($item.'Process Name' -ine 'lldb-dap.exe') { continue }
                if ($item.Result -notmatch '(?i)ACCESS DENIED|PRIVILEGE NOT HELD|SHARING VIOLATION|INVALID PARAMETER') {continue}
                $seen++
                if ($recorded.Count -ge 60) {continue}
                $path=[string]$item.Path
                # Exact pipe namespace/name is useful for distinguishing
                # AppContainer LOCAL from the global namespace. Everything
                # else is classified; never leak unrelated paths in CI logs.
                $pathClass='other'
                $target=''
                if ($path -match '(?i)\\(pipe|namedpipe)\\') {
                    $pathClass=if ($path -match '(?i)\\LOCAL\\') {'named_pipe_local'}else{'named_pipe_global'}
                    $target=if($path.Length -le 256){$path}else{$path.Substring(0,256)}
                } elseif ($path -match '(?i)\.dll$') {
                    $pathClass='dll_image'
                    $target=[IO.Path]::GetFileName($path)
                } elseif ($path -match '(?i)^HK(LM|CU|CR|U)\\') {
                    $pathClass='registry'
                    $target='[redacted]'
                } elseif ($path -match '(?i)\\Device\\') {
                    $pathClass='nt_device_or_file'
                    $target='[redacted]'
                } else {
                    $pathClass='file_or_other'
                    $target='[redacted]'
                }
                $recorded.Add([ordered]@{
                    operation=[string]$item.Operation
                    result=[string]$item.Result
                    path_class=$pathClass
                    bounded_target=$target
                    # CI only; one process name, no user source or full paths.
                    process_name='lldb-dap.exe'
                })
            }
            $summary.denial_count=$seen
            $summary.denied_operations=@($recorded.ToArray())
            $summary.evidence=if($seen -gt 0){'OBSERVED_LLDB_DAP_DENIALS'}else{'NO_LLDB_DAP_DENIALS_IN_TRACE'}
        }
    }
} catch {
    $summary.evidence='TRACE_COLLECTION_FAILED'
} finally {
    $summary|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $report
    # Do not expose raw PML/CSV as artifacts or retain them on CI workspace.
    Remove-Item -LiteralPath $csv,$pml -Force -ErrorAction SilentlyContinue
    Write-Status $summary.evidence
    foreach ($v in @($summary.denied_operations | Select-Object -First 15)) {
        Write-Host ("FIN04W_DENIED_OPERATION={0};RESULT={1};OBJECT={2};TARGET={3}" -f $v.operation,$v.result,$v.path_class,$v.bounded_target)
    }
}
exit 0
