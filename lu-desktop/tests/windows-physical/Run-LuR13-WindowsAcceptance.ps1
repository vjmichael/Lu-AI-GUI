# Lu R13 physical Windows evaluation — TEST ONLY, not an alpha certification.
# Works with built-in Windows PowerShell 5.1; no third-party modules.
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\LuR12DesktopDevEval'),
    [switch]$NonInteractive
)
$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$root = Join-Path $env:LOCALAPPDATA ("LuAcceptance\R13-" + $stamp)
New-Item -ItemType Directory -Path $root -Force | Out-Null
$notes = New-Object 'System.Collections.Generic.List[object]'
function Record([string]$check, [string]$result, [string]$detail) {
    $script:notes.Add([pscustomobject]@{check=$check;result=$result;detail=$detail})
    Write-Host ("[{0}] {1}: {2}" -f $result.ToUpperInvariant(),$check,$detail)
}
function Confirm-Lu([string]$label) {
    if ($NonInteractive) {Record $label 'not_checked' 'Manual UI observation required'; return}
    $answer = Read-Host ("{0} (y/n/s=skip)" -f $label)
    if ($answer -match '^y') {Record $label 'user_reported_pass' 'Affirmed by person testing'}
    elseif ($answer -match '^n') {Record $label 'user_reported_fail' 'Reported failed by person testing'}
    else {Record $label 'not_checked' 'Skipped'}
}
function Capture-Processes([string]$name) {
    $rows = @()
    try {
        $rows = @(Get-CimInstance Win32_Process -ErrorAction Stop |
            Where-Object {$_.Name -in @('lu-desktop.exe','llama-server.exe')} |
            Select-Object Name,ProcessId,ParentProcessId,ExecutablePath)
    } catch {
        Record "process_snapshot_$name" 'unverified' ('CIM unavailable: '+$_.Exception.Message)
    }
    $rows | ConvertTo-Json -Depth 4 | Out-File -FilePath (Join-Path $root ("processes-"+$name+".json")) -Encoding utf8
    return $rows
}
function PE-Subsystem([string]$file) {
    $s=[System.IO.File]::OpenRead($file)
    try {
        $r=New-Object System.IO.BinaryReader($s)
        if ($r.ReadUInt16() -ne 0x5a4d) {throw 'Missing MZ'}
        $s.Position=0x3c; $off=$r.ReadUInt32()
        if ($off -gt ($s.Length-128)) {throw 'Invalid PE header offset'}
        $s.Position=$off
        if ($r.ReadUInt32() -ne 0x00004550) {throw 'Invalid PE signature'}
        $machine=$r.ReadUInt16();$s.Position=$off+24
        $magic=$r.ReadUInt16();$s.Position=$off+24+68
        $subsystem=$r.ReadUInt16()
        return [pscustomobject]@{machine=$machine;magic=$magic;subsystem=$subsystem}
    } finally { $s.Dispose() }
}
Write-Host "Lu R13 installed-desktop acceptance (development evaluation only)"
Write-Host "Evidence directory: $root"
Write-Host 'The script never copies chat text, images, GGUF contents or bearer tokens.'
$desktop = Join-Path $InstallDir 'lu-desktop.exe'
$engineDir = Join-Path $InstallDir 'lu-inference'
$engine = Join-Path $engineDir 'llama-server.exe'
try {
    if (!(Test-Path -LiteralPath $desktop -PathType Leaf)) {throw "Desktop missing: $desktop. Install the R13 evaluation installer first."}
    $expectedDesktopHash='4ef60de064b555ab63e3ec961d3a144e7f4e203458b39ef61381a24e0192fd6e'
    $actualDesktopHash=(Get-FileHash -LiteralPath $desktop -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualDesktopHash -eq $expectedDesktopHash) {Record 'desktop_sha256' 'pass' $actualDesktopHash}
    else {Record 'desktop_sha256' 'fail' $actualDesktopHash;throw 'Unexpected Lu Desktop EXE — please use exact R13 installer'}
    $pe=PE-Subsystem $desktop
    if ($pe.machine -eq 0x8664 -and $pe.magic -eq 0x20b -and $pe.subsystem -eq 2) {
       Record 'desktop_pe' 'pass' 'PE32+ AMD64 Windows GUI subsystem'
    } else {
       Record 'desktop_pe' 'fail' ("Machine={0} Magic={1} Subsystem={2}" -f $pe.machine,$pe.magic,$pe.subsystem)
       throw 'Unexpected executable architecture or subsystem'
    }
    $manifestPath=Join-Path $engineDir 'manifest.json'
    if (!(Test-Path $manifestPath)) {throw 'Pinned inference manifest missing'}
    $manifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 'lu-inference-package-v1' -or $manifest.backendId -ne 'llama-vulkan') {throw 'Unexpected engine manifest'}
    foreach($f in $manifest.files) {
        $p=Join-Path $engineDir $f.name
        if (!(Test-Path -LiteralPath $p -PathType Leaf)) {throw ("Missing engine file: "+$f.name)}
        $len=(Get-Item -LiteralPath $p).Length
        $hash=(Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($len -ne [long]$f.bytes -or $hash -ne $f.sha256.ToLowerInvariant()) {throw ("Engine integrity failed: "+$f.name)}
        Record ("component_"+$f.name) 'pass' 'Length and SHA-256 match manifest'
    }
    try {
        $gpu=@(Get-CimInstance Win32_VideoController -ErrorAction Stop | Select-Object Name,DriverVersion,VideoProcessor)
        $gpu | ConvertTo-Json -Depth 4 | Out-File (Join-Path $root 'gpu-adapters.json') -Encoding utf8
        Record 'gpu_adapter_inventory' 'observed' ("Found {0} video adapters; this is not proof of offload" -f $gpu.Count)
    } catch {Record 'gpu_adapter_inventory' 'unverified' $_.Exception.Message}
    if (Test-Path -LiteralPath $engine) {
        $deviceLog=Join-Path $root 'llama-list-devices.txt'
        try {
            & $engine --list-devices *> $deviceLog
            $exitCode=$LASTEXITCODE
            Record 'vulkan_device_enumeration' ($(if($exitCode -eq 0){'observed'}else{'fail'})) ("Exit={0}; inspect {1}. Device visibility does NOT prove inference offload." -f $exitCode,$deviceLog)
        } catch {
            Record 'vulkan_device_enumeration' 'fail' $_.Exception.Message
        }
    }
    $baseline=@(Capture-Processes 'before')
    $start=Get-Date
    try {
        $process=Start-Process -FilePath $desktop -WorkingDirectory $InstallDir -PassThru -ErrorAction Stop
        Start-Sleep -Seconds 4
        $process.Refresh()
        if ($process.HasExited) {
            Record 'lu_launch_process' 'fail' ("Process exited early with code " + $process.ExitCode)
        } else {
            Record 'lu_launch_process' 'observed' ("Process running (PID {0}); actual GUI visibility requires confirmation" -f $process.Id)
        }
    } catch {
        Record 'lu_launch_process' 'fail' $_.Exception.Message
    }
    Capture-Processes 'running' | Out-Null
    Write-Host ''
    Write-Host 'In Lu, try: Import GGUF > Load > normal text reply > Unload > Attach MMProj > Load > image-dependent reply.'
    Write-Host 'Also test Cancel, restart Lu, and whether your conversation comes back.'
    Confirm-Lu 'A Lu window appeared and buttons were responsive'
    Confirm-Lu 'GGUF imported through native picker and loaded'
    Confirm-Lu 'Model gave a nonempty normal text reply'
    Confirm-Lu 'MMProj attached and image-containing question answered correctly'
    Confirm-Lu 'Cancel stopped a running generation'
    Confirm-Lu 'Chat history survived closing and reopening Lu'
    Capture-Processes 'after-interaction' | Out-Null
    if (-not $NonInteractive) {Read-Host 'Close Lu using its window, then press Enter to check worker cleanup'}
    Start-Sleep -Seconds 2
    $after=@(Capture-Processes 'after-close')
    $basePids=@($baseline | ForEach-Object {$_.ProcessId})
    $extraWorkers=@($after | Where-Object {$_.Name -eq 'llama-server.exe' -and $_.ProcessId -notin $basePids})
    if($extraWorkers.Count -gt 0) {Record 'worker_cleanup' 'fail' ("{0} new llama-server processes remain" -f $extraWorkers.Count)}
    else {Record 'worker_cleanup' 'observed' 'No new llama-server process remains (not a complete OS containment test)'}
} catch {
    Record 'preflight_or_execution' 'fail' $_.Exception.Message
} finally {
    $report=[pscustomobject]@{
        schema='lu-r13-windows-acceptance-v1';build='R13-dev-evaluation-NOT-ALPHA';
        timestamp=(Get-Date).ToString('o');install_dir=$InstallDir;observations=@($notes.ToArray());
        limitations=@('Manual UI outcomes are user-reported and not independent automation.',
          'Device enumeration alone does not verify AMD Vulkan offload.',
          'Actual model load offload logs and image-dependent correctness remain required.',
          'This is NOT an alpha approval report.')
    }
    $report | ConvertTo-Json -Depth 8 | Out-File -FilePath (Join-Path $root 'acceptance-observations.json') -Encoding utf8
    Write-Host ''
    Write-Host ("Evidence saved: "+$root)
    Write-Host 'Share acceptance-observations.json and llama-list-devices.txt if you want me to diagnose the results.'
    Write-Host 'Do not share private chat history or any API keys.'
}
