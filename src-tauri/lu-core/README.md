# Lu Core — modular foundation (experimental)

Lu is a **general-purpose AI operating environment** built from the Jan fork.
Domain-specific workspaces (NVP, Code, Research, Writing, IoT) extend Lu Core
without becoming core dependencies. This directory is an isolated integration
foundation, **not yet wired into the shipping Jan desktop or Cowork**.

## Implemented on this branch

- `ansf.rs`: first ANSF (AI-Native Session Format) storage slice.
  SQLite session books with immutable original messages, stable entry IDs,
  explicit topic chapters with summaries, FTS5 index, and bounded cursor
  pagination for future virtualized chat. This is **not yet** integrated
  with Jan desktop threads, context assembly, memory worker, or trusted
  owner verification. See
  [MEMORY_AND_MEDIA_ROADMAP.md](MEMORY_AND_MEDIA_ROADMAP.md) for intended
  conversation and streaming media milestones. Voice, camera and A/V are
  optional future core extensions; workspace DLCs come after Lu 1.0.
- `ansf_import.rs`: streaming, bounded, **non-destructive** importer for
  Jan desktop `messages.jsonl` records. Retains raw source JSON as immutable
  ANSF content, stable message IDs, speaker roles and searchable summaries.
  Re-running after interruption skips matching originals; edited Jan rows
  fail closed unless a trusted host confirms the incoming snapshot is newer.
  The explicit newer-snapshot mode appends an immutable linked revision. Reads supplied by
  a trusted caller, never arbitrary model-selected paths. Includes tests
  using a real historical Jan JSONL fixture. No live desktop migration yet.
- `ansf_revisions.rs`: append-only message revisions, immutable edit links,
  original-to-current resolution, durable revision history and UTC edit-event
  timestamps. The Memory pane has not been wired yet. Direct authenticated
  edits may be labeled `Edited`; imported changes must be labeled `Source
  update detected` because their actual human edit time is not established.
- `ansf_library.rs`: SQLite global catalog of conversation books and chapter
  summaries; FTS5 search across sessions, stable source references, and
  idempotent reindexing without copying original messages. It does not open
  referenced files or verify caller identity; Jan must provide trusted path
  confinement, owner checks, ingestion/reindex and search UI.
- `contracts.rs`: versioned core/workspace manifests, tool descriptors, effect
  declarations, native async tool interface and caller-supplied authorizer.
- `core_extension.rs`: trusted-host registration and independent unload of optional Lu-native Core Extensions (CEs). No MCP or untrusted package execution; this is a lifecycle foundation, not an installer.
- `registry.rs`: conflict-safe, atomic in-memory native registration, lookup,
  dispatch with authorization/type checks, and protected core registrations.
- `proto/lu/v1/core.proto` and `wire.rs`: generated Protobuf v1 call/reply
  envelopes, size/version checks, typed message names and explicit outcomes.
  `proto3 optional` distinguishes absent workspace/diff from empty values.
- `workspace_quarantine.rs` ("Sandbox Hell"): bounded inert intake for
  untrusted workspace DLCs. It hashes opaque archive bytes, prevents identity
  replacement, and allows terminal rejection **without extracting, importing,
  executing, registering a tool, or granting anything**. No approval-to-run
  operation exists in this stage. The receipt is currently in memory only,
  and SHA-256 is NOT publisher authentication or malware detection.
- `workspace.rs`: trusted-host registration, active/suspended instance states,
  instance-to-module authorization and guarded uninstall.
- `journal.rs`: SQLite WAL/FULL-sync durable effect intent and receipts,
  PREPARED → EXECUTING → APPLIED/FAILED/UNCERTAIN state transitions and
  restart reconciliation. Replayed effects are **not** automatically rerun.
- `durable.rs`: opt-in native dispatcher using journaled call IDs and stored
  Protobuf receipts; ambiguous execution failures become UNCERTAIN.
- `tasks.rs`: SQLite-backed task state, durable incremental checkpoints,
  heartbeats, progress and interrupted-state recovery. A heartbeat does
  **not** impose an arbitrary timeout.
