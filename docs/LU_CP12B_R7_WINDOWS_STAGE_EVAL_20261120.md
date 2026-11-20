# Lu CP-12B-R7 — Windows-native stage-review transport and lifecycle qualification

**Date:** 2026-11-20. **Status:** Scoped Windows-native transport acceptance PASS; installed graphical two-model Windows acceptance OPEN. This is development evidence, not a release approval.

## Source and scope

- Starting exact local R6 source commit: `aab5e43f02ebbb270ed2835cd91fe21ecb0068ca`. The R6 source was independently recovered from its saved source bundle and checked before editing.
- The Windows transport in `lu-desktop/src-tauri/src/dx_windows_pipe.rs` now handles authenticated local v1 count-only (`lu1`), v2 independently checked reviewed-count (`lu2`) and v3 stage-level reviewed evidence (`lu3`) invitations using the existing Windows named-pipe and per-account SID protections. Review and stage-review receive operations accept the same cooperative cancellation token as Linux. Invalid local findings are rejected before an endpoint or invitation is created; incoming peer evidence is verified by the existing finite native validators and independently reviewed at the desktop entry points.
- Source cross-platform dispatch in `dx_ipc.rs` enables v2/v3 only on Linux or explicitly feature-enabled `windows_dx_eval`. Feature-off Windows remains unsupported; the feature is absent from normal Cargo defaults. No networking, remote connection, diagnostic prose or repair authority added.
- The standalone Windows transport test crate locks its added test dependency `serde_json` to the original desktop version. A focused UI/security source-contract test covers default-off, native account/endpoint controls and independent consent/model verification.

## Actual acceptance and evidence boundaries

1. **Windows x64 MSVC target:** locked offline feature-on `cargo check --tests` PASS; actual Windows test executable linked successfully using original preserved Windows SDK/MSVC libraries. Target compile and link are not installed desktop acceptance.
2. **Native Windows kernel execution:** original preserved Micro11 Windows Server Core `10.0.25314.1000` under isolated QEMU (no NIC; disposable overlay), official original VC runtime DLLs staged only as the test executable's private runtime dependencies. The unchanged evaluation test executable SHA-256 is `d357f4904fc4b6d262b2015818345d3799e40dfd931f398c0aac8c5d01901021`.
3. **Native whole standalone test cohort:** `18 passed; 0 failed; 7 ignored` (ignored include entries exercised as children by parent tests and optional isolated-account tests); this includes native v1 compatibility, v2 review, v3 stage transfer/replay, malformed/tampered stage evidence, revocation/rotation, mid-frame cancellation, authenticated separate-process exchange, sender death and fresh session, nonce/MAC and privacy rejection. Focused `cp12br7` filter: `6 passed; 0 failed; 3 ignored` (subset of the 18; do not double count).
4. **Native account isolation:** separate native Windows standard account provisioned inside disposable VM by administrator for this exact test, signed into with Windows `LogonUserW` and impersonated for a kernel named-pipe access denial; the original owner's stage exchange succeeded. The single `--ignored --exact cp12br7_stage_different_account_kernel_denial` test **passed 1/1**. The temporary account was deleted and independently confirmed absent, ephemeral credential environment entries cleared, and VM shut down cleanly. This tests real alternate-SID token/ACL enforcement inside the guest, not a fully independent second desktop GUI launched as that user.
5. **Frontend contracts:** 12/12 source-contract tests passed. No newly downloaded model and no Library upload or GitHub CI/PR/merge. No real installed Windows NSIS application or genuine GGUF two-model graphical exchange qualified here.

## Failures and limitations

- Initial test EXE launch exited with Windows `0xC0000135` because the guest lacked the original Microsoft Visual C runtime; provided the original preserved DLLs alongside the isolated evaluation binary, then the unchanged binary executed and passed. The source or security policy was not weakened.
- Full Windows Tauri desktop Cargo check was attempted but halted by this environment's memory/time capacity while building dependencies, once concurrently with the VM causing the host to kill QEMU. The focused Windows transport check/link and native kernel execution are independently accepted; **full Windows Tauri build, installed NSIS UI, normal user model approval, distinct other-user GUI process, physical Windows and release are not accepted**.
- Earlier R5/R5-R1 positive actual fault-localization was unqualified with small GGUF models; R7 adds no model accuracy acceptance. R6 Linux installed results remain separate historical evidence.
- All state and receipts held in local workspace or compact handoff; do not copy unreviewed VM image, SDK, runtime, model, private invite codes or ephemeral account credentials into the Library.

## Next gate

**CP-12B-R7-R1:** Windows desktop feature-on build and installed two-Lu GGUF graphical acceptance on qualified Windows host; perform per-instance human approval/refusal, stage report, cancel/revoke/replay, crash/restart, account isolation with actual other-user process, no network egress and host-owned persistent consent receipts. Keep `windows_dx_eval` disabled in the production default until reviewed native installed and physical release qualifications pass. Then broader CP-12B integration/regression and Core acceptance. Source SHA, package hashes and test evidence must match unchanged tested candidate.
