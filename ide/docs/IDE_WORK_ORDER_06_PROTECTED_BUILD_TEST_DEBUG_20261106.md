# IDE WO6 — Protected build/test and native debugging

**Status:** started, implementation active, **no debugger execution or new desktop Run permission enabled**. Source branch `ide/wo6-protected-build-test-debug-20261106` stacked on WO5 PR #69 at commit `785ca287d74c39f525e0f0bfc4e6e30d65afd063`. WO4 delegated Linux recovery and WO5 native clangd admission/installed LSP acceptance remain independent open gates. Do not promote D1 user review or editable project files to execution authority.

## Acceptance scope

WO6 is not just an editor button. A successful protected build, test, or debug requires actual **native OS execution** from one fixed approved recipe with an exact immutable source seal; pinned trusted toolchain/adapter and dependency closure; fresh native one-use approval; restricted Windows AppContainer + protected SCM Job or Linux cgroup-v2 + Bubblewrap; network/secret/fs denial; capped CPU/memory/PIDs/stdout/runtime; durable cancellation, kill/reap, host-crash recovery receipts; and independent installed-app acceptance.

### WO6-A — Build/test control path

Reuse the existing supervisor D3 `JobPlan`, fixed Cargo recipes `rust_check`, `rust_tests`, `lu_ide_host_tests`, one-use grant and OS worker implementations. **Do not create another subprocess framework** or allow project manifest hooks to run outside containment. Connect a host-only native dispatcher to exactly one admitted frozen snapshot, with a sealed output view keyed by project ID, plan SHA-256, input snapshot SHA-256, source head, toolchain digest, worker job ID and terminal receipt. Unsupported targets remain unavailable. D1 `execution_intents` are historical nonexecuting review data and never act as grants. Never infer CI pass, source success, or installed success from a mere `JobPhase::Completed`.

### WO6-B — Debug Adapter Protocol foundation

The initial `host/dap_session.rs` adds:

- Incremental `Content-Length` CRLF framing, hard header/body/buffer limits, duplicate/invalid header rejection, and bounded in-flight requests.
- Whitelisted initialize, threads, stackTrace and disconnect client messages. Correlates every response to the single outstanding request sequence and expected command.
- Strong phase transitions. Rejects unexpected responses, adapter-supplied reverse requests (`runInTerminal`), arbitrary commands, untrusted event types and suspiciously large thread/stack frames; quarantines the session on protocol error.
- Never emits an adapter `launch`, `attach`, `evaluate`, `setExpression`, `runInTerminal`, conditional breakpoint, arbitrary command or PATH executable. Terminal output, arbitrary adapter paths and expressions are not passed through to the WebView.
- Does **not** mistake `disconnect` success for OS process-tree cleanup; only native worker and its durable broker can attest kill/reap. This is a **protocol parser**, not a debugger launcher or OS protection result.

### WO6-C — Native debugger admission (outstanding)

Independently identify a redistributable/pinned debug adapter per OS with reviewed license/dependencies, exact SHA and installer provenance. Bind a fresh D3 plan to exact project source, binary debug artifact, adapter executable and host-approved debug target. Some debug adapters execute user code and run-in-terminal helpers: never grant such requests or inherit interactive account secrets. A debuggee must remain inside the same supervised worker boundary (or a separately proven nested protected Job/cgroup) including all process descendants. Native `launch`, attach-to-arbitrary-PID, command REPL/evaluate and adapter-owned spawn are release-blocked unless explicitly implemented with new policy and OS containment proof. Begin with read-only stack/thread inspection of a fixed, already-contained test debuggee; never silently substitute simulated events for native evidence.

### WO6-D — Installed desktop and hostile acceptance (outstanding)

Prove actual Windows installed app and delegated Linux desktop launching native authorized workers, build/test progress, cancellation, worker termination, IDE restart reconciliation, and real debugger stopped state. Test malicious DAP frames/terminal requests, project path swaps, file races, inherited credentials, hostile build.rs scripts, network egress attempts, cgroup/Job descendant escape, output limits and stale source revision. Keep Run/Test/Debug buttons disabled or explicitly unavailable until exact verified OS permissions and installed release packages exist. WO7 signed packaging and WO8 end-to-end installed acceptance remain separate.

## Work order source evidence

- `ide/host/src/dap_session.rs`: bounded DAP incremental parser and fail-closed state machine, independent unit tests; **not attached** to OS processes.
- `.github/workflows/ide-wo6-build-test-dap.yml`: cross-OS host DAP+build intent tests and supervisor contract regression, does not claim native admission.
- No separately licensed debug adapter or installed debugger is shipped; this source branch deliberately cannot auto-launch one.

**Remaining original WOs at branch start:** WO4, WO5, WO6, WO7, WO8, WO9, WO10 (seven still open; WO6 now actively under development). Stop/finish gating is evidence-based rather than green-source-CI-only.
