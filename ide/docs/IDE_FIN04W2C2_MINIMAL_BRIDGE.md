# FIN-04W2-C2 — replacement restricted Windows admission bridge

**State:** Source replacement / not native accepted. Draft stacked on original
FIN-04 W2 PR #227. All LLDB, PE/PDB, debugging, installed release, W2-D
and user-approved real execution gates remain blocked.

## Reason for replacement

Original W2-C [run #43483699020](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43483699020)
passed native compilation, Windows SCM RUNNING, seeded owner-only file,
WFP active rule and original `Prepared` receipt, then denied with
`SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED` before the original six-phase
transaction or independent service one-use record. The legacy host also
constructed an all-true `OsContainmentProof` from a CI environment flag
after reading one service byte, a weak proof contract. Do not repair the
old implementation indefinitely or assert LLDB was at fault.

## Replacement implementation

- Removed legacy feature `fin04w2-approved-fixed-exec` and its
  `fin04w2_authority.rs`, `windows_fin04w2_host.rs` and 108-byte
  executable packet/parser. No old alternate execution route remains
  in the rebuilt configuration.
- `fin04w2-c2` is a **default-off** Windows debug-only feature,
  still permitting **only** the hash-pinned first-party harmless
  `windows_fin04w2_fixed_probe.exe`. No source/command/worker/PID
  selection, LLDB, attach, DAP or model adapter.
- The original SCM restricted token, independently protected pipe,
  ProgramData ACL, independently held Job, exact memory/process/CPU
  limits, kill-on-close and Windows firewall restrictions are preserved
  from W2-A/B native evidence, not recast as user-controlled flags.
- `fin04w2_c2.rs` holds all *non-OS* original journal verification.
  It replays the whole SHA-chained original journal, checks exact Job,
  session, sealed plan and precisely the required progression at
  `Authorized`, `Running`, and `ReleasePending` with a 60-second
  authorization age cap. It reserves and fsyncs one service-owned
  admission record **before** a suspended worker is created, and reads
  it back. Interrupted reservations permanently block replay.
- The service creates and verifies the *exact* owned kernel Job,
  hash-verifies the fixed benign EXE and places it **suspended** into
  that Job, then sends `S` through an authenticated peer connection
  with a fresh service nonce. The only client request is 74 bytes,
  including fixed opcode/version, original Job identity, sealed plan
  digest and service-generated nonce. Unknown verbs, trailing bytes,
  changing Job or stale nonces are rejected.
- The trusted native host obtains an `TrustedC2Admission` with
  private fields only after authenticated SCM peer identity/image
  and `S`. It then consumes the ORIGINAL live in-memory human grant
  without constructing an all-true synthetic OS proof, fsyncs
  `AdmissionPending` and `Running`, and signals `G`. The
  service independently replays the original journal at Running
  before resuming.
- Actual kernel Job accounting must then observe fixed parent and
  descendant, denied owner-only file operations, denied outbound
  socket and an empty tree before service signals `X`. Host fsyncs
  `ReleasePending`, sends `R`; service independently verifies it,
  closes its Job handle and sends `F`. Only then may the host fsync
  original `Completed`. Interruptions conservatively quarantine.
- Existing original CI host/SCM/ACL/WFP fixture is re-targeted to
  this **replacement feature** so its six original receipts,
  persistent service one-use marker, restart and replay denial must
  pass at the **same Windows source SHA**. Fixture simulated approval
  is labeled `CI_SIMULATED_NOT_PHYSICAL`; never physical consent.

## Acceptance gates

1. Original Windows/Linux native Rust compile, pure C2 packet/replay
   negative tests, and unchanged W2-A/B kernel and WFP regression.
2. Original Windows six-receipt benign transaction, independent
   service-owned fsynced one-use record, precise job identity,
   suspended-before-resume, and restart/replay refusal. Preserve three
   independently hashed original artifacts.
3. Genuine native IDE OS-dialog approval plus original installed
   source/build receipt and physical Windows custody. **Not yet
   implemented or authorized**.
4. Only then reviewed LLDB under the *same service-owned custody*
   boundary, stopped thread/RIP/PE/PDB; W2-D hostile message,
   revocation, cancellation, service/guardian death, restart and
   consolidated Windows/Linux acceptance. No installation allowed
   from CI-simulated fixture approvals.

## Independent work

FIN-03/6E-05B continues separately on draft PR #230. Its native
GitHub read-only credential-vault contract, six fixed live ref/policy
reads and source recheck do not grant remote mutation or affect
FIN-04's unchanged original PR #227 acceptance.
