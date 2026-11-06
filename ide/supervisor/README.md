# Lu IDE Wave D3 — Production supervisor control plane

**Implementation is intentionally non-executing until independent native OS
containment acceptance.** This crate does not depend on Lu Core or Tauri, does
not execute arbitrary project files, and is not wired to a Run button.

## Implemented

- Bounded host-side job plan: fixed recipe IDs, exact project/source/manifest
  hashes, actual input snapshot digest, toolchain digest, a declared effect
  class and enforced upper bounds for runtime, memory, PIDs and log bytes.
- Fresh per-job approval, scoped to plan fingerprint and source HEAD, produces
  an in-memory one-use grant record (60-second TTL) that is never serialized
  into receipts or emitted as an executable desktop capability. Historical D1
  review notes are intentionally **not** accepted as grants.
- Append-only JSONL state machine with explicit
  prepared/authorized/admission/running/cancel-requested/cancelled/failed/
  completed/denied/recovery-required/uncertain transitions. Receipts include
  sequence, previous hash, SHA-256 chain, session and source/plan coordinates.
  Every append is fsynced. Corruption and partial writes fail closed. Hashes
  detect accidents and naive edits; a local user able to rewrite the entire
  file can recompute this unsigned chain. It is **not a signature or MAC**.
- Private native cancellation orchestration logs cancellation **before** OS
  termination, then calls a broker that must verify the *entire* process tree
  is gone before writing a cancelled receipt. Failed termination records
  uncertain and quarantines all new jobs.
- Crash/restart detection: in-flight admission/running/cancelling receipts are
  flagged as requiring reconciliation. No automatic re-launch. An explicit
  native OS broker must prove orphan cleanup; otherwise the operation remains
  uncertain. Previous-session authorizations are not recovered.
- Unit fixtures simulate broker cleanup success/failure and crash boundaries,
  including duplicate/invalid state transitions, tampering, missing snapshots,
  replayed human approval and incomplete journal data.

## Deliberately not claimed as implemented

There is **no release-build worker launcher** in this crate. The
`production_worker_admission` gate always returns
`SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`. The native termination broker
interface is private and only the unit tests provide a simulated broker.

The Windows Job Object probe from D2-W1 passed native suspended admission,
process quota, deadline/descendant kill. **Job Objects do not isolate file or
network access.** Restricted tokens/AppContainer (or equivalent), safe
temporary output namespace, credential-free environment, OS CPU quotas,
process restart cleanup, and security-policy and path-race tests are required
before Windows can execute project code.

The Linux Bubblewrap fixture passed fixed namespace/network/read-only tests,
but cgroup-v2 delegation and resource quotas, descendant reaping, pre-exec
containment, pinned directory rights, seccomp and crash-orphan cleanup remain
open. Fail closed when prerequisites are unavailable.

The supervisor plan's `inputSnapshotSha256` is a contract field; **the actual
trusted input snapshot publisher/broker is not yet implemented**. A clean Git
HEAD alone does not certify ignored files, unsaved buffers, external deps or
compiler binaries. Do not enable execution without a validated native input
snapshot and grant.

## Next integration gates

1. OS-specific restricted worker admission on Windows and Linux, with actual
   rights enforcement and default-deny file/network policy.
2. A native broker binding immutable source, toolchain and output directories,
   with a secure per-job one-use authorization consumed atomically at launch.
3. Real process-tree cancel/timeout on both platforms, log/artifact size caps,
   crash recovery and orphan reconciliation under kill/restart injection.
4. Exact-head Windows/Linux native tests and physical installed desktop
   acceptance before registering any `job.run` capability.

This crate is intentionally isolated so Wave D3 can be tested without
accidentally widening the existing standalone desktop's permissions.
