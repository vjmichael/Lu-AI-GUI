$ErrorActionPreference='Stop'
$features='dap-transport,protected-recovery-service,dev046c-ci-control'
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib windows_event_owner
if ($LASTEXITCODE -ne 0) { throw 'DENIED: event wire/sequence/revocation unit tests failed' }
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib windows_kernel_pc
if ($LASTEXITCODE -ne 0) { throw 'DENIED: native Job OS owner tests failed' }
# One consolidated D6A/B/C/D hostile protocol/receipt/dispatch and restart test group.
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib debug_action_receipts
if ($LASTEXITCODE -ne 0) { throw 'DENIED: one-use debugger receipt/replay/restart inventory tests failed' }
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib debug_control_dispatch
if ($LASTEXITCODE -ne 0) { throw 'DENIED: independent new stop step outcome tests failed' }
cargo test --manifest-path ide/host/Cargo.toml --features 'dev046c-ci-control' --lib dap_session
if ($LASTEXITCODE -ne 0) { throw 'DENIED: native DAP step/breakpoint reply and source-denial tests failed' }
cargo fmt --manifest-path ide/supervisor/Cargo.toml -- --check
if ($LASTEXITCODE -ne 0) { throw 'DENIED: consolidated supervisor rustfmt contract failed' }
cargo fmt --manifest-path ide/host/Cargo.toml -- --check
if ($LASTEXITCODE -ne 0) { throw 'DENIED: consolidated host DAP rustfmt contract failed' }
cargo build --manifest-path ide/supervisor/Cargo.toml --features $features --bin windows_recovery_service --bin windows_null_service_bootstrap --example wo6_native_dap_acceptance
if ($LASTEXITCODE -ne 0) { throw 'DENIED: native Rust protected host compile failed' }
$svc=(Resolve-Path 'ide/supervisor/target/debug/windows_recovery_service.exe').Path
$hostExe=(Resolve-Path 'ide/supervisor/target/debug/examples/wo6_native_dap_acceptance.exe').Path
$bootstrap=(Resolve-Path 'ide/supervisor/target/debug/windows_null_service_bootstrap.exe').Path
$installed=$false
$nulTouched=$false
try {
  & 'ide/supervisor/fixtures/Install-ProtectedRecoveryService.ps1' -ServiceExecutable $svc -ExpectedServiceSha256 (Get-FileHash $svc -Algorithm SHA256).Hash -ApprovedHostExecutable $hostExe -ExpectedHostSha256 (Get-FileHash $hostExe -Algorithm SHA256).Hash -CiAcceptanceUnsigned -Start
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: native restricted SCM service setup failed' }
  $installed=$true
  $nulTouched=$true
  & $bootstrap --grant-fixed-service
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: exact service NUL grant failed' }
  & $bootstrap --verify-fixed-service
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: exact NUL service state missing' }
  $env:LU_IDE_WO6_DAP_TRUSTED_SERVICE=$svc
  $env:LU_IDE_E2_NATIVE_TRACE='1'
  & $hostExe --project-debug-owner
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: actual Win32 LLDB owner-event source/journal normal run failed' }
  & $hostExe --project-debug-owner-cancel
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: actual LLDB owner-event cancellation/restart failed' }
  & $hostExe --project-debug-owner-crash
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: real LLDB owner-event crash/Job descendant reaping and fsynced restart recovery failed' }
  # D6C extends this SAME grouped real Windows native acceptance batch.
  & $hostExe --project-debug-step
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: protected CI step missing fresh independent OS Job/PDB source and durable ResultVerified receipt' }
  & $hostExe --project-debug-breakpoint
  if ($LASTEXITCODE -ne 0) { throw 'DENIED: source-only breakpoint was accepted without strict uncertain outcome' }
  Write-Host 'DEV045D5_D6C_WINDOWS_OWNER_NORMAL_CANCEL_CRASH_STEP_BREAKPOINT=PASS'
} finally {
  if ($nulTouched) {
    & $bootstrap --revoke-fixed-service
    if ($LASTEXITCODE -ne 0) { throw 'DENIED: exact service NUL revocation failed' }
  }
  if ($installed) {
    & sc.exe delete LuIdeRecoveryBroker 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'DENIED: restricted SCM service cleanup failed' }
  }
  $global:LASTEXITCODE=0
}
