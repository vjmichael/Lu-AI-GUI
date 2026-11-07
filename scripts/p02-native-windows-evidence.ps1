# P02 physical Windows evidence: read-only CPU/RAM and optional WDDM inference-PID counters.
# Nothing is installed, launched, killed, reconfigured, or downloaded.
# A device reading is NOT model capacity, an installed-app test, or a GPU grant.
[CmdletBinding()]
param(
    [string]$EvidenceFile = '',
    [string]$InstalledExe = '',
    [int]$AppPid = 0,
    [int]$InferencePid = 0,
    [switch]$RequireInstalled,
    [switch]$RequireGpu
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'P02 evidence requires native Windows' }

function Resolve-LiteralFile([string]$Path) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    return [System.IO.Path]::GetFullPath((Get-Item -LiteralPath $Path -ErrorAction Stop).FullName)
}
function Find-Process([int]$TargetPid) {
    if ($TargetPid -le 0) { return $null }
    return Get-CimInstance -ClassName Win32_Process -Filter "ProcessId = $TargetPid" -ErrorAction Stop
}
$os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
$ramMiB = [long][math]::Floor([double]$os.FreePhysicalMemory / 1024.0)
$cpuSamples = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop |
    Where-Object { $null -ne $_.LoadPercentage } |
    ForEach-Object { [double]$_.LoadPercentage })
if ($ramMiB -le 0 -or $cpuSamples.Count -eq 0) {
    throw 'Native Windows RAM/CPU counters are unavailable'
}
$cpuPercent = [math]::Round(($cpuSamples | Measure-Object -Average).Average, 1)
if ($cpuPercent -lt 0 -or $cpuPercent -gt 100) {
    throw 'Native CPU counter returned an invalid value'
}
$installedProof = $false
$installedActual = $null
$installedHash = $null
if ($InstalledExe -or $AppPid -gt 0) {
    $candidate = Resolve-LiteralFile $InstalledExe
    $proc = Find-Process $AppPid
    if ($candidate -and $proc -and $proc.ExecutablePath) {
        $actual = [System.IO.Path]::GetFullPath($proc.ExecutablePath)
        $same = [string]::Equals($candidate, $actual,
            [System.StringComparison]::OrdinalIgnoreCase)
        $gui = Get-Process -Id $AppPid -ErrorAction Stop
        if ($same -and $gui.MainWindowHandle -ne 0 -and $gui.Responding) {
            $installedProof = $true
            $installedActual = $actual
            $installedHash = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
}
if ($RequireInstalled -and -not $installedProof) {
    throw 'Physical installed Lu app process and responsive GUI window not proven'
}
$gpu = [ordered]@{
    status = 'not_requested'
    inferencePid = $null
    dedicatedBytes = $null
    sharedBytes = $null
    counterSource = 'Windows WDDM GPU Process Memory; not a llama backend admission proof'
}
if ($InferencePid -gt 0 -or $RequireGpu) {
    $gpu.status = 'unavailable'
    $proc = Find-Process $InferencePid
    if ($proc -and $proc.Name -ieq 'llama-server.exe') {
        $gpu.inferencePid = $InferencePid
        try {
            $dedicated = @(Get-Counter -Counter '\GPU Process Memory(*)\Dedicated Usage' -ErrorAction Stop).CounterSamples
            $shared = @(Get-Counter -Counter '\GPU Process Memory(*)\Shared Usage' -ErrorAction Stop).CounterSamples
            $pattern = "(?i)(?:^|_)pid_$InferencePid(?:_|$)"
            $ded = @($dedicated | Where-Object { $_.InstanceName -match $pattern })
            $sha = @($shared | Where-Object { $_.InstanceName -match $pattern })
            if ($ded.Count -gt 0 -or $sha.Count -gt 0) {
                $gpu.dedicatedBytes = [long](($ded | Measure-Object -Property CookedValue -Sum).Sum)
                $gpu.sharedBytes = [long](($sha | Measure-Object -Property CookedValue -Sum).Sum)
                $gpu.status = 'measured'
            }
        } catch {
            $gpu.status = 'counter_unavailable'
        }
    } elseif ($InferencePid -gt 0) {
        $gpu.status = 'inference_process_missing'
    }
}
if ($RequireGpu -and ($gpu.status -ne 'measured' -or
    ($gpu.dedicatedBytes + $gpu.sharedBytes) -le 0)) {
    throw 'No positive WDDM memory sample for the actual llama-server PID; GPU proof unavailable'
}
$evidence = [ordered]@{
    schema = 'lu-p02-native-physical-evidence-v1'
    utc = (Get-Date).ToUniversalTime().ToString('o')
    computer = $env:COMPUTERNAME
    platform = 'Windows'
    availableRamMiB = $ramMiB
    observedCpuPercent = $cpuPercent
    installedAppPathAndResponsiveWindow = $installedProof
    installedAppSha256 = $installedHash
    gpu = $gpu
    # No GGUF filename, document text, command line, or model path is recorded.
}
$report = $evidence | ConvertTo-Json -Depth 8
Write-Output $report
if ($EvidenceFile) {
    $out = [System.IO.Path]::GetFullPath($EvidenceFile)
    [System.IO.File]::WriteAllText($out, $report + [Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false))
}