- `model.rs`: allowlisted OpenAI-style model-facing tool catalog; strict
  JSON argument checking and typed Protobuf dispatch/results. The catalog
  never grants execution permission by advertising a schema.
- `native_files.rs`: first two actual bundled Lu-native tools,
  `lu_files_read_text` and `lu_files_list_directory`. They are read-only,
  require a selected workspace root and policy authorization, and use
  capability-based `cap-std` directory handles to constrain opened paths.
- `conversation_authority.rs`: **Conversation-Bound Authority** for model
  actions. The trusted host must verify the originating, persisted USER message
  and conversation owner before opening a task. Every task carries an immutable
  conversation ID, user-turn ID, task ID, exact execution scope, allowed
  tool/effect pairs, and optional expiration. Untrusted tool results, pages,
  documents, and model-generated claims cannot mint new grants. The safe
  `dispatch_bound` entry point rechecks origin, scope, effect, expiry,
  revocation, and the host's independent authorization policy. Harmless,
  already-delegated operations need no repeated confirmation.
- `privilege_guard.rs` (nickname: **Mythos Containment Guard**):
  non-overridable model privilege ceiling. Native dispatch, PolicyEngine grant
  issuance, durable receipt replay, and model-facing structured errors all
  reject declared `Elevated` and `DeviceControl` effects. A model persona
  cannot obtain an exemption by calling itself "safe" or providing a generous
  authorizer. A future hardware workspace needs a separate authenticated,
  physically fail-safe device controller—not a bypass switch.
- `policy.rs`: deny-by-default, exact-tool/exact-session capability grants,
  effect checks, optional explicit expiry and immediate revocation. Host-side
  authentication and durable approval auditing remain necessary before release.
- `tests/*`: registration, permissions, workspace lifecycle, Protobuf, effect
  journal, duplicate-effect blocking and task checkpoint test coverage.
- `../plugins/tauri-plugin-agent-tools/src/lu_bridge.rs`: **feature-gated**
  native compatibility adapter wrapping Jan's built-in tools, without MCP.
  Reuses Jan's permission gate and denies operations needing an unanswered
  permission prompt. Legacy JSON tool args are contained in a transitional
  Protobuf message; new tools should define their own typed Protobuf requests.
- `.github/workflows/lu-core-ci.yml`: tests, Clippy and headless legacy bridge
  compilation/integration tests (sandbox-dependent pre-existing Jan shell
  tests are not treated as bridge tests).

## Security decision: non-admin is a ceiling, not a grant

See [SECURITY_AUTHORITY_AND_OS_CEILING.md](SECURITY_AUTHORITY_AND_OS_CEILING.md)
for the accepted user clarification and desktop integration checklist.

Lu's model-driven tools must operate with **no more than non-administrative OS
rights** and **less access than the human account whenever possible**. Standard
user credentials still reach private files, browser profiles, SSH keys and
network endpoints, so running the desktop as a standard user is NOT an adequate
sandbox by itself. Execute code/untrusted tools in independent, OS-constrained
workers with task-specific file/IPC/network grants and no administrative
inheritance. Never use model output to request UAC elevation or create grants.

This is an architecture requirement rather than a completed runtime
guarantee. `privilege_guard.rs` blocks *declared* elevated/device effects
and does not implement OS process isolation. The eventual trusted host must
enforce the ceiling even when a tool implementation is buggy or compromised.

## Conversation authority and prompt injection boundary

An AI **requests** actions; only the authenticated human via the trusted host
can delegate authority. Each tool call must trace to a specific persisted
user-authored message or explicitly authorized task rooted in that conversation.
A task can have multiple ordinary tool calls without repeated permission
prompts, but it can never silently gain new capabilities from a retrieved
document, tool response, model-generated plan, or another conversation.

**This is not complete prompt-injection prevention.** Untrusted content may
still manipulate a model into choosing bad actions *inside* broad preexisting
grants. The host must verify the action's purpose, specific arguments, target
resources, data-exfiltration destinations, and high-risk outliers. For example,
"read this PDF" does not justify "upload unrelated documents" even if Lu has a
network tool registered. Typed argument constraints, legitimate task plans,
revocation after user goal changes, and a privileged-action approval flow are
future desktop integrations.

