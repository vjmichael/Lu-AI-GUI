# Lu IDE D7D — Protected Windows Service and Crash-Recovery Acceptance

**State:** source implementation in draft PR #63; real Windows exact-head verification remains **QUEUED / NOT PASSED** as of 2026-11-06. This document does not certify installed Windows security or authorize desktop execution.

## Production protection

- Only explicitly installed \`windows_recovery_service.exe\` starts through the Windows Service Control Manager. Identity: \`NT AUTHORITY\LocalService\` with restricted \`NT SERVICE\LuIdeRecoveryBroker\` service SID; no model, WebView, Tauri Run, user-provided tool command, or uncontrolled child process entrypoint.
- Elevated install policy pins both approved service and host SHA-256; Authenticode required for real distribution. Temporary unsigned fixture is explicitly limited to a hosted GitHub Actions test runner. Existing service/install root is never adopted as trusted.
- Service-owned receipts and config reside directly under \`%ProgramData%\LuIDERecoveryBroker\` (not under a potentially user-controlled intermediate \`LuIDE\` folder). Inheritance removed, explicit SYSTEM/service SID grants only. Runtime independently checks protected DACLs and denies unexpected ACEs.
- \`\\.\pipe\LuIdeRecoveryBroker.v1\` rejects remote clients. DACL prevents ordinary interactive users from creating server instances. Both endpoints independently verify the other: native process PID, creation time, original token user, service restricted SID membership, installed image path and SHA-256. Request scope and nonce use a strict bounded JSON schema. A named pipe request is **not** permission to build.
- The Windows Job Object uses a \`Global\` name to cross Session 0 and an explicit owner/SYSTEM/service access DACL. Before the child is resumed, the service must hold an exact kernel Job handle, verify it empty, fsync a protected READY witness and return a nonce-bound acknowledgement.
- Original owner death or lost authenticated pipe requires tree termination and \`ActiveProcesses=0\`. The independent service retains the original Job kernel handle in a bounded recovery hold.
- Restart recovery requires **two-phase reconciliation**: native host independently opens and verifies the original Job, writes a durable \`RecoveryRequired\` preflight while maintaining quarantine, obtains a service-owned \`recovered_release\` receipt (fsynced before ACK), and rechecks the still-open original Job before writing the terminal \`Cancelled\` supervisor receipt. Missing objects, broken pipe, missing ACK, or failed job query do not clear quarantine.
- The D7C same-user recovery process remains available only for older explicitly feature-gated tests. \`protected-recovery-service\` production build selects the new SCM service lease instead.

## Windows acceptance

Workflow: \`.github/workflows/ide-d7d-protected-service.yml\` (four independent native Windows jobs; two use \`windows-2025\`).

1. **native-windows-service**: pinned Rust 1.88 compile and unit tests, real elevated SCM install, restricted service token and ACL verification, authenticated named pipe admission and normal empty Job release. Reject a forged interactive server pipe and attempted interactive config mutation.
2. **crash-recovery-windows**: compile native protected service and fixed self-sleeper fixture; start the service under its own Windows account; admit Job before suspended child resumes. External PowerShell observer checks live child kernel handle, tries pre-crash recovery (must fail), forcibly kills original host, confirms child kernel exit, reopens the retained original named Job with ActiveProcesses=0, launches a new approved host process for independent Job inspection, obtains the service's protected fsynced recovery ACK, closes observer handle and verifies the original Job name is gone.
3. Native library unit test exercises durable \`RecoveryRequired\` after restart and confirms further work remains quarantined without protected service acknowledgement.

4. **pinned-offline-protected-cargo**: packages the Windows Rust/MSVC/SDK fixture, compiles the real \`d7_windows_acceptance\` example under \`protected-recovery-service\`, pins the executable as approved host, runs an actual isolated Cargo check, verifies durable journal/output on reopen, then forces cancellation and verifies ordered cancellation receipts. In this mode the host never reads ACL-protected service witnesses; it requires the authenticated fsynced service ACK.
5. **service-loss-windows**: authenticates the real native owner/worker, kills the separate SCM service while the host is still alive, requires the native service-liveness watchdog to terminate the Job, and verifies the child exited. After service restart, a missing original named Job must refuse reconciliation.

The crash fixture validates the **kernel/service/reconnect loop**. The full protected Cargo fixture and service-loss fixture add OS evidence but do not stand in for a full installed desktop run of \`execute_windows_worker\`, pinned offline Cargo fixture, D6 real AppContainer network isolation, or independently signed production installer.

## Remaining strict gates

- Run and inspect actual exact-head Windows logs. Repair all compiler, service install, ACL, IPC, hostile pipe and external kill failures; re-run on the repaired exact SHA.
- Test a full real worker build/test transaction with the actual D3 supervisor journal, not just fixed self-sleeper and isolated durable state unit tests.
- Inject service crash, host/service simultaneous crash, truncated/replayed/tampered witness, hung/disconnected pipes and Windows reboot. Missing Job must remain quarantined.
- Verify signed installer provenance and true Program Files installation, offline pinned SDK and D6 network isolation, then installed desktop behavior. Until all pass, keep native Run and Lu Core bridge disabled.


## Latest D6 dependency

Native prior D6 Windows probe exited with code \`1099\` (a loopback socket failure without a raw WinSock code). This is **not** accepted as network isolation. Do not convert timeouts, connection refusal or missing socket errors into WSAEACCES or a passing gate; require a genuine policy-backed proof of denied network access in the AppContainer context. D6 remains an independent release blocker even if the D7D SCM smoke tests pass.
