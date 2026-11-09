$ErrorActionPreference='Stop'
$features='dap-transport,protected-recovery-service,dev045d5-ci-event-owner'
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib windows_event_owner
if ($LASTEXITCODE -ne 0) { throw 'DENIED: event wire/sequence/revocation unit tests failed' }
cargo test --manifest-path ide/supervisor/Cargo.toml --features $features --lib windows_kernel_pc
if ($LASTEXITCODE -ne 0) { throw 'DENIED: native Job OS owner tests failed' }
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
  Write-Host 'DEV045D5_REAL_WINDOWS_OWNER_EVENT_NORMAL_CANCEL_REPLAY=PASS'
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
