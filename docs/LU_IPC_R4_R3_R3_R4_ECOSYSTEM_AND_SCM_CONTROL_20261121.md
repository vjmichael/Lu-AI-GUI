# LU-IPC-01D-R4-R3-R3-R4 — Native SCM control and Lu ecosystem compatibility
Date: 2026-11-21. **Status: draft source candidate; not production installed acceptance.**

## Decision: one public gateway; independent specialist adapters

Keep **one** Lu-owned canonical Protobuf contract:
\`src-tauri/lu-core/proto/lu/workspace/v0/workspace.proto\`,
package \`lu.workspace.v0\`, wire **major 1/minor 0**. Its Git blob at this
review is \`4e01d921197fb1c58c99958479b1cede36e13c9a\`.
All first-party DLCs use the same canonical 4-byte big-endian length/framed
Protobuf, 256 KiB outer frame / 64 KiB typed message, version negotiation,
OS-local Windows named pipes or Linux AF_UNIX sockets, and package-bound
authenticated Hello/Challenge/Proof/Welcome. The peer's claimed identity,
OriginRefs, manifest, object locator or signature are not effect grants.

A single *protocol* does NOT imply a single binary module, runtime, token
or unrestricted mutual trust. Each extension keeps its own standalone
domain-specific schema, native worker protection, exact-project consent,
journal and independently verified results. Lu Core is the hub that routes
typed capability calls, cancellation/reconciliation, progress and shared
inference. Specialist-to-specialist workflows are Core-orchestrated; no
DLC-origin launch of another DLC's privileged process.

| Provider | Exact reviewed source | Public adapter boundary | Required Core-side work |
| --- | --- | --- | --- |
| **NVP / Film** | \`vjmichael/nvp_offline_film\` convergence \`7ece5eb6efec8007fed867bfcf86a59770f081a5\`; \`native/nvp_lu_ipc_client/src/lib.rs\`; \`backend/nvp_lu_ipc_v0.py\` | Rust/prost Lu v0 Health + Film \`nvp.film.v1\` typed data. Keep NVP native scene/take/audio ownership and its protected worker. | #245 installed Health; #246 ObjectRef resolver and explicit EXECUTE+WRITE_WORKSPACE effect composition; protected NVP creator grants, cancellation and receipts |
| **IDE** | \`vjmichael/Lu-AI-GUI\` draft IDE PR #236 \`f1f0ed4bc1e740d2a9f85615b988b779a46651db\`; \`ide/proto/lu/ide/v1/workspace.proto\` | \`lu.ide.v1\` is IDE's **internal project/UI protocol**, not an incompatible replacement for Lu v0. An external Lu v0 adapter remains separately installable/optional. | #245 installed genuine Health + enrollment. Preserve IDE's separate project, edit, build, debugger, SCM and revocation gates. |
| **Cognitum** | \`vjmichael/cognitum\` \`integration/cog-lu-ipc01e-r2-custody-20261121\` at \`f4edf47428bc55e61eee707d65104ccba68c1141\`; \`cognitum/mel/lu_ipc_native.py\`, \`lu_ipc_lifecycle.py\`, \`lu_ipc_outbox.py\` | Same Lu v0 transport; separate typed MEL and SSM Protobuf. Cognitum's R6 durable outbox/revision evidence is not a Lu grant. | #245 canonical Health; #248 explicit PURE_COMPUTE effect (never recast as READ), registered typed SSM, independent signer/receipt and output-release grants. |

These are source-level observations, **not** evidence of installed connected
Core <-> DLC sessions. Clients must pin a reviewed Core schema, reject
unknown major and unsupported minor/features, and preserve exact type URLs.
Compatibility tests should reuse the same Core-maintained fixture vectors for
Hello/HMAC, frame length, replay, negative credentials, session epoch, Health,
Cancel, Reconcile, stream credit and typed results; do not fork authorization
rules into separate Python/Rust convenience layers.

## Separate private SCM control plane

**NOT public Protobuf**: \`\\.\pipe\LuCoreIpcScmControlV1\` is a
fixed-purpose binary control envelope between the trusted installed Core
and \`LuCoreIpcSupervisor\` running as unimpersonated LocalSystem.

Operations: authenticated STATUS, fixed EXECUTOR_LAUNCH,
STOP_EXACT_EXECUTOR and INSPECT_SIGNED_STOP only. Never expose this service
as a \`WorkspaceFrame\` capability, and do not accept arbitrary executable,
shell argv, PID, Job handle, token, signer key or file path from a DLC.

The SCM checks all of: protected installation record and DACL (SYSTEM and
Administrators), exact Core image bytes and embedded trusted publisher,
actual kernel pipe client PID/birth/session/SID/message token, signed bounded
command using an independently enrolled per-user CNG P-256 key, one random
per-connection challenge, own original service generation, original
call/fingerprint/module/session/Core epoch/conversation/user turn/task/workspace,
and original one-use Job lane. A valid signature alone is **never permission**.

**Fixed launcher** (source candidate): SCM duplicates only the accepted
Core kernel peer's user primary token; strips privileges; asks Windows to
launch the installer-pinned signed worker with the pre-installed
\`LuCoreIpcExecutorV1\` zero-capability AppContainer profile, CREATE_SUSPENDED,
no user-supplied arguments. It independently observes TokenIsAppContainer and
non-SYSTEM user identity, assigns the *original* new private kill-on-close Job,
verifies Job membership, and then resumes the held original thread. SCM retains
the handles; no Core/DLC process can become their owner.

The signed service admission intent is persisted before resume. For every
genuine executor Job-empty stop the service signs and write-through persists
the original-process-generation receipt with CNG machine ECDSA P-256, in
protected non-reparse \`ProgramData\Lu\IPC\scm-executor-receipts-v1\`.
The Core obtains the bounded signed bytes over its authenticated service pipe
and verifies them against a *separately installed Core-side* public pin.
The original exclusive SQLite grant preserves the same origin and service
epoch. A successfully verified stop may convert EXECUTING to UNCERTAIN,
**never** to APPLIED; no auto-reexecution or clearing the restart barrier.
A crash between admission fsync/resume/stop/receipt is UNCERTAIN, not proof.

### Required signed installer artifacts (missing until physical acceptance)

* Fixed, Authenticode-trusted SCM executable and independently approved image
  SHA-256 / publisher certificate SHA-256, registered as fixed-name SCM OWN_PROCESS
  SYSTEM service; program files location is immutable and ACL protected.
* Protected \`ProgramData\Lu\IPC\enrollments\scm-control-core-host-v1.txt\`
  (exact signed Core image path, image SHA, signer SHA, installed Core user SID).
* Protected \`scm-control-core-pub-v1.bin\` with independently provisioned public
  72-byte CNG P-256 blob; non-exportable per-user Core private key
  \`LuCoreIpcCommandP256V1\`. SCM never creates this key automatically.
* Protected \`scm-receipt-pub-v1.bin\` and separately pinned Core-installed
  public verification key; machine CNG \`LuCoreIpcExecutorStopP256V1\` private
  key (non-exportable), preprovisioned and verified before SCM RUNNING.
* Protected \`scm-fixed-executor-v1.txt\` (fixed executable path, image SHA,
  signer SHA), pre-created \`LuCoreIpcExecutorV1\` AppContainer profile and
  SYSTEM-only receipt root. No unsigned worker fallback or automatic profile
  creation in the live service.

### Explicit unresolved blockers

This candidate is NOT an installed authorized tool execution pipeline.
The protected fixed worker does not yet receive a typed capability payload
or an independently attested end-to-end native-tool result. The original
user-turn grant must be fenced synchronously through
\`ConversationAuthority::with_authorized_worker_launch\`; a standalone signed
SCM message must never bypass revocation races. Public DLC EXECUTE/WRITE and
Cognitum PURE_COMPUTE remain disabled until #245/#246/#248 admission, type
mapping, resource policy and installed cross-process tests.

A confirmed Windows native compiler/test pass is needed for all new Win32 FFI.
An installed physical Windows test must observe genuine SYSTEM SCM service,
original non-SYSTEM AppContainer token and Job kernel active-zero, installer
publisher/provenance, replay/downgrade refusal, disconnect/revocation,
SCM crash before/after spawn, Core crash, machine reboot and power loss,
signed result and original SQLite reconstruction. **No Linux Debian test
can satisfy Windows OS custody**. Do not dispatch individual CI batches for
each repair; reconcile one exact-SHA cohort.

## Next integration order

1. Complete actual Core-owned signed-install/Health endpoint (#245) as the
   common baseline for IDE, NVP and Cognitum.
2. Qualify the private SCM launcher under true Windows token and Job custody,
   with synchronous original-turn revocation and typed worker admission.
3. Finish Core explicit PURE_COMPUTE (#248) and NVP multi-effect ObjectRef
   resolution (#246), preserving domain-side approvals and independent receipts.
4. Publish one locked canonical SDK/test-vector compatibility bundle for
   all first-party adapters; independently qualify each installed pair and
   multi-provider workflow without cloning Core's private launcher.
