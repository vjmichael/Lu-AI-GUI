# IDE-PRECORE-CLOSEOUT-R3 native Windows read-only candidate observation.
# Does not sign/install/start workers, modify SCM, promote releases, or execute projects.
# Its output is a measurement, NOT an authenticated FIN-01D terminal/Job receipt.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$RepositoryRoot,
    [Parameter(Mandatory=$true)][string]$CandidateRoot,
    [Parameter(Mandatory=$true)][string]$SourceSha,
    [string]$InstallRoot,
    [string]$OutputPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Deny([string]$code) { throw "DENIED: IDE-PRECORE-R3: $code" }
function Assert-NoReparse([string]$path) {
    $item = Get-Item -LiteralPath $path -Force -ErrorAction Stop
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        Deny 'reparse_point_not_accepted'
    }
}
function Assert-NoStandardWrite([string]$path) {
    $acl = Get-Acl -LiteralPath $path
    foreach ($ace in $acl.Access) {
        if ($ace.AccessControlType -ne 'Allow') { continue }
        $sid = $ace.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
        if ($sid -notin @('S-1-1-0','S-1-5-11','S-1-5-32-545')) { continue }
        $write = [Security.AccessControl.FileSystemRights]::Write -bor
                 [Security.AccessControl.FileSystemRights]::Modify -bor
                 [Security.AccessControl.FileSystemRights]::FullControl -bor
                 [Security.AccessControl.FileSystemRights]::Delete
        if (($ace.FileSystemRights -band $write) -ne 0) { Deny 'standard_user_can_write_signed_install' }
    }
}
function Safe-Asset([string]$root,[object]$rel) {
    if ($rel -isnot [string] -or [string]::IsNullOrWhiteSpace($rel) -or
        $rel.Length -gt 240 -or $rel.Contains(':') -or $rel.Contains('\')) {
        Deny 'invalid_candidate_relative_path'
    }
    $current=$root
    foreach ($part in $rel.Split('/')) {
        if ($part -in @('','.', '..') -or $part.EndsWith(' ') -or $part.EndsWith('.')) {
            Deny 'invalid_candidate_relative_component'
        }
        $current=Join-Path $current $part
        if (!(Test-Path -LiteralPath $current)) { Deny 'signed_candidate_asset_missing' }
        Assert-NoReparse $current
    }
    if (!(Test-Path -LiteralPath $current -PathType Leaf)) { Deny 'signed_candidate_leaf_missing' }
    $resolved=(Resolve-Path -LiteralPath $current).Path
    if (!$resolved.StartsWith(($root.TrimEnd('\')+'\'),[StringComparison]::OrdinalIgnoreCase)) {
        Deny 'candidate_asset_escaped_root'
    }
    return $resolved
}
function Verify-NativeSignature([string]$path,[object]$record) {
    if ($record.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
        $record.signerThumbprint -notmatch '^[0-9a-fA-F]{40}$') {
        Deny 'signed_candidate_pin_incomplete'
    }
    $bytes=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($bytes -ine $record.sha256) { Deny 'candidate_bytes_not_pinned' }
    $sig=Get-AuthenticodeSignature -LiteralPath $path
    if ($sig.Status -ne 'Valid' -or !$sig.SignerCertificate -or
        $sig.SignerCertificate.Thumbprint -ine $record.signerThumbprint) {
        Deny 'native_authenticode_not_trusted'
    }
    return [ordered]@{
        sha256=$bytes
        signerThumbprint=$sig.SignerCertificate.Thumbprint.ToLowerInvariant()
        authenticode='Valid'
    }
}
if ($env:OS -ne 'Windows_NT') { Deny 'windows_required' }
if ($SourceSha -notmatch '^[0-9a-fA-F]{40}$') { Deny 'source_sha_must_be_exact' }
$repo=(Resolve-Path -LiteralPath $RepositoryRoot).Path
$candidate=(Resolve-Path -LiteralPath $CandidateRoot).Path
Assert-NoReparse $repo
Assert-NoReparse $candidate
$head=(& git -C $repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $head -ine $SourceSha) { Deny 'source_head_drift' }
$dirty=@(& git -C $repo status --porcelain --untracked-files=all)
if ($LASTEXITCODE -ne 0 -or $dirty.Count -gt 0) { Deny 'source_tree_not_clean' }
$ancestor=& git -C $repo merge-base --is-ancestor 'f1f0ed4bc1e740d2a9f85615b988b779a46651db' $SourceSha
if ($LASTEXITCODE -ne 0) { Deny 'original_ide_pr_ancestry_not_verified' }
# Exact reviewed R2 IPC lock successors must be the packaged source inputs.
foreach ($pin in @(
    @{rel='ide/host/Cargo.lock'; sha='f9fc6178178bf2d56c6ca308472b9ba2e35a27910e3a30f6b72e9339bb4d10b2'},
    @{rel='ide/app/src-tauri/Cargo.lock'; sha='77e317747fdf2743f55363c9809f866a770255'}
)) {
    $file=Join-Path $repo $pin.rel
    Assert-NoReparse $file
    $actual=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
    if ($actual -ine $pin.sha) { Deny 'r2_reviewed_lock_identity_drift' }
}
$confFile=Join-Path $repo 'ide/app/src-tauri/tauri.conf.json'
Assert-NoReparse $confFile
$conf=Get-Content -LiteralPath $confFile -Raw | ConvertFrom-Json
if ($conf.productName -ne 'Lu IDE' -or $conf.identifier -ne 'org.lu.ide' -or
    $conf.bundle.active -ne $false) { Deny 'standalone_default_disabled_source_identity_changed' }
$manifest=Get-Content -LiteralPath (Join-Path $candidate 'wo7-release.manifest.json') -Raw | ConvertFrom-Json
if ($manifest.schema -ne 'lu.ide.wo7.signed-nsis.v1' -or
    $manifest.sourceSha -ine $SourceSha -or
    $manifest.licensesReviewed -ne $true -or
    $manifest.licenseInventorySha256 -notmatch '^[0-9a-fA-F]{64}$' -or
    $manifest.releaseApproved -ne $false) {
    Deny 'signed_candidate_or_independent_license_review_not_ready'
}
$observed=[ordered]@{}
foreach ($role in @('installer','service','desktop')) {
    $record=$manifest.$role
    if (!$record) { Deny 'signed_candidate_role_missing' }
    $fixed=@{installer='lu-ide-signed-setup.exe';
             service='windows_recovery_service.exe';desktop='lu-ide-desktop.exe'}
    if ($record.path -cne $fixed[$role]) { Deny 'unreviewed_candidate_asset_role_path' }
    $file=Safe-Asset $candidate $record.path
    $observed[$role]=Verify-NativeSignature $file $record
}
$installedServiceObserved=$false
if ($InstallRoot) {
    $root=(Resolve-Path -LiteralPath $InstallRoot).Path.TrimEnd('\')
    $allowed=@((Join-Path $env:ProgramFiles 'Lu IDE'))
    $x86=[Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if ($x86) { $allowed+=Join-Path $x86 'Lu IDE' }
    if (!($allowed | Where-Object { $root -ieq [IO.Path]::GetFullPath($_).TrimEnd('\') })) {
        Deny 'nonfixed_install_root'
    }
    foreach($location in @($root,(Join-Path $root 'resources'),(Join-Path $root 'resources\recovery'))) {
        Assert-NoReparse $location
        Assert-NoStandardWrite $location
    }
    $serviceExe=Join-Path $root 'resources\recovery\windows_recovery_service.exe'
    $desktopExe=Join-Path $root 'lu-ide-desktop.exe'
    foreach($bin in @($serviceExe,$desktopExe)) {
        Assert-NoReparse $bin
        Assert-NoStandardWrite $bin
    }
    $null=Verify-NativeSignature $serviceExe $manifest.service
    $null=Verify-NativeSignature $desktopExe $manifest.desktop
    $svc=Get-CimInstance -ClassName Win32_Service -Filter "Name='LuIdeRecoveryBroker'" -ErrorAction Stop
    if (!$svc -or $svc.StartName -notin @('NT AUTHORITY\LocalService','NT AUTHORITY\LOCAL SERVICE') -or
        $svc.PathName.Trim('"') -ine $serviceExe -or $svc.ServiceType -ne 'Own Process') {
        Deny 'installed_scm_identity_mismatch'
    }
    $sc=Join-Path $env:SystemRoot 'System32\sc.exe'
    $sidType=(& $sc qsidtype LuIdeRecoveryBroker 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0 -or $sidType -notmatch 'SERVICE_SID_TYPE\s*:\s*RESTRICTED') {
        Deny 'installed_scm_restricted_service_sid_unverified'
    }
    $installedServiceObserved=$true
}
# The original worker Job, SCM signed stop/release, revocation, cancellation and
# restart receipts must be collected in separate genuine original-job probes.
# SCM service identity by itself CANNOT establish empty Job or safe termination.
$receipt=[ordered]@{
    schema='lu.ide.precore.r3.windows-readonly-observation.v1'
    sourceSha=$SourceSha.ToLowerInvariant()
    signedCandidateNativeSignaturesVerified=$true
    signedArtifacts=$observed
    installedRestrictedLocalServiceIdentityObserved=$installedServiceObserved
    protectedWorkerJobCustodyVerified=$false
    originalTerminalCancelRevokeRestartReceiptsVerified=$false
    physicalAcceptanceGranted=$false
    releaseApproved=$false
}
$output=$receipt | ConvertTo-Json -Depth 8
if ($OutputPath) {
    if (Test-Path -LiteralPath $OutputPath) { Deny 'refuse_overwriting_native_observation' }
    $file=[IO.Path]::GetFullPath($OutputPath)
    if ($file.StartsWith(($repo.TrimEnd('\')+'\'),[StringComparison]::OrdinalIgnoreCase)) {
        Deny 'witness_must_be_outside_source_checkout'
    }
    $stream=[IO.FileStream]::new($file,[IO.FileMode]::CreateNew,
                               [IO.FileAccess]::Write,[IO.FileShare]::None)
    try {
        $bytes=[System.Text.UTF8Encoding]::new($false).GetBytes($output)
        $stream.Write($bytes,0,$bytes.Length)
        $stream.Flush($true)
    } finally { $stream.Dispose() }
}
Write-Output $output
