# Lu IDE — Development changelog

- **B7 project workspace continuity (Nov 8):** Source-implemented per-project selection, split-pane state and bounded tab focus across native folder switches, with strict live-tab/project validation. Closing or unregistering a project invalidates view memory; stale cross-project editor state cannot be active. Six Node source tests added and five direct V8 pure-source smoke checks passed. Full Node native/installed OS GUI acceptance OPEN. No auto-open or private draft data copy.

- **B6 actual Ubuntu26 GTK/X11 failure repair (Nov 8):** Real native runner #42518812684 passed 117/117 frontend tests, built genuine unsigned Debian with notices and opened installed GUI, but GTK chooser failed because bare Xvfb lacks EWMH _NET_ACTIVE_WINDOW. WO8 installer test now starts/reaps OS Openbox under same Xvfb/DBus, confirms _NET_SUPPORTING_WM_CHECK before real AT-SPI chooser input, and refuses unsupported sessions. Exact-head installed retry, Win2022/Ubuntu24 native source tests and B4/B5 physical performance receipts remain OPEN. Separate repo queue maintenance run #42518984700 independently verified terminal cancellation of 34 superseded Lu Core checks (none from protected current heads), 267 further eligible deferred.

- **IDE B6 late native read and unsaved-draft race repair (Nov 8):** Isolated B6 source branch atop B5 #147 adds synchronous explorer read reservation, post-IPC tab-capacity/identity checks for direct and saved-search opens, matching native response project/path, late directory-project checks and fail-closed source reload on changing CodeMirror/draft revision. Five Node regressions committed; native/frontend execution and installed Windows/Linux race acceptance not yet proved. Prior B4/B5 required exact-head checks and physical installed memory/latency JSON receipts remain OPEN.

- **B5 genuine installed performance evidence harness (Nov 8):** Extended existing real B4 unsigned Windows NSIS/Linux Debian acceptance to use the same actually installed package for a separate 48-source native UI workload, OS process-descendant memory samples, clipboard-verified file-open and tab-switch round trips, typing/Undo/explicit Save and 49th-tab denial. SHA-scoped JSON receipts are uploaded only when real observations exist. Current native OS execution remains queued/pending, so installed acceptance and actual speed/memory findings remain OPEN.

- **IDE B5 large-project editor (Nov 8):** Added 48-tab/project and 96-global bounds without losing unsaved drafts/Undo, 12-button tab window/jump, 120-row directory increment, direct source open beyond truncated native 400-entry listings, lightweight parsing for >128Ki text and memoized split diagnostics. Added 10k mock tab-window benchmark and exact-head Win/Linux source CI. Genuine installed performance evidence remains open.

- **IDE B4 installed test harness (Nov 8):** Added real unsigned Windows NSIS/UI Automation and Linux Debian GTK/AT-SPI two-editor OS acceptance, verifying distinct CodeMirror views, unsaved isolation, targeted secondary Save, focused Ctrl+S and uninstall. Replaced trivial clean-checkout permissions check with exact B2-ancestor source diff. Native/installed CI still required before release or feature acceptance.


- **IDE B4 split editor (Nov 8):** Added independently editable side-by-side open-source tabs with separate CodeMirror identities, selection-safe swapping, secondary Save, focused Ctrl+S routing, responsive stacked layout and project-scoped invalidation. Pure tests + native Win/Linux build gates included; genuinely installed two-pane UI acceptance remains OPEN.


- **IDE B2 native installed harness (Nov 8):** Added genuine Windows NSIS UIA and Linux Debian GTK AT-SPI/X11 installed editor scenarios for OS picker, session/durable draft clipboard, private checkpoint, process restart, explicit recovery, no implicit source Save and actual package uninstall. Private Unix vault permission checks hardened, autosnapshot pauses on full immutable history. These native jobs must pass before installed acceptance; signing/legal release approval stays deferred.


- **IDE B2 durable drafts (Nov 8):** Ported PR #43 session-only revisioned CodeMirror buffer identities and Unicode/CRLF/Undo/Save rebase tests to current stack. Added app-private immutable, fsynced SHA-bound draft checkpoints, CAS stale-writer denial, bounded retention, explicit recover/discard, durable clipboard refs and private opt-in continuous backup. No implicit source Save, worker or Lu Core IPC. Native Windows/Linux + real installed UI acceptance remains pending; original PRs #43/#45 retained.


