# Lu IDE — Development changelog

- **P03 — native security development & honest acceptance (November 7, 2026, PR #80):** Restored the malformed E2 real Windows/Linux protected-clangd workflow (previous current heads never scheduled it), repaired two native Rust 1.99 formatter diffs affecting Windows/Linux kernel probe CI, and extended native LLDB-DAP debug-only response diagnostics without leaking project paths. Previously verified real Linux cgroup-v2/Bwrap containment, Linux clangd editor operations, debugger abrupt-host-crash kernel cleanup/journal quarantine, and Windows protected Cargo/SCM crash recovery remain positive **on earlier commits**. The current exact-head E2/WO6 tests are queued; Windows LLDB-DAP AppContainer startup still reports permission denial and Linux LLDB launch still returns adapter error. P03 is **OPEN**, not release accepted. Evidence: [P03 native ledger](docs/IDE_P03_NATIVE_SECURITY_ACCEPTANCE_20261107.md), [phase ledger](docs/IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md).


- **P01 source baseline consolidated (Nov 7, 2026):** [draft PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78) on `main`, candidate imports latest nondeferred IDE source plus canonical P01–P07 ledger. Verified only `ide/` and 32 IDE workflows differ from main; all Lu Core/Desktop/docs/CI preserved; WO10 absent. Audited 26 IDE PRs, recorded unported B2/B3 branch features and other exceptions. **P01 source-stage done; P02 not started; native Win/Linux and release validation still red.** [Audit](docs/IDE_P01_INTEGRATION_AUDIT_20261107.md).


- **Release Closeout P01–P07 tracking (2026-11-07):** Created canonical [phase ledger](docs/IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md) with P01 authoritative branch/source reconciliation and historical PR coverage, P02 deterministic native build/test repair, P03 WO4–6 kernel/LSP/DAP security, P04 WO7–9 signed installed acceptance, P05 GitHub PR/CI reduction, P06 reproducibility/governance, and **P07 deferred until after Lu Core v1.0**. User requested one-phase-at-a-time stop and concise completion/outstanding report. WO9 still native-red; older B2/B3 parallel PR source not assumed incorporated.


- WO9-D/E/F native repairs and restart revalidation (Nov 7 UTC): fixed installed Linux Tauri Rust E0277 by deriving strict `Deserialize` on native one-file Git staged receipt; excluded injected coordinator State from frontend IPC request-schema test; added real Windows/Linux `native-build` feature compilation. Hardened durable DAG reload: previously passed steps require exact native supervisor journal revalidation and human-triggered no-rerun verification before subsequent protected job authorization; crash quarantine stays fail-closed. Expanded signed Windows installed WO9 harness with real slow Cargo cancellation, OS worker reap, host crash/restart and no-auto-replay tests. Host Win/Linux + frontend previously green on `16fb0ad`; latest exact-head native install and signed tests **not accepted**. See [WO9 ledger](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md).


- WO9-D/E/F (Nov 7 UTC): implemented reviewed one-file local Git stage/commit with SHA-pinned buffered Git blob, conditional old-HEAD branch ref, native OS stage/commit confirmations and hostile drift/hook/submodule/symlink controls; durable fsynced multi-project dependency DAG with manual existing WO6 protected OS worker requests and crash-quarantine/no replay; actual installed Windows NSIS UIA Git/portfolio and Linux Debian Openbox/AT-SPI ZIP-readback/uninstall suites; manual independently signed Windows/Linux OS-protected two-step DAG acceptance; default-deny release metadata and negative tests. Repaired inherited WO8 native test executable-basename and Linux missing-window-manager bugs on WO8/WO9. **Source implemented; exact native OS and genuine signed installed acceptance remain open; WO10 bridge disabled.** [WO9 ledger](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md).


- WO9 advanced packaging/multi-project implementation started (Nov 7 UTC): new project-scoped Git/verified-artifact workflow planner (up to eight native-picker registered projects), deterministic per-project fingerprint, drift/fail-closed verification, explicit native-folder fsynced no-overwrite JSON manifest export, portable ZIP of bounded independently rehashed artifact files with postwrite revision check and no Git/script execution. Enabled Packages & Workflows GUI and Tauri IPC for named host operations, added Git2 real fixture, ZIP readback and negative tests plus Win/Linux CI workflow. Git writes, dependent protected execution and installed package/release acceptance remain open; WO8 and WO7 OS gates unchanged. [WO9 ledger](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md).


- WO8 source repairs (Nov 7 UTC): persisted project-scoped Cargo and LLDB supervisor journal now visible on installed desktop after restart; full quarantine/reconciliation security state kept outside bounded display; actual signed-native Cargo and DAP cancellation/host crash/service crash/source swap acceptance scripts and fail-closed per-gate metadata are committed. Native CI/release still open; no signed candidate provided. [WO8 ledger](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).


- WO8 native failure repair and installed E2E expansion (Nov 7 UTC): repaired actual WO7 two-vs-three-binary signed-test fixture fault in both WO7/WO8 branches; added Windows real external file-save conflict harness, Linux native AT-SPI installed editor Save/restart/conflict, signed Windows real protected Cargo cancellation, desktop crash, LocalService crash/restart and source-tamper denial with observed worker PID/job/receipt; added manual exact-head native workflow, bounded Jobs view reconciliation receipts, expanded fail-closed evidence gates. **Actual signed installed OS acceptance OPEN**, CI status must be checked against current head. [WO8 work order](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).


- WO8 native installed E2E implementation extended (Nov 7 UTC): CodeMirror's real accessibility textbox, Windows black-box review/Save and durable restarted project reopen, independent Linux Debian+GTK graphical launch and uninstall workflow, manually authorized signed-candidate GUI protected Cargo/LLDB native OS check, and fail-closed exact-SHA/platform/receipt release evidence matrix with hostile tests/source syntax CI. The native Windows/Linux installed jobs and genuine signed protected-feature release acceptance remain OPEN. [Ledger](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).


- WO8-A/B native Windows installed acceptance started (Nov 7 UTC): stacked draft branch `ide/wo8-installed-e2e-20261107`; added real unsigned NSIS install and UI Automation host, OS picker, tabs/source-open, read-only digest and no-worker-autostart checks, uninstall and dedicated Windows OS CI. Declares no native result until exact-head CI; WO8-C–F editing persistence/Linux/signed protected OS acceptance open. [Work-order evidence](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).


- WO7-B/C/D/E source implementation (Nov 7 UTC): committed fixed reviewed Windows payload staging, no-extra-file Tauri handoff, PE regular/delay imports audit, release-only Tauri NSIS hooks/config, privileged LocalService service lifecycle, SHA-pinned native Authenticode signing and NSIS assembly, signed candidate preflight, actual Windows NSIS + SCM installed smoke fixture/manual exact-artifact workflow, and fail-closed exact-head release evidence consistency. Added hostile Python source tests plus Windows PowerShell syntax check; source test verdict must be taken from **current** PR #73 run, not stale earlier green. No approved LLDB adapter, real cert, installer package, native protected UI execution, or final release receipt. See [WO7 ledger](docs/IDE_WORK_ORDER_07_SIGNED_NSIS_RELEASE_20261106.md).


- **WO7-A signed NSIS source preflight started (2026-11-06 local):** new stacked branch `ide/wo7-signed-nsis-release-20261106`, protected Windows installer/service SHA and signer validation, exact SHA review records, default-deny manifest and hostile cross-OS tests/workflow. No binary signed or staged; upstream WO6 native DAP, final third-party license inventory and protected installed application remain OPEN. [WO7 ledger](docs/IDE_WORK_ORDER_07_SIGNED_NSIS_RELEASE_20261106.md).


- Started WO4 Linux delegated containment (Nov 6–7, 2026): new stacked draft PR #68 and [dedicated ledger](docs/IDE_WORK_ORDER_04_LINUX_DELEGATION_20261106.md). Linux worker requires a pre-provisioned empty cgroup-v2 parent with host in separate `lu-ide-host` leaf, enabled CPU/memory/PID controllers, exact empty kernel `populated 0` evidence, and fail-closed cleanup semantics. The standalone probe now tests the same delegation contract. Prior native Windows WO1–3 acceptance remains unchanged. **Seven work orders remain (WO4–WO10).** WO4 Linux native delegated execution acceptance pending.


- IDE 07 protected Windows acceptance and closeout (Nov 6–7): historical exact head `750638e` yielded native behavioral PASS for protected SCM handshake/READY, independent overlapping NUL grants + revoke, host-crash fsynced recovery and fresh-nonce retry, service-loss Job kill/refused false recovery, and real confined offline Cargo completion/published output/restart. Red workflow exit discovered to be missing SYSTEM/Admin SCM DELETE right and expected-negative-probe `$LASTEXITCODE`; corrected both, made privileged uninstall fail if `sc delete` fails. Fixed D7B Windows crash fixture's reserved `$PID` name. **New exact-head complete all-job green and protected Cargo cancellation remain open.** Ledger: `docs/IDE_WORK_BATCH_02_20261106.md`.


- IDE 07 WO2/WO3 (Nov 6): native CI proved Windows exact-user pipe opened, then service image/kernel token authenticated under query-only restricted-SID check; service refused owner before READY. Implemented narrow, authenticated-client impersonation for verified process PID, creation, pinned executable and TokenUser checks, reverting to restricted LocalService before any privileged effect. Fixed D7B independent owner-crash fixture dispatch; applied six exact Rustfmt hunks. Latest exact-head native Cargo/SCM/crash acceptance pending. See `docs/IDE_WORK_BATCH_02_20261106.md`.


- IDE WO2/WO3 acceptance extension (2026-11-06): added a disposable Windows protected-SCM two-Job overlapping NUL lease and independently verified release fixture, staged after the original authenticated pipe smoke test with fixed-service-only bootstrap and explicit revocation attempt. No blanket AppContainer rights or release enablement; exact-head native CI, SCM pipe admission and service/owner crash gates remain open.


- IDE WO2/WO3 live CI continuation (2026-11-06): repaired Windows `E0433/E0425` by restricting SCM recovery module to `protected-recovery-service`; applied all 31 exact native Rustfmt hunks from Windows job 128235540208. Both latest observed D7D crash/service-loss native fixtures failed *before* owner admission, not inside cleanup; added secret-safe fixed-stage stderr collection so CI diagnoses the actual protected pipe denial. Native protected Cargo/service acceptance remains open.


- IDE 07 / WO3 (2026-11-06): fail-closed retry on subsequent unverified named-pipe instances preserves already authenticated kernel Job-holder threads; initial instance ACL verification remains a hard startup gate. CI-only MIC classification added for access-denied triage, without changing service/host authority. Exact-head native acceptance is still pending.


- IDE 07 / WO2 native security hardening (2026-11-06): serialized service-process NUL DACL transactions across concurrent protected Job leases; exact-mask attestation before revocation and fail-closed collision behavior; Windows lock contention regression test. Existing LocalService-owned per-job intent/grant/revoke and restart reconciliation are source-implemented, NOT exact-head accepted. Historical disposable-CI contained Cargo/cancellation run `1090800` passed; protected-service pipe admission and owner/service crash proofs still open. See `docs/IDE_WORK_BATCH_02_20261106.md`.


- WO2 native Windows preparation: fixed-service, elevated-only NUL device DACL bootstrap (service SID only, exact rights, no AppContainer wildcard ACE), compiled but not invoked or release accepted; durable per-job service grant/revoke still pending (2026-11-06).

- WO2/WO3 closeout: D7B native pinned Windows Cargo actual AppContainer build/output and cancellation tests PASS on 1090800 (CI-only NUL); production NUL broker remains open. WO3 protected service pipe denies all access even for matching approved user; added token-restriction diagnostic, bypass-resistant exact ACE verification and optional diagnostic fixture that does not suppress real SCM failure (2026-11-06).

- WO2/WO3 code closeout continuation (2026-11-06): capability-rooted quota scanning replaces mutable 40-ms output hashing; final full hash/seal still post-Job. Corrected `cap_fs_ext::OsMetadataExt` native E0624; added bounded cancel-vs-walltime-vs-output traces. CI-only D7D exact-user DACL control and current TokenUser diagnostic isolate protected service access denial without weakening ACL. Protected recovery service handler concurrency capped at 12 with RAII session release, keeping held kernel Jobs from being dropped on client flooding. Applied native Rustfmt exact diffs. All actual pinned Cargo and SCM hostile acceptance remain open; see `docs/IDE_WORK_BATCH_02_20261106.md`.


- IDE WO2/WO3 active native repairs (2026-11-06): separated mutable output live quota from post-reap immutable output seal; bounded scans at 250 ms and kept cancellation at 40 ms; fixed `cap_fs_ext::OsMetadataExt` native compiler `E0624`. D7C rooted rename acceptance now tests byte identity for either/both safe aliases; independent D7B crash witness captures native stderr on admission failure. D7D CI-only native client identity/pipe-rights probes and a disposable exact-ACL access self-check isolate genuine LocalService pipe admission denial without security bypass. Pinned Cargo, SCM recovery and release NUL remain OPEN; see `docs/IDE_WORK_BATCH_02_20261106.md`.


- IDE WO2/WO3 continued closeout (2026-11-06): corrected native named-pipe DACL object type; kept the original kernel Job for a 60-second durable protected recovery ACK retry; added independent native host-crash repeat-ACK and eventual Job release test. Disposable NUL compatibility now refuses release compilation, while the native Cargo worker refuses unproven release NUL access before authority is consumed. Work orders remain OPEN until genuine protected SCM / pinned Cargo OS acceptance. See `docs/IDE_WORK_BATCH_02_20261106.md`.


- WO3 protected-service IPC strengthening: `d65c202` verifies actual named-pipe kernel DACL before announcing SCM RUNNING; exact SYSTEM, restricted service SID and installer-approved host SID only, with client pipe-create permission forbidden. The prior native D7D host pipe open still denied despite user SID pinning; CI-only access-mask diagnostics (`3539186`) await new exact-head verification. No service acceptance falsely promoted.

- WO2/WO3 native progress: real Windows D6 AppContainer isolation fixture passed behavioral checks at job `128199718599` (only Rustfmt failed; `6fc0b36` repaired); D7C/D7D crash-observer C# Job Object accounting fixed from 32-byte incomplete ABI to actual 48-byte Win32 ABI (`958609d`), eliminating `ERROR_BAD_LENGTH` cause. Protected SCM client still denied opening pipe at job `128199760472`; fixed-scope CI-only read/write/control probes added (`3539186`). Orders 2 and 3 remain open for real pinned Cargo, crash-safe production NUL and independently verified protected service cleanup/restart. Source: `ide/docs/IDE_WORK_BATCH_02_20261106.md`.

- WO2/WO3 further source hardening: NUL CI compatibility requires explicit `ci-native-null-lease` **compile-time** feature plus disposable debug CI environment; D7C/D7D hostile observers now surface native Win32 Job query failures rather than ambiguous empty/denied evidence. Production capability provisioning and independent SCM crash recovery still open. Draft PR #65, no release enablement.


- IDE WO2/WO3 resumed engineering (2026-11-06): AppContainer network timeout acceptance requires dual live host endpoint controls, no child connections, exact token capabilities and verified Job empty; temporary `NUL` permission confined to explicit disposable CI per-profile SID and revoked before success, not release accepted; D7D service pipe binds installer-approved individual host SID and versioned service config, confirms endpoint before SCM RUNNING, repairs privileged installer AST syntax, and tightens real kernel Job crash witness. Related supervisor core Windows/Linux unit CI passed on prior exact head, but current AppContainer Cargo, SCM crash recovery, and production NUL failover are not accepted. See `docs/IDE_WORK_BATCH_02_20261106.md`.


- IDE first three-work-order batch (2026-11-06): repaired Windows E2 HANDLE writer thread ownership, feature-gated native example compilation, real Tauri frontend build and interrupted recovery test; added strict child token-capability verification and bounded protected SCM pipe-denial diagnostics. Source commits `2076532`, `0e941c7`, `3c38f5e`. Native CI and protected-service acceptance remain outstanding; see `docs/IDE_WORK_BATCH_01_20261106.md`.


- E2 exact native Windows failure repair (2026-11-06): corrected hostile inside-root child-rename **test assumptions** without permitting a junction/secret escape; applied native Windows Rustfmt 1.99 formatting to service token, AppContainer network probe and crash witness. Recorded Cargo's still-blocking AppContainer `NUL` device denial (exit 101; compiler never launched), distinguishing it from a missing rustc or a successful native sandbox check. Also identified cancelled-checkout manifest errors as cancellation artifacts. Source-only commits `940d395`, `01950f1`, `ed7c038`, `5052622`; native exact-head results and installed-app security remain open.


- Native CI repair closeout: inspected historical D7D/D7B/D7C/E1/AppContainer job logs; fixed Rust unsafe-block parsing and Win32 threading token imports, pinned offline VS 17.14 jobs to Windows 2022, installed Rustfmt in Rust 1.99.1 CI, applied native E1 rustfmt diffs, and classified a previously ambiguous AppContainer network timeout without granting acceptance. Added exact-head failure ledger `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`.

- Native E2 repair: reproduced CI `PermissionDenied` in the hostile LSP snapshot test (fixed test-only write permissions), applied formatter-provided host and supervisor Rustfmt changes, and added bounded exit/reporting for failing D7B Cargo plus debug-only authenticated SCM admission stages for failing D7D without changing kernel security decisions.
- Added real unsigned standalone Windows NSIS install/launch/restart/uninstall smoke fixture and CI workflow, deliberately separate from signed E2 language-server release acceptance. Release remains disabled; hosted Linux cgroup delegation and Windows OS gates outstanding.

## 2026-11-06 — Wave E2 protected language-service integration (source, native pending)
- Extended protected Windows desktop source from clangd-only to fixed native rust-analyzer, clangd and Ruff language selection; only per-language verified installation hashes unlock buttons. Ruff is restricted to diagnostics by both UI and native host.
- Added isolated supervisor tests for durable successful close, cancelled session and failed service release quarantine after restart; added native route rejection tests.
- Added signed-service and license-verified NSIS staging, per-machine installer config overlay, malicious resource/no-overwrite tests and queue coalescing for E1/E2 GitHub CI. No server binary is shipped and OS acceptance remains open.

- Exercised actual local clangd 17 LSP protocol for initialization, diagnostics, hover, definition and completion; committed sanitized JSON-RPC transcript and fixed bounded completion-prefix textEdit handling with explicit human insertion. No native protected-worker execution claim.
- Introduced real LSP session wire processor: initialize, safe document sync, bounded diagnostics/completions/hover/same-file definition, strict pending/revision checks and rejection of server-initiated execution or edits.
- Added optional internal supervisor transport endpoint requiring trusted running D3 job, same plan SHA, OS containment proof and verified tree cleanup; **no unrestricted process spawn or active native LSP child**.
- Wired CodeMirror marker effects, problems/suggestions/hover panels, same-file jump and human-selected completion insertion, native read-only worker status, language server catalog page and UTF-16/stale result guards.
- Added offline installer candidates and mandatory license notice, dependency, asset hash and release provenance audit; **all assets remain unbundled and unapproved**. New Rust/Node/Python tests and cross-platform E2 CI; real Windows/Linux LSP and D7D OS acceptance pending.

## 2026-11-06 — Wave E1 free language-server discovery and wire protocol (draft)
- Added standalone host catalog of eight free open-source LSP providers across Rust, C/C++, Python, JS/TS, HTML, CSS and JSON, with fixed executable hints and license/upstream identity. Discovery cannot run a program.
- Added bounded JSON-RPC stdio framing parser/encoder, strict length/JSON-RPC envelope checks and source-revision diagnostic contract; native Rust and frontend safety tests.
- Exposed read-only `ide_list_language_servers` Tauri command and typed `lsp.catalog` capability schema without executable grant, project access or Lu bridge. Added cross-platform E1 acceptance workflow. Wave D native containment remains open; E1 does not launch servers.

## 2026-11-06 — D7D final OS acceptance matrix expansion (still draft)
- Extended real Windows offline Cargo acceptance example for native protected-service mode, using authenticated service-release ACK rather than reading private evidence, and added a Windows 2025 full Cargo/forced cancellation workflow.
- Added hostile service death test: approved owner detects lost SCM broker, kills the contained Job and refuses to certify a missing original Job on restart.
- Removed invalid hard-coded SYSTEM owner from LocalService-created named pipe DACL. Exact-head native Windows jobs are queued; D6 network proof and installer acceptance remain unresolved.

## 2026-11-06 — D7D protected Windows recovery service (draft)
- Added separately restricted SCM LocalService native broker, mutual kernel-verified named-pipe identities, service-owned direct ProgramData evidence ACLs, bounded request protocol, explicit cross-session Global Job ACL, and fsynced READY/release witnesses.
- Added worker \`protected-recovery-service\` opt-in and two-phase restart reconciliation. Recovery retains quarantine until protected service ACK and original Job's native recheck.
- Added Windows SCM smoke and independently observed owner-crash/Job kill/restarted-client fixtures in PR #63. Exact-head Windows CI and installed desktop acceptance not yet passed; Run remains disabled.

## 2026-11-05 — D6 fixed AppContainer + Job Object admission
- Added per-probe zero-capability AppContainer profile, fixed self-copy staged inside profile, STARTUPINFOEX security capabilities, pre-resume Job Object assignment with kill-on-close/memory/PID limits, token check and host secret-read denial. Failures do not fall back to unrestricted CreateProcess.
- Added Windows CI build/runtime probe/formatting workflow and acceptance boundaries. Fixed prior D4 formatting of Linux cgroup and Windows restricted-token probes from exact CI output. Source/worker and desktop effects remain disabled until actual Windows platform and installed acceptance.

## 2026-11-05 — D5 verified-source staging and Linux native worker adapter
- Added bounded private `PublishedInput` source snapshot copying, checksum revalidation and fixed recipe manifest identity, with symlink/reparse/device denial and CPU-memory storage budgets. No Git SHA is treated as sufficient proof of tested bytes.
- Added opt-in Linux process worker that launches only compiled-in offline Rust recipes through trusted Bubblewrap, after cgroup-v2 CPU/memory/PID quota creation and pre-exec process placement. Read-only snapshot, private output mount, cleared environment and network namespace deny by default. `NoContainment` if any prerequisite is missing; no unsafe fallback.
- Connected host-only D3 one-use grant consumption to admission-pending/running/terminal journal states, and verified-cgroup process-tree cancellation/recovery paths with bounded stdout/stderr and output artifacts. No WebView or Lu Core executable capability added.
- Linux feature compilation/native cgroup hostile tests pending; Windows AppContainer production worker and installed desktop acceptance still not implemented. Central design/acceptance docs updated.

## 2026-11-05 — D4 restricted-rights and cgroup-v2 admission probes
- Added a fixed Windows native SID restricted-token primitive probe with no child execution. This is not an AppContainer or OS rights acceptance for untrusted code.
- Added a fixed Linux delegated cgroup-v2 quota admission probe that checks CPU/memory/PID controllers, attempts bounded settings on a disposable *empty* cgroup, and denies when delegation is absent. A successful workflow can still contain an explicit controlled denial.
- Native Windows/Linux workflow and scope document added; supervisor remains fail-closed. No Tauri or Lu bridge changes.

## 2026-11-05 — D3 supervisor authorization, receipt and recovery control plane
- New standalone Rust `ide/supervisor` crate (no desktop/Tauri dependency): strict fixed-recipe JobPlan, current source and actual input snapshot/toolchain digest fields, bounded resource/effect budgets, fresh per-job desktop-only one-shot short-lived grant.
- App-private append-only receipt journal with sequential SHA-256 chain and transition validation; synced writes; truncated/tampered logs reject startup. A hash chain is not an authenticated/MAC receipt.
- Private native cancellation/recovery broker interface: write cancel before kill, verify all descendants before cancelled; failed kill uncertain/quarantined; restart of in-flight job requires explicit native OS cleanup receipt, never auto-relaunch.
- Default platform admission denies execution; no general worker, no Tauri command or Lu bridge. D1 review notes remain non-executable. Native Windows/Linux unit regression and frontend contract tests added; native containment and installation gates still outstanding.

## 2026-11-05 — Wave D2-W1 Windows Job Object containment probes
- Reused existing experimental `ide/sandbox` crate, without linking it to Lu IDE/Tauri. Added compiled-in Windows probe binary and target-specific `windows-sys` dependency. No general-purpose subprocess or arbitrary project code execution surface is added.
- Fixed acceptance fixtures cover suspended-create/Job assignment/resume, bounded job-memory and process-count limits, deadline kill, and child+grandchild cleanup. Startup environment stripped to SystemRoot/TEMP/TMP. Job Object is killed on handle close; worker is not exposed to users.
- Added separate native Windows-only workflow `ide-sandbox-windows.yml`, documentation and central project handoff pointers. D2 remains experimental, full security and installed-Windows app acceptance open.

## 2026-11-05 — D1 native/frontend CI repair
- Reconciled Rustfmt's 34 exact hunks from first D1 native job. Native Rust tests passed on predecessor but formatter check failed; no process launch was added. `RECIPES` moved to static storage for lifetime clarity.
- Frontend test updated to exclude host-only Tauri `intents` state from WebView request field schema parity. The new D1 UI and typed native schema remained unchanged.
- New D1 exact-head acceptance must run again; do not count predecessor results as passed.

## 2026-11-05 — Wave D1 execution intent protocol (no worker launch)
- Added a separate, bounded, append-only host execution-intent journal and 3 typed Tauri capabilities (`job.prepare`, `job.review`, `job.list`). D1 deliberately records **no execution authority**. Source Git HEAD, worktree cleanliness, fixed recipe and manifest SHA-256 are observed by native code, not submitted by the frontend.
- Short-lived 120-second prepared plans and human review are single-session, with native revalidation; stale, edited, expired, cross-project, replayed and corrupt-record attempts fail closed. A previous review can be withdrawn without dispatch.
- Enabled a human-facing **Job requests** screen with exact source/manifest evidence and cancellation. Explicitly states no executable process, script, model, package install or build/test worker can run.
- Updated capability catalog to 30 operations / 38 schema definitions, existing static native/UI contract tests, and new Rust persistence/replay tests plus frontend UI/schema regression tests.
- **Unimplemented by design:** OS process containment, process-tree cancellation, resource quotas, execution authority, launched worker receipts, crash-recovery reconciliation and installed Windows acceptance. D2 shall use a separate host-owned supervisor; D1 consent notes must NEVER be accepted as executable tokens.
- Branch stacks on C6 accepted native CI source `c403a8ba47d6ac1662b8627ce5ab34a9a16b591e`; D1 exact-head CI pending.

## 2026-11-05 — Wave C6 task evidence and development ledger (source implementation)
- Extended the existing private append-only `DevelopmentLedger` with typed `TaskEvent` payloads and host-backed project task summaries. Human progress edits do not rewrite or claim verified acceptance; they keep source HEAD/dirty state and required gate IDs.
- Added `ide_record_task_update` and `ide_observe_task_ci`, both typed in the semantic capability catalog (27 native commands, 36 definitions). CI is independently fetched by the native host, matched to the current human task event and source SHA, then logged. Stale updates and duplicate/unscoped task IDs fail closed.
- Added Continuity task editor, required gate toggles, task evidence list, link/test action and privacy-sanitized task checkpoint coordinates. No standalone worker, Git mutation, installer claim or Lu bridge.
- New `host/src/task_evidence.rs` Rust tests and `app/tests/task_ledger.test.mjs` Node tests cover task/non-acceptance separation, malformed states, cross-project reads and stale CI links. Exact-head CI acceptance and installed Windows interaction pending.
- **Dependency repair:** C5 source failed only Linux rustfmt after its native tests passed; GitHub formatter diff applied to C5 at `1dd4a58d84d6bc5f383f75e463b15f82baea6de8`. C6 branch starts from that repaired C5 commit. Its own formatting gate remains to be validated.

## 2026-11-05 — Wave C5 exact-source GitHub Actions receipts
- Added native read-only `git.ciExactHead` behind a human-triggered Source control action. It derives only validated public github.com origin, queries bounded fixed api.github.com endpoints for the **exact local HEAD SHA**, checks workflow file paths and required job metadata, rejects partial results and duplicate/missing/other-head jobs, and distinguishes success/failure/running/no evidence.
- Added typed JSON Schema v2 descriptor (25 genuine host operations, 31 nested types) and Tauri command. No credentials, arbitrary HTTP targets, background network access, project mutation, subprocess execution or Lu bridge.
- C4 evidence matrix now uses an explicitly cached same-project, same-SHA receipt if available. C3 checkpoint export copies only a sanitized cached summary; no external query on Copy. No test run proves local dirty draft, installed desktop, or task acceptance.
- Added `host/src/github_ci.rs` classifier unit tests and `app/tests/github_ci.test.mjs` source/schema/reconciliation regression. Exact-head CI and installed Windows acceptance pending. C5 branch stacked on C4 source `ca606dfb378ce31945a768a23e9ab6a31cbe60e8`.

## 2026-11-05 — C4 frontend registry CI repair
- C3 CI found a macro matcher regression: rustfmt moved `capability!` arguments across lines, causing an empty match set despite 24 registered operations. Fixed the JS test regex and added an explicit 24-operation assertion. Native registry/Tauri parity inspected; no host behavior or permissions changed.
- C4 exact-head CI remains pending following this source update. C3 fix also committed independently.

## 2026-11-05 — Wave C4 evidence reconciliation (source implementation)
- Added a read-only evidence matrix to Continuity. Uses existing native host results to classify local Git cleanliness, stored tracking refs, live remote (not queried), bounded artifact hashes/baselines, host events, project-note task claims, CI, installed desktop acceptance and toolchain usability **without promoting guesses to verified status**.
- Explicitly preserves partial scans, truncated change lists, contradictory dirty counts, absent baselines and unknown remote state. The development checkpoint JSON now includes the same bounded conservative reconciliation, but excludes untrusted source/document excerpts.
- Added `app/web/development_reconciliation.js` and `app/tests/development_reconciliation.test.mjs`; source-only, no new host command, Lu bridge, Git mutation, package action or effect grant.
- C4 branch is independently created from Wave C3 exact SHA `2572fa445d31d265b0ef27ef52a1dd4a09156326` to avoid cancelling C3 acceptance CI. Full C4 CI/Windows installed acceptance remains open; local checkout is not inspected.

## 2026-11-05 — Wave C3 bounded development checkpoint export
- Implemented human-initiated **Copy development checkpoint**, with fresh read-only host context + inventory comparison, strict project-ID matching, stale-request invalidation, versioned bounded JSON and explicit non-atomic/provenance limits.
- Exports local Git state, source revision IDs, known artifact bytes/digests, baseline transitions and host event IDs, **not** source text, untrusted document excerpts, event summaries, absolute project roots, passwords, live GitHub contents or execution authority.
- Added `development_checkpoint.js` and Node tests for missing/mismatched project evidence, bounded output, private content exclusion and UI action wiring; local pure tests passed, integration CI and Windows installed acceptance pending. Project-switch invalidation also clears busy-state flags so an older in-flight read cannot leave the new project permanently disabled.
- Isolated Wave C PR #47; predecessor HEAD `26040e8331f5a5b8053f4af8248a88d9d551f04f`. Do not call clipboard/installer accepted before exact-head checks and installed UI acceptance.

This records **what changed**. Read [PROJECT_STATE.md](PROJECT_STATE.md) for what is authoritative *now*, and [CONTINUITY_PROTOCOL.md](CONTINUITY_PROTOCOL.md) before any new development session.

## 2026-11-05 — Wave C first native CI repair (pending recheck)
- First source matrix [42143131954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143131954) confirmed frontend success but failed Linux Rust E0308 in `resource_probe.rs`: `cap_std::time::SystemTime` differs from `std::time::SystemTime`.
- Fixed explicitly with `into_std()` before Unix timestamp conversion; implementation SHA `dbaa04ff41a6c1c20367f88f6c2455af47843370`. New exact-head [CI 42143391542](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143391542) pending; not yet accepted.
- No runtime execution or Lu connection enabled.

## 2026-11-05 — Wave C read-only development truth (source implemented)
- Added scoped native Git staged/unstaged patch and local commit history with typed `git.diff`, `git.history`, explicit Source control UI, 64 KiB/80-file patch cap and 30-commit history cap. Untracked file contents and any remote fetch remain excluded.
- Added `resource.probe` under the registered project directory capability, producing a SHA-256 of inspected bytes or an explicit missing, changed, non-file, or oversize state. No claim that executables/installers work; no tool execution.
- Added native fixture tests, frontend typed-contract checks, human UI and scoped evidence documentation. PR and CI pending; Windows installed acceptance not claimed. B source and acceptance PRs remain separate.
- Design and gates: [IDE_WAVE_C_DEVELOPMENT_TRUTH.md](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md).

## 2026-11-05 — Wave B1 Find and Replace (source committed)
- Implemented Ctrl+H Find and Replace in human editor with literal case-sensitive/insensitive search, Replace One, confirmation-gated Replace All, and an explicit scan cap that refuses incomplete bulk edits.
- Changes apply only to the active unsaved CodeMirror buffer in one undoable transaction, with an expected-document check and consumed nonce to avoid accidental reapplication on remount. Explicit human Save and revision conflict handling remain authoritative; no new native file-write API.
- Added pure-JS tests for literal regex metacharacters, Unicode UTF-16 offsets, case, literal dollar signs and all-or-nothing scan limits; updated frontend structural assertions and CSS.
- **Verification:** code committed; [source CI 42130813013](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42130813013) pending at last inspection. No installed Windows GUI acceptance or installer produced. Note that branch-wide CI concurrency can supersede older native Git runs; a separate feature branch is preferable next time.

## 2026-11-05 — Live GitHub comparison and native Git CI repair
- Implemented typed read-only public GitHub branch comparison and explicit Source control Check GitHub action. Compares local HEAD, locally stored tracking SHA, and time-stamped remote branch SHA separately; does not infer ancestry or execute Git, use credentials, or connect Lu.
- Fixed a genuine Windows Rust E0505 fixture lifetime failure in [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) by releasing the borrowed Git tree before the repository.
- Source `b39c6267`; [CI 42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605) Windows host/native desktop checks passed; Linux and frontend queued at last inspection. No installed Windows acceptance.

## 2026-11-05 — Git status native fixture stabilization
- Changed tracked-file test fixture to have a different byte length after editing, preventing false negatives from timestamp/stat-cache resolution. This is a regression reliability change, not a new runtime feature.
- Source `27e6dde8cf6bd12e6ea2fbe19b27c2fb8f74c89f`; [CI 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) queued at last inspection. No latest exact-SHA CI conclusion or installed Windows smoke proven.

## 2026-11-05 — Human-first Source control pane (source implemented, CI open)
- Replaced sidebar's placeholder Source control section with a read-only local Git status view driven by `ide_read_git_status` Tauri command and versioned `git.status` registry contract.
- Displays branch, local SHA, locally stored upstream tracking status and ahead/behind, real dirty/staged/untracked counts, and up to 120 changed paths; no fetch, push, staging, commit or project-code execution.
- Native-to-schema parity audit confirms 20 operations with 20 matching descriptor schemas; native Git fixture tests and JS static checks added. Latest source `4d1a559d2f37afb062524827a6d23aff6e62f9b5`; **exact-source CI/physical Windows GUI acceptance not yet established.**

## 2026-11-05 — A3 native Git snapshot transitions
- Persist local libgit2 worktree state with each explicit inventory baseline; historical stores remain readable via optional/default fields. Compare the newest baseline against a fresh native status and classify clean → dirty, dirty → clean, both dirty (not equivalent), or unverifiable.
- Real Git fixture now checks baseline cleanliness followed by a modified tracked file and a new untracked file. Source commit `a37be15b808b2d238ac2457aee9e357d056be13f`, [CI 42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued at last inspection.
- The operation is read-only except for user-requested IDE-private baseline storage; GitHub remote HEAD is not fetched, and stored upstream refs can be stale.

## 2026-11-05 — A3 native Git worktree reader (source implemented)
- Added `git2` and `host/src/git_status.rs` for actual read-only local Git branch, staged/modified/untracked paths, and locally configured upstream comparison and ahead/behind. Only the exact native-registered project root may be inspected; parent repository discovery is refused. No shell, Git hooks, network operations, fetch or Git mutations.
- Included true worktree status in the Continuity read model, typed JSON Schema and human UI. Added real native Git fixture tests and frontend source/security assertions.
- **Evidence:** source committed; [exact-source CI 42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending at last inspection. No Windows installed acceptance or live GitHub comparison established.

## 2026-11-05 — A3 historical evidence retention
- Snapshot capture now preserves multiple observations instead of overwriting the previous baseline. Comparisons use the latest snapshot; the UI displays how many baselines exist for the active project. History is bounded to 64 snapshots/1 MiB and refuses further captures rather than silently deleting evidence.
- Added native regression for a second snapshot and newest-baseline comparison. Source commit `d7fd8cfac1d91a1ddc3073e631a249c1f5b1f9a7`; [CI 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258) queued at last inspection. No Windows-installed GUI acceptance or verified global paths.

## 2026-11-05 — A3 root-folder discovery and current CI (source only)
- Root-level installers/archives are now discoverable with SHA-256 when the user selects a standalone `Delivered` folder through the existing native project picker. Added host-native regression fixture for this route; no external scanning or execution rights added.
- Source commit under verification: `7409a3cb54fb6c71d42150b9ab362c3b1b5b7096`. [CI run 42124780433](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42124780433) queued at last inspection. **Do not claim this runtime accepted until Linux/frontend/Windows results complete.**

## 2026-11-05 — A3 evidence snapshot and benchmark context refinement
- Added 128 MiB total SHA-256 scan budget on top of 64 MiB per-file limit; over-budget files remain visible but explicitly unhashed. Local toolchain candidates are `present_unverified`, never assumed usable.
- Extended revisioned, untrusted project context to include four recognized benchmark manifest/result paths; stored source revisions, not rerun results. Baseline tests cover matching, changed, new and missing observed artifact bytes after restart.
- Typed capability directory now advertises **19 real host commands**; schema and native signature parity audited structurally. Latest source CI [42124467605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42124467605) queued at last inspection.
- Out-of-project files, actual Git working-tree diff and live GitHub are NOT inspected by this first A3 implementation. Installer execution / toolchain version checks require future supervised and approved native workers.

## 2026-11-05 — A3 bounded artifact/resource inventory and durable baseline (source implemented, CI open)
- Added `host/src/inventory.rs`: read-only bounded scan of registered project subfolders `Delivered`, `dist`, `benchmarks`, `results`, selected tool/bin folders and NSIS output locations. Captures file size and SHA-256 (up to 64 MiB); skips symlinks and never executes or downloads project code.
- Added read-only local `.git/HEAD` and loose ref/tracking-ref observation. **Not a live remote comparison or authoritative working-tree status**; packed refs/worktree pointers require later support.
- Added `host/src/inventory_history.rs`: explicit persisted evidence baseline, restart recovery and comparison reporting matched/changed/new/missing/unverifiable inspected bytes; no forced cleanup or destructive Git operation.
- Added human Continuity evidence display, `Record evidence snapshot` control, and two typed A2 capabilities. Added native persistence/change/symlink/toolchain/Git fixture tests and frontend contract checks.
- **Not accepted yet:** exact-head native CI [42123874954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42123874954) still queued at last check; Windows installed UI, out-of-project resources, live GitHub and executable toolchain/installer acceptance remain open.

## 2026-11-05 — Wave A2 typed schemas and Wave A3 project continuity (source implementation)
- Refined the capability directory for human readability: concise typed signatures, expandable full request/result/error schemas, and nested types self-contained using JSON Schema 2020-12 `$defs`. Added reference integrity tests.
- Expanded project orientation to support both project-root documents and `ide/` subdirectory documents. Recorded successful source writes now include a structured safe relative path alongside revision digest, rather than an anonymous save.
- Latest source CI [42122003384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42122003384) was queued at last check; no exact-source pass/fail has yet been observed.

- Replaced loose input names/result labels with fully typed JSON Schema contract definitions for native request/result shapes, including nested project/file/span/review/proposal records and explicit error schema. Descriptor schema version increased from 1 to 2; contracts remain discovery-only.
- Implemented host-owned bounded `development-ledger.jsonl` in IDE app data, recording successful native project lifecycle and explicit source save observations with revision when available. Added `ide_read_project_context` and human Continuity screen for recent events and revisioned project note excerpts.
- Context distinguishes human/AI-readable project docs from trusted host observations, labels project text untrusted, and marks remote Git, artifacts and toolchains uninspected/unverified. No Lu bridge, shell execution, automatic save or package operations.
- Added native ledger restart/corruption/access tests and JS typed schema/command-parity/context checks. **Source written; exact-head CI and installed Windows acceptance still open.** Ledger is a best-effort orientation record, not an effect journal.

## 2026-11-05 — Wave A2 Semantic Capability Registry (implemented source; acceptance pending)
- Added bounded, versioned, host-authored descriptions of 16 real standalone IDE commands in `host/src/capabilities.rs`; effect class, input names, preconditions, evidence and error/continuation documentation remain separate from actual execution authorization.
- Exposed read-only `ide_list_capabilities` and a human Capabilities directory; added Rust validation and frontend command-parity assertion.
- Capability discovery is never a model bridge, grant, executor, or claim that proposed future Git/toolchain/build services already exist.
- **Verification:** source committed; exact-head CI and installed Windows GUI checks remain open. No new package or local artifact claimed.

## 2026-11-05 — Wave A1 CI repair and human Copy source reference (unverified)
- Investigated [native CI 42074135705](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42074135705): Windows desktop and frontend passed; Linux Rust host tests completed, then formatting check failed on the new span files. Applied rustfmt-suggested formatting changes.
- Added editor toolbar's `Copy source reference`: verifies currently saved SHA revision and selected cursor line using native `ide_read_source_span`, then copies a machine-readable reference with project ID, relative path, full SHA, line and UTF-8 byte offsets.
- Unsaved drafts are explicitly refused for this saved-disk operation. New UI tests were added; no live Lu connection or file writes.
- **Verification and packages:** exact-head CI after repair remains open; native clipboard UX not physically tested; no new Windows installer generated.

## 2026-11-05 — IDE-ARCH-02 / Wave A1 saved-source span (implemented, unverified)
- Implemented `host/src/source_span.rs` bounded read-only exact-revision source spans: one-based line numbers, UTF-8 byte positions, LF/CRLF terminators and blank/trailing lines. Reuses trusted `preview_source_file` and rejects stale revisions; maximum 120 lines and 32 KiB selected text.
- Added `ide_read_source_span` to standalone Tauri command registry and `host/tests/source_span.rs` native tests (Unicode, CRLF, stale SHA, traversal, invalid range, closed project, Unix symlink).
- **Implementation is disk-only.** Unsaved CodeMirror state is not yet covered; no human-facing reference control, AI connection, execution or implicit Save added.
- **Verification:** source committed; tests added but **not executed in this session**. No new installer, exact-head CI result or physical Windows acceptance established. Local resources / `Delivered` not inspected.

## 2026-11-05 — IDE-ARCH-02 capability-first redesign (design only)
- Added [capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md) defining one authoritative Rust IDE host, shared human/AI semantic operations, revision-aware source identity, development ledger, verified resource/installer inventory, evidence journal and a gated worker supervisor.
- Replaced the old strictly linear stage implementation order with Waves A–G, preserving eight canonical stage identifiers and existing safety gates. Start Wave A1 source-span API, A2 capability descriptors, A3 continuity read model; defer live Lu bridge.
- **Verification:** design/source commits only; no native/Windows acceptance or local artifact inventory performed. These planned services are **not runtime implementations**.

## 2026-11-05 — AIW-IDE-01 architectural alignment (design-only)
- Read the canonical Lu v1.0 reconciliation's `LU-AIW-01` R1–R7 and draft PR #28's operational-memory design/source. PR #28 remains a source-only, unmerged Lu Core feature, not an IDE dependency.
- Added [AI-first IDE design](docs/AI_FIRST_IDE_DESIGN.md) and IDE-specific `AGENTS.md`. Updated the canonical design to mandate precise source identity, semantic capability discovery, verified tool/artifact resources, task context, evidence and recovery.
- **Implemented:** documentation and contributor orientation. **Not implemented:** source-span host APIs, IDE operational inventory, context assembler, capability registry, receipts, any live Lu integration.
- Acceptance: design inspection only; no exact-head CI run or Windows installation test for this change. Local filesystem and `Delivered` package locations not inspected.

## 2026-11-05 — PCP-01 continuity adoption
- Added `CHANGELOG.md`, `PROJECT_STATE.md`, `CONTINUITY_PROTOCOL.md`, and `scripts/check_continuity.py`.
- Added `.github/workflows/ide-continuity.yml` pull-request gate requiring changelog/current-state changes alongside IDE implementation changes. Branch protection required-check configuration and passing workflow run are **not verified**.
- This records real remote state but **does not inventory inaccessible local workspaces or certify package paths**.

## 2026-11-05 — Standalone IDE R3.7 source (not release-accepted)
- Added in-file Find UI: Ctrl+F, active-buffer matching, next/previous navigation, match count, Escape/close. Changed: `app/web/main.jsx`, `app/web/code_editor.jsx`, `app/web/styles.css`, `app/tests/standalone.test.mjs`.
- Last known R3.7 source commit before documentation: `eb0aefdb1f4e954044b353e64b8e308d13b0e6f6` on `ide/workspace-development`.
- Verification: code committed, tests added; **not yet confirmed by a passing exact-head CI run or user Windows test**. No installer claimed.
- Added dismissible green staging banner and preserved amber saved-file notification (R3.6 source, earlier commits on same branch). User requested the notification; updated behavior has **not** received Windows acceptance.
- **Lu-to-IDE bridge deferred by user:** no live connection, no dependency on Lu being ready.

## 2026-11-05 — R3.5 interactive milestone
- Proposal inbox, review, approval-for-staging, editor-only staging, and human Save remain separate operations.
- User reported the staging and save workflow functioning on Windows; user requested improved green staging feedback. This is **interactive user feedback**, not a full native adversarial-test or installer acceptance.
- Earlier CI/release and acceptance references remain in [ENGINEERING_HANDOFF.md](ENGINEERING_HANDOFF.md) and [R3.5 evidence](docs/IDE_07_R35_PROPOSAL_INBOX.md).

## Earlier milestones
- R3.4: multi-hunk review and data-only proposal schema, automated/interactive evidence recorded.
- R3.3: reviewed save and stale-file handling, automated/interactive evidence recorded.
- R3.2: registered-project removal and persisted catalog state confirmed by user.
- R3.1: code editor line numbers and syntax highlighting confirmed by user.
- R3: editor tabs, editing, explicit save, reopening source verified by user.
- R1–R2: standalone Windows shell, projects and read-only source browsing.

See the engineering handoff and linked acceptance documents for exact evidence. Do not use these summaries as substitute for exact commit or artifact verification.
