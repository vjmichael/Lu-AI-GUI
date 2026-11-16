# Original Windows WO6 debug-only ETW file-I/O denial classifier.
# Never saves or uploads raw file names, source bytes, event XML or the trace.
# An ETW candidate is not proof that the denied operation caused the exit.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$TracePath,
    [Parameter(Mandatory=$true)][string]$PidWitness
)
$ErrorActionPreference='Stop'
function Emit([string]$value) {
    Write-Host ('PROTECTED_WO6_ETW_FILE_DENIAL=' + $value)
}
if (!(Test-Path -LiteralPath $TracePath -PathType Leaf) -or
    !(Test-Path -LiteralPath $PidWitness -PathType Leaf)) {
    Emit 'trace_or_native_pid_unavailable_not_acceptance'
    exit 0
}
try {
    $info=Get-Item -LiteralPath $TracePath
    $pidInfo=Get-Item -LiteralPath $PidWitness
    if ($info.Length -le 0 -or $info.Length -gt 32MB -or
        $pidInfo.Length -le 0 -or $pidInfo.Length -gt 4096) {
        Emit 'trace_or_pid_bounds_rejected'
        exit 0
    }
    $ids=@{}
    foreach($line in @(Get-Content -LiteralPath $PidWitness -TotalCount 64)) {
        $number=0
        if ($line -match '^[1-9][0-9]{0,9}$' -and
            [int]::TryParse($line,[ref]$number)) {
            $ids[[string]$number]=$true
        }
    }
    if ($ids.Count -eq 0) {
        Emit 'no_attested_child_pid'
        exit 0
    }
    $begins=@{}
    $outcomes=New-Object 'System.Collections.Generic.List[string]'
    $events=Get-WinEvent -Path $TracePath -Oldest -MaxEvents 50000 -ErrorAction Stop
    foreach($record in $events) {
        if ($record.ProviderName -ne 'Microsoft-Windows-Kernel-File' -or
            $record.Id -notin @(12,24,30)) { continue }
        try { [xml]$xml=$record.ToXml() } catch { continue }
        $fields=@{}
        foreach($entry in @($xml.Event.EventData.Data)) {
            if ($null -ne $entry -and $entry.Name) {
                $fields[[string]$entry.Name]=[string]$entry.'#text'
            }
        }
        $irp=[string]$fields['Irp']
        if ([string]::IsNullOrEmpty($irp)) { continue }
        if ($record.Id -in @(12,30)) {
            # $PID is a read-only PowerShell automatic variable.
            $childProcessId=[string]$record.ProcessId
            if (!$ids.ContainsKey($childProcessId)) { continue }
            if ($begins.Count -ge 4096) { continue }
            $operation=if ($record.Id -eq 30) { 'create_new' } else { 'create_open' }
            $path=[string]$fields['FileName']
            $object='other_or_unclassified'
            if ($path -match '(?i)wo6-dap-bundle') { $object='measured_adapter_bundle' }
            elseif ($path -match '(?i)appdata[\\/]local[\\/]packages') { $object='appcontainer_local' }
            elseif ($path -match '(?i)[\\/]windows[\\/]') { $object='windows_system_file' }
            elseif ($path -match '(?i)[\\/]device[\\/]') { $object='windows_device_file' }
            $begins[$irp]=@($operation,$object,$childProcessId)
            continue
        }
        $raw=[string]$fields['Status']
        $status=$null
        if ($raw -match '^0[xX]([0-9a-fA-F]{8})$') { $status=$Matches[1].ToUpperInvariant() }
        elseif ($raw -match '^[0-9]+$') {
            try { $status=('{0:X8}' -f [uint32]$raw) } catch { continue }
        }
        if ($status -ne 'C0000022' -or !$begins.ContainsKey($irp)) { continue }
        $from=$begins[$irp]
        $candidate=[string]::Join(';',@(
            'operation=' + $from[0], 'object_class=' + $from[1],
            'ntstatus=C0000022', 'pid_irp_correlated_not_causality'))
        if (!$outcomes.Contains($candidate)) { $outcomes.Add($candidate) }
        if ($outcomes.Count -ge 8) { break }
    }
    if ($outcomes.Count -eq 0) {
        Emit 'no_pid_correlated_file_access_denial_other_operations_unobserved'
    } else {
        foreach($candidate in $outcomes) { Emit $candidate }
    }
} catch {
    Emit 'etl_parse_unavailable_no_authorization_claim'
}
# All printed categories are closed/fixed. Never emit the raw event or path.