- **IDE B3 current-stack port (Nov 8):** Ported original PR #46 bounded saved-source Find in Project to the current advanced Lu IDE branch. Native capability-rooted search and Unicode/CRLF/security tests retained; added typed source.findProject read operation, actual Tauri handler, 42-capability parity and React CodeMirror results pane with saved revision rechecks, dirty-draft preservation and stale cross-project response suppression. Dedicated native Windows/Linux exact-head source/desktop compilation workflow added. Installed UI acceptance and original B2/B3 source consolidation remain open; no auto-save, Lu execution or signing claims.


- **DEV-02.6B reviewed installer resource closure (Nov 8):** Hardened production native release verifier against binary-only clangd approval, symlinked source parents, forged native DLL/resource SHA and destination collisions. Both Windows NSIS and Linux Debian stagers now preserve independently reviewed original LLVM/Clang resource files inside the worker bundle and include their bytes in existing full-tree runtime SHA pins; synthetic cross-OS staging tests expanded. Original upstream asset verification and true protected runtime execution still require independent CI evidence and signing/legal review.

- **DEV-02.6B original native resources + protected protocol (Nov 8):** Draft PR #141 stages complete authentic SHA256-locked native release archive file trees (Clang DLL/resource siblings included) and notices within native worker quota, with cross-OS hostile archive/byte-identity CI. Added real protected three-language Rust fixture for kernel Job/cgroup-owned LSP initialize, diagnostics, semantic queries, cancellation and durable journal restart; independent self-hosted OS runners required for acceptance. Only JSON receipts; no unreviewed production bundles, signing/legal approval or installed GUI claims.

- **DEV-02.6A native closure research (Nov 8):** Draft PR #140 introduces Windows PE import/delay-import inspection and Linux ELF dependency/glibc/RPATH inspection of original SHA-locked native server archives, plus bounded actual Windows/Linux CI-only `--version` probes and hostile fixture regressions. Original library layout/hashes are recorded; JSON-only evidence. New exact-head native CI pending. Real protected LSP, LLVM resources, release approval, genuine installed desktop and final signing/legal gate remain open.

- **DEV-02.5 test-only native package (Nov 8):** Created a separate PR #138 stacked on DEV-02.4 for SHA256-locked original release candidate ZIP packaging with original clangd/rust-analyzer/Ruff notices; disposable Windows/Linux hash-install readback and hostile tamper tests; no binary artifacts uploaded. Native exact-head results pending. No approved redistribution, production LSP execution, full LLVM dependency closure, genuine signed NSIS/deb installed acceptance or legal signoff claimed.

- **IDE closeout native improvements (Nov 7):** Linux real delegated `clangd` PASS on prior exact head; fixed Windows native clangd CI PowerShell and immutable P05 38-workflow dual-ledger pins. Added bounded LLDB-DAP alternate error schema classification, strict GTK chooser button captions and exact installed Windows native consent root/InvokePattern safety tests. Source commit only until new-head native Windows/Linux CI and installed GUI are observed; legal/signed release false.

- **IDE 10 P02/P06 evidence then P03/P04 source repair (Nov 7):** Windows P06 55f70b6 native evidence archived 521 package inventory entries, zero missing package notices; Ubuntu P02 source/installed and P06 inventory were queued at inspection, and seven special MPL/r-efi plus separate native/binary distribution legal reviews remain open. On separate draft continuation branch stacked on P06, repaired a malformed Windows protected-clangd PowerShell PE-import check and added exact inline PowerShell AST parser CI; tightened genuine Windows owned native consent dialog to Window/Pane OS UIA without auto-approval; fixed disposable transactional upgrade worker count type confusion with hostile upgrade/rollback tests. Exact new-head native CI, protected AppContainer/DAP, Linux installed, signed installer and legal release acceptance remain OPEN; P05 cleanup deferred.


- **P06 WebView2 runtime obligation (Nov 7):** Kept original MIT wrapper text separate from mandatory three-binding default-deny Microsoft runtime redistribution review, with frozen policy and hostile tests. No legal clearance.