**Not yet enforced by shipping desktop or Cowork:** the independent ledger is
a tested Lu Core contract, but Jan must implement a `TrustedUserTurnVerifier`
that reads persisted conversation records, authenticates their owner and role,
and binds the origin to Rust-hosted tool dispatch. The model/webview must never
choose that trusted origin. Existing registry `dispatch` remains available as a
compatibility path and therefore is not conversation-bound by itself. Durable
replay and in-flight operations need binding and cancellation too.

## Untrusted DLC policy ("Sandbox Hell")

We cannot control what a malicious developer puts in a workspace package.
We **can** control whether Lu ever executes those bytes or grants them tools.

**Default: external DLCs are inert and untrusted.** Download, archive metadata,
manifest text, tool schemas, retrieved docs and scripts are **data**, never
authority. The new `WorkspaceQuarantine` records an archive digest without
unpacking or registering native implementations. Suspected malicious archives
may be rejected; there is NO callable promotion or execution path yet.

To implement real DLC loading in a later milestone, require all of:

1. Publisher identity verified cryptographically against explicit user/host
   trust policy; archive integrity checked before and after staging (a digest
   alone provides no trust).
2. Validate decompression, paths, symlinks, file types, compression ratios and
   resource limits outside the privileged Lu process.
3. Distinguish safe declarative UI contributions from executable code;
   neither manifest claims nor scripts may register privileges.
4. Launch external code only in OS-enforced, non-admin, isolated workers
   (on Windows investigate restricted token/AppContainer/LPAC) with no inherited
   Lu process secrets, browser credentials, operating-system handles, host
   filesystem access or unrestricted network.
5. An authenticated conversation/task and narrow per-operation grants are
   mandatory. Tool calls with missing origin or mismatched scopes are denied.
6. Network egress is denied unless a specific destination is authorized;
   physical-device control and elevation remain unavailable through Lu's
   agent dispatcher.
7. Audit approvals and tool effects, support immediate revocation, terminate
   suspect worker processes and reconcile outstanding file effects.
8. Test malicious manifest injection, DLL/shared-library loading, plugin
   spoofing, ZIP traversal, cross-workspace tool escalation, symlink races,
   sandbox escapes, and compromised worker behavior on supported operating
   systems.

**Integration status:** external workspace loader does not exist yet.
`WorkspaceHost::register_workspace` is still a *trusted-host API* accepting
preconstructed Rust implementations; it must **never** be called on behalf of
untrusted package bytes. Do not interpret the inert intake service as a
working runtime sandbox or a complete installation gate. A future loader
must be incapable of bypassing the quarantine and isolated-worker boundary.

## Intentional boundaries and non-goals

This is **not yet a release-ready agent runtime**.

- The separate Tauri desktop adapter now has a feature-gated connection to
  **ordinary chat only**, exporting confined read-only scratch-workspace
  tools. Normal Jan builds and Cowork remain on existing dispatch; the desktop
  integration is experimental until its Windows CI and manual smoke tests pass.
  The parent app's `lu-foundation` Cargo feature is opt-in.
- The adapter demonstrates reusing Jan native tools, but is not a new tool
  installer, dynamic worker loader, or external MCP server. It is dormant
  without the opt-in `lu-bridge` feature.
- Protobuf envelopes are generated and decoded; per-tool request bytes must be
  decoded/validated using the schema for that tool, including payload limits.
  A legacy JSON string inside Protobuf is a transitional compatibility path.
- A strict model-facing JSON catalog, typed Protobuf codecs and result envelope
  now exist for the first two bundled tools, but full model-provider-specific
  integrations, resilience against malformed tool streaming, and actual Tauri
  IPC routing remain future work.
- Workspace packages are supplied by a trusted host. No artifact signing,
  dependency solver, untrusted package execution, package installation, or
  live hot-reload is implemented. The workspace host checks instance-to-module
  bindings; a direct registry user must install an equally strict policy.
- Effect journal and task checkpoint storage are implemented, but the
  production orchestrator/worker scheduler does not use them yet. A single
  durable owner and a startup recovery protocol must be enforced at integration.
  There is no automatic recovery of external/physical side effects.
- Atomic registry unload blocks **new** calls; already-running calls need a
  separate worker cancellation/termination protocol.
