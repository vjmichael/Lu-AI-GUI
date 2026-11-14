# E1-P06A — typed host-reviewed third-party adapter preflight

## Purpose and scope

The goal is to make an external application's operations intelligible to Lu
without accidentally granting generic shell, arbitrary CLI flags, filesystem,
network or privilege escalation rights. E1-P06A creates a **host-only**, typed
adapter record bound to the existing E1-P01/P02 OS executable verification
and authenticated original user-turn `ExecuteCode` grant. It does **not**
create a tool for the model or execute any third-party program yet.

## Implemented source candidate

- `src-tauri/lu-core/src/native_app_adapter.rs` defines a bounded catalog
  (32 records) with exact application/adapter/tool/workspace identifiers,
  reviewed executable SHA-256 and release label, typed version-only operation,
  host-owned fixed `--version` or `-version` flag, output cap at 4096 bytes
  and maximum 15-second declared execution time.
- The model can submit only the exact fixed typed-operation payload; any
  injected argv, path, model-origin impersonation, workspace mismatch, wrong
  effect, missing current grant or host veto is rejected **before** observation.
  Read-only app discovery is not equivalent to ExecuteCode.
- Preflight reuses the genuine E1 host admission: real local OS executable,
  whole-file SHA-256, PE/ELF signature and current opened-file identity.
  It returns no filesystem path, argv, PID, or credentials, and advertises
  `process_started=false`, `semantic_output_verified=false`,
  `third_party_runtime_sandbox_verified=false` and
  `network_isolation_verified=false`. A reviewed version label is not a
  runtime version query.
- Three Rust tests use an actual Rust test PE/ELF image for preflight, not a
  simulated binary. They cover positive authenticated source and host policy,
  task revocation, separate read-only vs ExecuteCode permission, injected
  CLI text, model forged source, wrong workspace, invalid output cap,
  duplicate/removed adapter and deliberately changed binary digest.
- Native CI matrix is Linux ubuntu-24.04 and Windows 2025, plus exact frozen
  Git inventory and P01/P02 regression retests.

## Deliberate pending gates

**E1-P06B:** a genuinely third-party, host-reviewed application must be
launched with the fixed typed operation through the existing OS supervisor
under independently checked identity, containment, durable journal and per-
turn permissions; verify actual version text with an independent parser and
strict stdout bound, terminal process reaping, cancellation, timeout and
failure recovery on Windows/Linux. The current source is not P06B success.

**E1-P06C:** meaningful typed document/media operations need separate
application-specific arguments and resource broker grants, independently
checked effects and true sandbox/network boundary tests. No arbitrary
shell, PATH search, scripts, untrusted downloaded executable manifests,
self-minted model grants or historical approval replay are enabled.

Prior E1-P03 real process and P04B custody and P05C genuine Qwen three-arm
runtime gates remain open, and installed physical Windows/NSIS/Vulkan release
acceptance is deferred near Lu v1.0. **Source implementation is not native
acceptance.**

## E1-P06B — actual third-party version process (November 7, 2026)

Added the genuine fixed-operation Git version executor using existing OS supervised worker custody, separately bounded stdout, independent Git/Python text validation, journal-backed APPLIED or UNCERTAIN transition, original-turn cancellation/revocation, timeout and exclusive restart reconciliation. Third-party Python interpreter is used for real child kill/reap in test-only compiled fixed sleep/oversize operations; production remains immutable version flags. All real hosted Linux/Windows CI acceptance remains **OPEN** until execution logs are observed; no broad network/filesystem sandbox or general app operation is claimed. See `docs/LU_E1_P06B_NATIVE_ACCEPTANCE_20261107.md`.