- **P06 selectors MPL full legal text (Nov 7):** Pinned canonical complete MPL-2.0 SPDX v3.28.0 text, checked exact selectors 0.38.0 frozen cargo checksum and preserved mandatory original covered-file source and legal review status. No legal approval inferred.

- **P04 rollback interrupted during rollback (Nov 7):** Added fsync-journal `rolling_back` state and genuine child process death at journal/rejected/previous restore boundaries; resumable exact digests, user-state preservation and worker-quiescence source tests. Signed installed release still gated.

- **P06 dlopen2 complete source MIT originals (Nov 7):** Pinned immutable historical commit containing both dlopen2 0.8.2 and dlopen2_derive 0.4.3 manifests; complete original copyright MIT text source-attested and bundled only for exact versions. Native CI pending.

- **P06 seven native library/SDK notice restorations (Nov 7):** Pinned full upstream MIT and Apache sources for libappindicator-sys, jni-sys-macros, three Android NDK packages and two winapi GNU import libraries. Seven independent native library binary redistribution reviews remain open.

- **P06 Apple SDK policy source audit (Nov 7):** Pinned original objc2 upstream policy across 19 exact packages and source tags; independent Apple SDK derivative/copyright/full linked terms review remains open. No commercial clearance inferred.

- **P06 WebView2 Rust binding original MIT notices (Nov 7):** Three exact native crate tuples pinned to immutable upstream MIT blob; the Microsoft runtime is separately reviewed and release remains denied.

- **P06 Protobuf wrapper vs Google binary legal provenance (Nov 7):** Retrieved original MIT license from upstream protoc-bin-vendored v3.3.0, pinned to nine exact resolved Rust crates with fail-closed source hashes. Added separate nine-entry required embedded Google-built protoc binary redistribution review; MIT wrapper recovery never grants distribution clearance. Tests and CI pending.

- **P06 exact upstream alloc-stdlib/Tauri plugin notices (Nov 7):** Pinned upstream release tags/manifests and complete original BSD-3-Clause, MIT, Apache-2.0 legal texts, staged only on exact missing-notice packages and audited all dual-license originals with immutable Git hashes. No legal signoff; CI evidence pending.

- **P06 actual original r-efi license notices (Nov 7):** Version-tagged 5.3.0 and 6.0.0 `AUTHORS` original MIT/Apache/LGPL option and copyright statements retrieved from upstream, preserved in tree and verified against exact Cargo registry crate bytes via pinned Git blob, with fail-closed modified/wrong-version/symlink tests. Dual-option human review stays open; new OS unresolved count pending.

- **P06 target-specific third-party license prioritization (Nov 7):** Added exact native filtered Cargo dependency graph + notice inventory SHA-bound scope on Windows/Linux, with no legal exemption for absent platform targets or vendored binaries. Latest Windows archive has 45 unresolved entries after three source notice repairs; all remain open until reviewed. Added hostile regression tests and original metadata artifacts for CI review.

- **P05/P06 native persisted project lifecycle (Nov 7):** Real Windows installed source/save passed then reopening failed because the project was closed; added genuine UIA/AT-SPI Reopen → Open status → Edit flow and source regression tests. Native acceptance pending.

- **P06 source lockfiles committed (Nov 7):** Added exact real Windows/Linux reviewed Cargo.lock (500 crates) and package-lock.json (62 npm packages) to Git source, LF-pinned with .gitattributes; removed temporary read-only lockfile log exporter; enforced npm ci/Cargo --locked source graph and hash checks. No signed/installed release approval.

- **P06 commercial-license obligations audit (Nov 7):** Recorded five actual MPL-2.0 crates and two multi-licensed `r-efi` versions, requiring explicit legal/notice review without incorrectly treating an LGPL `OR` choice as mandatory. Exact-name/version metadata frozen into hostile review gate; no license or release approval inferred.

- **P05 native WO9 upstream receipt verification repair (Nov 7):** Inspected exact-head Win/Linux failure receipts: WO9 Windows repeated upstream source fetch; WO9 Linux omitted it. Corrected both actual packaging jobs, added complete CI wiring regressions across six E2/WO8/WO9 jobs; real installed UI acceptance remains open.

