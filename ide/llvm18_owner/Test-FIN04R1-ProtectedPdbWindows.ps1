# FIN-04R-1 exact-source native Windows acceptance for the PROTECTED PE/PDB
# reader. This file does not run by merely being committed. Requires the
# physical Windows owner or independent Windows acceptance host with real
# LocalService SCM, AppContainer, original Job kernel custody and pinned
# LLVM18 first-party owner-event bundle. No signed/installed GUI acceptance.
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{40}$')]
  [string]$ExpectedSourceHead,
  [Parameter(Mandatory=$true)][string]$PinnedLldbBundle,
  [Parameter(Mandatory=$true)][string]$PinnedCargoBundle,
  [Parameter(Mandatory=$true)][string]$ProtectedScmServiceExecutable,
  [Parameter(Mandatory=$true)][string]$EvidenceDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
function Deny([string]$reason){throw "FIN04R1_NATIVE_DENIED:$reason"}
if ($env:OS -ne 'Windows_NT'){Deny 'actual_Windows_required'}
$head=(git rev-parse HEAD 2>$null)
if ($LASTEXITCODE -ne 0 -or $head.Trim() -ine $ExpectedSourceHead){
  Deny 'exact_source_HEAD_mismatch'
}
function NativePath([string]$arg,[string]$name){
  $p=(Resolve-Path -LiteralPath $arg -ErrorAction Stop).Path
  if (![IO.Path]::IsPathRooted($p)){Deny "not_absolute_$name"}
  return $p
}
$bundle=NativePath $PinnedLldbBundle 'lldb_bundle'
$cargo=NativePath $PinnedCargoBundle 'cargo_bundle'
$service=NativePath $ProtectedScmServiceExecutable 'SCM_service'
$pin=Join-Path $bundle 'fin04-owner-provenance.json'
if (!(Test-Path -LiteralPath $pin -PathType Leaf)){Deny 'first_party_source_manifest_missing'}
$provenance=Get-Content -LiteralPath $pin -Raw | ConvertFrom-Json
$lldb=Join-Path $bundle 'bin/lldb-dap.exe'
if ($provenance.schema -ne 'lu.ide.fin04.first-party-lldb-owner.v1' -or
    $provenance.observerOnly -ne $true -or
    $provenance.productionDebuggerActions -ne $false -or
    $provenance.replayAuthorized -ne $false){
  Deny 'unexpected_LLVM_provenance_or_debugger_control_feature'
}
if ((Get-FileHash -LiteralPath $lldb -Algorithm SHA256).Hash -ine $provenance.lldbDapExeSha256){
  Deny 'actual_LLDB_binary_not_equal_first_party_pin'
}
$required=@('LU_IDE_WO6_DAP_BUNDLE','LU_IDE_WO6_DAP_TRUSTED_SERVICE',
  'LU_IDE_D7_NATIVE_TOOLCHAIN')
$env:LU_IDE_WO6_DAP_BUNDLE=$bundle
$env:LU_IDE_WO6_DAP_TRUSTED_SERVICE=$service
$env:LU_IDE_D7_NATIVE_TOOLCHAIN=$cargo
function AssertNative([string]$what,[scriptblock]$call){
  & $call
  if ($LASTEXITCODE -ne 0){Deny $what}
}
AssertNative 'release_observer_unit_tests' {
  cargo test --manifest-path ide/supervisor/Cargo.toml --features fin04-release-owner-evidence --lib windows_event_owner
}
AssertNative 'bounded_step_and_breakpoint_verifier_fixtures' {
  cargo test --manifest-path ide/supervisor/Cargo.toml --features fin04-release-owner-evidence --lib debug_control_dispatch
}
AssertNative 'release_PDB_source_contracts' {
  node --test ide/app/tests/fin04r1_protected_pdb.test.mjs
}
AssertNative 'release_Rust_supervisor_compiler' {
  cargo check --manifest-path ide/supervisor/Cargo.toml --release --features fin04-release-owner-evidence
}
AssertNative 'real_release_PDB_fixture_compile' {
  cargo build --manifest-path ide/supervisor/Cargo.toml --release --features fin04-release-owner-evidence --example wo6_native_dap_acceptance
}
# This host executable is NOT installed Lu and cannot claim physical user
# approval. The service must ALREADY be separately installed with its
# genuine restricted SCM identity and exact original Job kill/recovery owner.
$exe='ide/supervisor/target/release/examples/wo6_native_dap_acceptance.exe'
if (!(Test-Path -LiteralPath $exe -PathType Leaf)){Deny 'real_native_acceptance_executable_missing'}
$exe=(Resolve-Path -LiteralPath $exe).Path
AssertNative 'real_AppContainer_Job_PDB_owner_continue_restart_failed' {
  & $exe --project-debug-protected-pdb
}
# Independent negative admission: after the genuine original kernel-owned
# stop and protected PDB line, deliberately DROP the same native debugger
# gateway. DapGateway::Drop must persist CancelRequested before killing the
# original Job process tree; after its terminal Cancelled receipt, a NEW
# Supervisor::open must reject any historical one-use approval replay.
AssertNative 'real_AppContainer_Job_protected_PDB_cancel_reap_restart_failed' {
  & $exe --project-debug-protected-pdb-cancel
}
$hash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
New-Item -Path $EvidenceDirectory -ItemType Directory -Force | Out-Null
$result=[ordered]@{
  schema='fin04r1.native-windows-receipt.v1'
  exactSourceHead=$ExpectedSourceHead
  lldbBundleExeSha256=(Get-FileHash -LiteralPath $lldb -Algorithm SHA256).Hash.ToLowerInvariant()
  nativeFixtureExeSha256=$hash
  serviceExeSha256=(Get-FileHash -LiteralPath $service -Algorithm SHA256).Hash.ToLowerInvariant()
  nativeReleaseCompilerPassed=$true
  originalWindowsKernelOwnerPassed=$true
  separateProtectedPePdbReaderPassed=$true
  originalOwnerContinuePassed=$true
  originalKernelJobRestartAuditPassed=$true
  protectedPdbCancellationJobReapingAndRestartPassed=$true
  abnormalWorkerDeathAfterProtectedPdbAccepted=$false
  postCommandProductionStepAndBreakpointPassed=$false
  productionOneUseControlsAccepted=$false
  physicalInstalledHumanAcceptancePassed=$false
  observedUtc=[DateTime]::UtcNow.ToString('o')
}
$path=Join-Path $EvidenceDirectory 'fin04r1-native-protected-pdb.json'
($result | ConvertTo-Json -Depth 3)|Set-Content -LiteralPath $path -Encoding UTF8
Write-Host "FIN04R1_NATIVE_WIN_RELEASE_PDB=PASS_IF_EXECUTED;PRODUCTION_CONTROLS=DISABLED"
Write-Host "EVIDENCE: $path"