- The bundled read-only filesystem tools use capability-relative handles, but
  production Windows/Linux/macOS adversarial testing, startup trust binding,
  revocable user consent and sensitive result handling are still required.
- The model privilege ceiling checks the DECLARED `ToolEffect`; it cannot
  attest arbitrary tool code. Running a native application as administrator
  would still be unsafe, and code-execution tools require independent OS-user
  isolation and sandboxing. The policy itself has no override for `Elevated`
  or `DeviceControl`. Denying declared device control blocks future
  sprinklers/toasters until a separate, carefully reviewed safety service
  exists.
- No complete generalized native toolbox, NVP workspace, sandbox policy
  replacement, or comprehensive cybersecurity guarantees are claimed.

## Validation

From the repository root with Rust installed:

```sh
cargo fmt --manifest-path src-tauri/lu-core/Cargo.toml -- --check
cargo test --manifest-path src-tauri/lu-core/Cargo.toml --all-targets
cargo clippy --manifest-path src-tauri/lu-core/Cargo.toml --all-targets -- -D warnings

# Headless compile and bridge-specific tests; uses no MCP server:
cargo check --manifest-path src-tauri/plugins/tauri-plugin-agent-tools/Cargo.toml --no-default-features --features lu-bridge
cargo test --manifest-path src-tauri/plugins/tauri-plugin-agent-tools/Cargo.toml --no-default-features --features lu-bridge lu_bridge::tests::
```

The GitHub Actions run for this branch is the authoritative compilation record.
Earlier runs proved the initial registry/Protobuf/journal/workspace tests passed
but revealed a Rust ownership error (fixed) and a Clippy style issue (fixed).
New commits require fresh validation before merging. Running the ENTIRE legacy
Jan test suite on GitHub-hosted Linux also revealed expected shell-sandbox
environment failures; those tests must be rerun where sandbox enforcement is
available. This chat itself has no local `cargo` toolchain.

## Next integration stages

1. Get all Lu Core and bridge-specific CI checks green, pin dependency versions,
   and verify on Windows (the primary user target).
2. Extend the now-implemented allowlisted model-facing JSON tool catalog and
   typed Protobuf bridge with additional audited native capabilities. Keep MCP
   optional. Avoid advertising huge tool sets to small local models.
3. Wire the opt-in Lu host feature into an authenticated desktop service
   with the existing policy checks, user approvals and workspace instance
   ownership; compare Jan's legacy behavior before changing Cowork.
4. Integrate the user's original, advanced native toolkit. Audit each tool's
   permissions, structured error handling, recoverability and Windows behavior.
   Reuse Jan safeguards selectively instead of cloning its limited tool set.
5. Integrate durable task/effect checkpoints with real workers, cancellation,
   deterministic restart, and independent result/artifact storage.
6. Implement signed or otherwise explicitly trusted workspace installation,
   UI panels, module update/removal lifecycle and per-module scopes. Then build
   specialized NVP/Code/Research/Writing/IoT packages.
7. Migrate ordinary chat and Cowork to one coherent capability catalog while
   keeping context-specific policies and gradual rollback/fallback.
8. Run end-to-end tests on local/cloud models, tool-invocation failure cases,
   malicious/accidental input, memory persistence and crash recovery.

## Machine ethics and software governance

The registry's authority boundary is **not** a moral theory. A useful machine
ethics framework should coexist with independently enforceable controls:
versioned rule ownership, authorization for policy changes, provenance,
auditability, least privilege, reversible decisions and explicit emergency
stops for physical devices. An AI should not grant itself new privileges simply
because a model or third-party module rewrites a policy statement.

- `ce_packages.rs`: strict bounded CE manifest parsing, dependency ordering and version checks, inert quarantine, and trusted bundled read-only lifecycle. This does not execute external packages, verify signatures, or provide OS process isolation.

- CE registry dispatch now acquires a per-module in-flight lease. Disabling a busy CE closes new-call admission and returns `CallsInFlight`; the host must wait or cancel the outstanding future and retry. This is a cooperative drain barrier, not a worker kill switch or an OS sandbox. Untrusted executable CEs remain quarantined.