- **P06 source governance begins (Nov 7):** New P06 branch stacked on P05 draft; independent Win/Linux actual resolved dependency graph CI with npm/Cargo lock hashes, original upstream license archive and source SHA receipts; default-deny six-phase OS/security/release acceptance matrix and hostile tests. No committed lockfile/independent signer/legal review or installed release approval claimed. GitHub rulesets returned empty but protection endpoint forbidden; repository permissions not inferred.

- **P05 complete unsigned verification artifact license reach (Nov 7):** Extended exact upstream full-text fetch+hashed receipt/actual-package hash to both E2 Windows, WO8 Win/Linux and WO9 Win/Linux package builds. Repaired all corresponding CLI 2.12 pins; preserved exact older workflow blob identities, native installed tests and no-release policy.

- **P05 version and upstream license closeout (Nov 7):** Pinned full Rust/JS/CLI Tauri 2.12 family after actual cross-platform `tauri 2.11.5` vs `tauri-runtime` 2.12 compiler failures. Added fail-closed Cargo.lock drift tests. E2 native NSIS now retrieves, hashes, compares and bundles all six canonical upstream original license texts with per-source exact SHA provenance, plus npm/Cargo notices. Missing/mismatched upstream receipts denied; transitive legal review and real installed OS gates remain open.

- **P05 native CI triage and Tauri version guard (Nov 7, 2026):** Exact-head P05 Windows/Linux source matrix passed at `85cf291`, but both real unsigned Windows E2 NSIS builds failed due Cargo semver drift to `tauri 2.12.5` vs pinned `@tauri-apps/api 2.11.1`. Pinned Rust `tauri = =2.11.5`; added reusable fail-closed Rust/JS/CLI compatibility validation and eight hostile tests to exact-head P05 CI. Kept original E2 installed tests, notices, provenance and default-deny release policy; native retest pending.

- **P05 final operations disposition (2026-11-07):** All 30 previously open IDE PRs and 38 native/source/signed CI workflows were individually retained for stated source and acceptance reasons. Machine-readable P02/P03/P04 CI owner/retention matrix and five hostile regression checks added. No safe PR closure, retarget, CI delete or trigger reduction is authorized until native/installed signed release acceptance; source review work complete without falsely satisfying external gates.


- **P05 final historical source audit (2026-11-07):** Verified 15 WO9-ancestral commits, reviewed all seven late-divergent patch groups, archived PR #70 native proof without promoting it to current-head OS acceptance, and isolated #43/#45/#46 unported B2/B3 features and deferred #76 WO10. Machine-readable semantic source ledger and hostile Windows/Linux CI added. No unsupported PR or security workflow closure; operational cleanup remains gated on P03/P04 native/signed release evidence.


- **P05 E2 installed CI repair (Nov 7, 2026):** Investigated E2 Windows NSIS native CI failure: Tauri's P04 `verification-notices` resource was absent in two older smoke build workflows. Added prebuild actual resolved licenses and exact-package SHA/license receipts to both E2 variants, kept all installed UI test steps and strengthened missing-artifact denial. Audited original and replacement workflow Git blob SHAs; new native OS retest still required. P05 phase remains source audit complete / PR-ops reductions gated behind native acceptance.


- **P05 — GitHub source and CI consolidation inventory (Nov 7, 2026):** Created separate P05 branch stacked on P04; audited all **30 currently open IDE PRs** (78 total open repository PRs) against P04 exact Git tree, pinning original heads and unique source SHA-1s. #43/#45 buffer identity, #46 Find in Project remain unported; #76 WO10 explicitly deferred. All 38 IDE workflows remain preserved and source-pinned; Windows/Linux P05 hostile inventory CI rejects premature PR closures, fake native evidence and dropped source/security gates. Semantic code/CI comparisons, native P02–P04 acceptance and resulting individual PR/CI reduction remain outstanding; **zero PRs closed and zero workflows disabled**. [P05 report](docs/IDE_P05_PR_CI_CONSOLIDATION_20261107.md).


- **P04 source licensing and OS rollback extension (Nov 7, 2026):** unsigned native Windows and Linux package verification includes resolved npm/Cargo legal notices plus exact package SHA receipts. Added a native Linux `dpkg` post-install failure-injection and old-package recovery test (no signature required, real package manager and preserved user-state digest). Signed WO7 builder requires complete independently reviewed legal evidence as a separate input outside the clean source tree. Source implemented; OS/installed CI outcomes and protected installer upgrades still open.


