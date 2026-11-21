# IDE-CORE-IPC-01B — Protected native Health broker client and lifecycle

**Status:** IDE-only native Linux implementation and synthetic installed-boundary
fixture qualified. **NOT** a signed installed issuer, Windows installed admission,
or exchange with a real running Lu Core. Core owns the actual issuer/Health
service and authorization policy (issue #245); no Core code was changed.

## Trust boundaries

1. Only the **installed native host startup** may supply `InstalledIssuerAnchor`
   and the pre-created journal directory. Do not accept their values from a
   WebView/Tauri command, prompt, project, persisted UI settings, environment
   variables, IPC responses or user-selectable paths. The pinned installation
   SHA-256 is evidence to be validated against an independent signed publisher
   manifest at that OS boundary; a caller-provided SHA is NOT attestation.
2. The separate **native credential issuer service** MUST authenticate the
   calling installed IDE's OS identity and verified package/signer, authorize
   precisely `core.health.readonly`, burn each 32-character durable
   `journal_operation_ref` once, apply enrollment revocation, and honor the
   short-lived nonce and expiry. The provided client pins the issuer UID,
   either an exact same-UID private 0700/0600 endpoint OR an exact distinct-UID
   installed 0750/0660 endpoint with an installer-pinned access GID; each
   retains an inode witness and Linux
   `SO_PEERCRED`. These checks do not install or authenticate the issuer's
   own internal authorization policy.
3. The issuer provides a **single transient credential** and exact pinned Core
   owner UID/socket. The IDE verifies the returned operation ref, request
   nonce, original package digest, permission scope, Core owner UID, a
   <=60-second expiry, canonical Protobuf encoding, and a bounded frame
   before invoking the callback. It wipes both decoded credential storage
   and the temporary 32-byte working key after the callback. No secret or
   pathname is written into the journal.
4. The existing read-only Core codec requires Linux socket ownership and
   `SO_PEERCRED` and then verifies the **Core Welcome HMAC** before sending
   one `HealthRequest`. A `HealthReport` is advisory, **never a capability
   grant**. There is no native tool execution, model control, project file
   read or arbitrary Protobuf operation in this bridge.
5. Each native `CoreHealthSession` is consumable once. The journal writes
   `reserved` and `started` markers using exclusive creation, mode 0600 and
   `fsync`, then a single terminal marker (`observed`, `refused`,
   `cancelled`, `revoked` or `uncertain`). A crash between markers is
   **uncertain**, not permission to reconnect. Restart reconciliation scans
   all valid markers with bounded inventory and rejects tampering, unknown
   names or untrusted owner/permissions. No automatic replay is provided.

## Wire contract: IDE to installed issuer

`ide/proto/lu/ide/v1/native_broker.proto` is **an IDE-side proposal for the
native OS credential issuer**, distinct from `lu.workspace.v0` Core IPC. It
uses canonical length-delimited Protobuf frames with a 4-byte big-endian
length (1–4096 bytes), a 6-second absolute exchange limit and nonrenewable
per-I/O deadlines. The service MUST echo both the random 32-byte nonce and
durable operation reference and refuse foreign package or scope. This new
issuer service is not included with Lu Core's Health dispatcher and requires
separate installed-owner implementation/contract review before deployment.

## Qualification performed

- Exact pinned IDE PR #236 checkout plus the previously accepted R5-R5
  IDE-only native host patch; Rust 1.99.1 and offline locked crate inputs.
- Genuine Linux native filesystem and Unix-domain-socket tests, including
  kernel `SO_PEERCRED`, incorrect owner, forged enrollment, expiry,
  operation-reference mismatch, duplicate use, cancellation while waiting,
  journal recovery, corruption and cancellation/revocation default denial.
- End-to-end synthetic issuer -> enrolled local synthetic Core -> authenticated
  HealthReport -> fsynced terminal receipt. This is **not a live installed
  Lu Core service**, publisher signature, OS credential vault or Windows SCM
  broker test.

## Unfinished installed acceptance (must remain fail-closed)

- Hook this module to a genuine installed OS-owned credential service and
  protected native startup, with signed package/publisher pins and verified
  caller admission. No model-facing Tauri command and no credential setter.
- Core-owner implementation of issuer semantics and authenticated registered
  DLC enrollment; issue #245; independently verify the actual live Health
  dispatcher and enrollment issuance, revocation and key rotation.
- Windows named-pipe OS-token/service identity verification, credential/SCM
  custody, installer packaging, physical Windows denial tests.
- Repeated restart/power-loss installed acceptance, service downgrade/rollback
  tests, and real cross-process persistent issuer one-use enforcement.
- Only after these gates can any installed or production `connected` state be
  reported; even then it remains advisory, not a grant to execute tools.

## 01B-R2 — Installed Linux host startup and cross-UID qualification

The native startup has an **opt-in** `native-core-health` Cargo feature in
`ide/app/src-tauri`. It calls only
`InstalledCoreHealthRuntime::from_installed_linux()` in the existing Tauri
`.setup` boundary and, if independently admitted, registers a native-only
runtime. A missing/invalid profile is a silent default-DENY for IPC (the IDE
itself still opens); **startup never borrows credentials or sends Health**.
There is deliberately no WebView/LLM/tool command or public health-probe
method. A future qualified native human-authorization owner must explicitly
admit each one-shot call.

The **fixed, not configurable by users or models** installed profile is
`/etc/lu-ide/native-health-v1.json`. The installer owns the parent as UID 0
with no group/other write, and a regular non-symlink profile as UID 0 mode
`0644`. The JSON object must contain **only** these fields:

```json
{
  "version": 1,
  "client_uid": 1000,
  "executable": "/opt/lu-ide/bin/lu-ide-desktop",
  "executable_sha256": "<64 lowercase hexadecimal characters of that installed executable>",
  "package_sha256": "<64 lowercase hexadecimal characters of the installer-approved package>",
  "issuer_socket": "/run/lu-ide-issuer/lease.sock",
  "issuer_uid": 0,
  "issuer_group_gid": 1050,
  "core_uid": 1000,
  "journal_directory": "/var/lib/lu-ide/user-1000/health-journal"
}
```

**Illustrative values, not deployed authority.** The profile must come from
an approved installer. R2 checks the kernel calling UID, separate issuer UID,
current native executable path, root ownership/mode and actual executable
SHA-256, bounded non-symlink profile, package digest syntax and private 0700
journal ownership. Before admitting a new session it scans existing markers
and never auto-replays an uncertain operation. The installed issuer client
connects with **root-owned directory mode 0750 + socket mode 0660**, each
with the exact pinned access GID; Linux kernel `SO_PEERCRED` still must match
the pinned issuer UID. Private same-UID Core sockets retain 0700/0600.

The access group is only a filesystem **reachability** restriction, not a
publisher/signature or caller-authority proof. The privileged issuer must
*independently* verify actual client PID/UID, installed binary/publisher and
installer package pin, and durably consume the operation ID on its own disk
**before** releasing a short-lived lease. This client does not create that
Core-owned service and cannot make it safe merely by checking the reply.

### R2 independent host evidence and limits

On a disposable Debian 13 root-capable test host, the explicitly invoked
privileged integration fixture creates a real UID 0 issuer Unix socket owned
by GID 65534, starts an IDE client under actual UID/GID 65534 using `setpriv`,
checks kernel peer credentials, loads a root-owned executable-bound profile,
records a one-use operation in a root-owned fsynced issuer burn directory,
and refuses the replayed operation after client restart. The separate IDE
journal durably records uncertain Core outcome, then cancelled and revoked
outcomes on independent new session objects. Cancellation/revocation require
no extra credential issue. These are **test-only synthetic lease bytes** and
an intentionally absent Core endpoint. They prove the Linux OS boundary but
**not** deployment of the issuer or a genuine installed Core Health success.

The cross-UID parent and child test entry points are `#[ignore]` by default,
because ordinary unprivileged CI cannot qualify a true OS identity boundary.
Run deliberately on a disposable root-capable Debian host with:

```sh
cargo test --manifest-path ide/host/Cargo.toml --locked --offline \
  --lib root_owned_real_uid_issuer_custody_and_durable_burn -- --ignored --nocapture
```

The 01B-R2 default host Rust 1.99.1 tests and locked build are separately
qualified. The opted-in Tauri desktop **cannot be completely compiled in the
current bare Debian container**, which lacks `glib-2.0.pc` (GUI dependency).
The modified desktop `src/lib.rs` itself passes `rustfmt --check`; a broader
app Cargo fmt additionally encounters untouched supervisor formatting diffs.
No release signer/installer, systemd/SCM issuer or Windows named-pipe native
custody is included in this patch. No Lu Core source is changed.

R2 also refuses an IDE run as root, mismatched effective/real UID or a
root/invalid issuer access GID; these are explicit denial checks, not a
replacement for the issuer's independently validated caller identity.