- **P04 licence notices and crash-resilient redundant versions (Nov 7, 2026):** Retrieved upstream LLVM/Python/Tauri/CodeMirror/esbuild full license texts; resolved npm/Cargo license collector and SHA-pinned notice verifier integrated into actual Windows NSIS/Linux Debian unsigned verification packages, including GitHub artifacts and an installed/package inventory presence gate. Signed WO7 builder now requires independent legal inventory review. Added fsynced OS filesystem transaction with retained prior version, exact hashes, manual recovery and real subprocess-abrupt-exit fault tests across switch phases. **Fixture-only OS mechanism** until protected NSIS/dpkg/SCM integration; P04 native installed GUI and final release approval still open. Signing identity deferred. [P04 ledger](docs/IDE_P04_INSTALLED_RELEASE_CLOSEOUT_20261107.md).


- **P04 — installed release and transition closeout in progress (November 7, 2026, PR #84):** Native Windows unsigned NSIS installed **Shell navigation and real OS folder-picker discovery PASSED** on an earlier P04 commit after fixing a no-project React Jobs view render crash. Installed editor tests then failed **at a malformed double-backslash CI disposable fixture path prefix**, now repaired. Linux .deb GTK/X11 launch and authentic AT-SPI dialog confirmation passed earlier, but no project was persisted because GTK folder chooser did not close after three confirmations; fixture now selects the exact disposable child directory in the **real native chooser** before approval. Signed-only Windows NSIS same-version reinstall, reviewed two-version sequential replacement/injected recovery, and GPG-signed Debian transition harnesses are in source; no approved signer, signed artifacts, full dynamically loaded library license review or actual signed OS acceptance exists. A static private DLL audit now requires SHA-pinned transitive dependencies and verified notices. **P04 remains DRAFT/OPEN; production releaseApproved=false**, two-version *in-place* upgrade and real failed-installer rollback not demonstrated. [P04 evidence](docs/IDE_P04_INSTALLED_RELEASE_CLOSEOUT_20261107.md).


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


- **IDE P02–P06 release closeout (Nov 7/8, draft):** Real Windows WO9 TaskDialog accessibility observation led to a guarded OS-only Text/Pane caption pointer witness (pending native acceptance). Strengthened executable signing preflight for valid trusted Code Signing certificate chain and private key. Filed actual 521-package, zero-missing-notice unsigned NSIS artifact technical receipt and independent legal/signature review checklist. This is *not* a signed release or legal clearance.

- **Native IDE OS closeout (Nov 7/8):** Corrected WO9 installed Windows fixture's Git-HEAD versus worker-count baseline shadowing following real native consent and stage/commit success. Added fail-closed regression; restricted Linux ZIP destination acceptance to actual OS chooser. Added an explicitly non-equivalent Ubuntu 26.04 installed GTK editor/rollback/ZIP diagnostic while 24.04 jobs remain queued. Publisher signing and final independent legal sign-off deferred without release bypass.

- **GUI native failure-driven repairs (Nov 7/8):** Real Ubuntu26 dpkg failure/restore and installed window PASS; narrowed GTK chooser to bottom-right real Select Folder and guarded native selection fallback after three non-closing OS actions. Windows WO9 installed WebView2 exposed only one UIA descendant on one run; added bounded real OS keyboard accessibility focus retries. Both require new exact-head OS acceptance; no protection or signing gate weakened.

- **Native GUI diagnostic (Nov 8):** Observed actual Windows WO9 dialogs and WebView native focus recovery, but exact Git commit HEAD was unchanged at immediate check; now waits bounded 24s for real hash change and checks diff-tree. Linux GTK native chooser distinguishes visible active window from stale hidden AT-SPI nodes and requires exact registered project independently of dialogs. No native acceptance yet; default-deny signing unchanged.

- **Linux installed GTK (Nov 8):** Real X11 file chooser exposed no button actions after successful unsigned `.deb` and native dpkg rollback. Added narrowly scoped X11 keyboard confirmation only for an AT-SPI-visible and title-identified chooser; genuine project registration remains mandatory. OS retest pending.

- **Installed GTK follow-up (Nov 8):** After X11 active-window lookup failed on real Ubuntu26 CI, added OS xdotool click limited to the AT-SPI-visible native file chooser geometry, followed by exact Tauri GUI registration proof; installed acceptance still pending.

- **DEV-02.2 protected multilingual LSP source and real CI repair (Nov 8):** PR #131 stacks on DEV-02.1; connects existing Linux root-owned Bubblewrap/delegated cgroup-v2 supervised worker to opt-in Tauri language-server commands, alongside Windows independently signed LocalService/AppContainer. Adds rust-analyzer and clangd diagnostics/completion/hover/same-file definition pathways, Python Ruff diagnostics-only and explicit denial of unreviewed Pyright semantics, per-provider original notice/full-tree hash staging and manual truly installed Windows/Linux acceptance workflows. The first exact-head Win/Linux jobs failed JSX parse (fixed) then stale legacy Windows-only safety test guards (fixed); subsequent frontend run passed all 86 tests but Windows independent host test was incorrectly given `--locked` despite having no committed host Cargo.lock (workflow repaired). Most recent workflow YAML parse failure from literal escaped newline fixed. **Current exact-head Windows/Linux native Rust and genuine installed OS acceptance still open**; owner legal review, signing identity, approved distribution bundles and self-hosted protected runner remain explicitly deferred, not approved.

- **DEV-02.3 LSP offline approval-intake / Python semantic assessment (Nov 8):** New separate draft atop DEV-02.2 introduces deterministic offline review-packet generation for genuine Windows/Linux clangd/rust-analyzer/Ruff binary+original notice SHA256 evidence; unsafe paths, symlinks and incomplete provider sets fail closed. Every release, copyright, license, signature and OS containment approval flag remains false; no provider executable is installed or launched. Evaluates BasedPyright/Pyright Node-runtime semantic alternative against Astral ty native beta, retaining Ruff diagnostics-only until true reviewed provider acceptance. New Win2022/Ubuntu24 Python 3.13 CI and hostile path tests; source tests and actual native installed acceptance pending. User-deferred legal identity/signing unaffected.

- **DEV-02.3 upstream original notices and installed Windows broker integrity (Nov 8):** Original LLVM LLVM-exception, rust-analyzer MIT/Apache and Ruff license notice Git blobs pinned at release refs; introduced fail-closed source-byte verification and original-notice artifact capture, without legal approval. Repaired manual protected installed NSIS gate to invoke the independently audited `Install-ProtectedRecoveryService.ps1` rather than direct `sc.exe create`, require signed installed host and service under Program Files, and verify real SCM cleanup. Static acceptance contract now checks service SIDs/private ACLs in the real installer. Actual signed hardware/runner acceptance still OPEN. Native original upstream archive checksum and notice workflows queued; never claim completion for queued jobs.

- **DEV-02.4 real Python semantic candidate benchmark (Nov 8):** Added test-only exact-version BasedPyright 1.40.2 and official SHA-pinned ty 0.0.99 Windows/Linux CI, 20 source fixtures, genuine stdio JSON-RPC initialized language servers and counted real diagnostic/hover/completion/same-file definition responses with bounded process termination and p50/p95 measurements. This does NOT authorize Python semantic actions in installed Lu, does not select a provider or change the unapproved binary/release manifest. Real exact-head OS results and legal/runtime/containment review pending; Ruff remains diagnostics-only.


- **DEV-02.4 real Python benchmarking failure repairs (Nov 8):** First installed-on-CI candidate server comparison #42494557323 returned real 20/20 hover+completion on both BasedPyright/ty OS paths, 20/20 ty definitions on both; BasedPyright 20/20 definition on Ubuntu, 0/20 accepted on Windows. ty's terminal flag was false on both OS. Added strict native `file:` canonicalized same-file identity checks and bounded mismatch classification; separated OS reaping from clean LSP shutdown/exit code/forced kill in response receipts, with independent hard acceptance for clean exit. Actual final-head rerun remains pending. These are not production Lu installed workers.


- **DEV-02.4 upstream LSP shutdown contract repair (Nov 8):** Verified ty's own real LSP fixture sends `shutdown` → `exit` → closes client stdin → waits. The first Lu real comparison did not close stdin; corrected actual benchmark order and added unit coverage proving bounded native protocol EOF and nonzero-exit still never qualifies as clean. Real Win/Linux rerun pending; no installed Lu language-worker authority granted.
