# Lu IDE P04 — Installed release closeout

**Started November 7, 2026.** Branch ide/release-closeout-p04-20261107 is stacked on P03 draft PR #80. P03 native security verification remains open; no P03 approval is inferred from moving on.

**Status: P04 SOURCE WORK ACTIVE; NATIVE INSTALLED RELEASE NOT ACCEPTED.** P05 and deferred WO10/P07 are outside scope.

## Real native installed evidence inspected

| Work area | OS observations (November 7, 2026) | Result |
|---|---|---|
| WO8 Windows unsigned NSIS | Actual installer built/installed, then WebView2 UI Automation exposed only the top-level document and no "Execution requests" accessible control. Actual run 42392835630, job 128463857896 | FAIL: installed UI, editor, folder picker unproved |
| WO8 Linux .deb | Actual Debian install and GTK/X11 window focus passed; native AT-SPI failed to find the registered fixture project following folder-picker interaction. Actual run 42392835622, job 128463841177 | PARTIAL: graphical package launch proven; editor/save/restart not proven |
| WO9 Windows | Actual unsigned NSIS installed, but the same native accessibility tree blocks inspected Git review/ZIP actions. Actual run 42392835681, job 128463856114 | FAIL: no native portfolio/Git release evidence |
| WO9 Linux | Actual .deb installed, but OS project-picker/AT-SPI navigation fails before native ZIP and Git review. Actual run 42392835681, job 128463856182 | FAIL: no native portfolio/ZIP release evidence |
| WO7 signed package | Real installer, signer certificate, service and DLL license review are not supplied. Production release manifest remains releaseApproved=false. | BLOCKED: no signed installed release, no real acceptance |

These are current native failures, not simply pending CI. The installed Windows UI tree contained only a few named nodes; changing tests to invoke a backend directly would bypass the real UI and is not acceptable. The Linux fixture must actually select a project through the OS picker before the editor and workflow tests can pass.

## P04 source implemented

1. **WO7 signed smoke correctness:** The existing manual WO7 signed Windows installer workflow called a mandatory native test fixture without its required desktop SHA-256 and signer thumbprint. Both are now supplied from the reviewed candidate manifest. This only fixes missing test inputs; no signer or package was created.
2. **Windows signed reinstall and user catalog:** Test-P04SignedReinstall.ps1 can run ONLY in disposable elevated native Windows CI, with authentic pinned Authenticode installer/service/desktop, a fixed Program Files path and no preexisting Lu IDE installation. It registers a real project through the installed UI, measures the user-data projects.json and source-file hashes, uninstalls, verifies service removal, reinstalls the same signed build, reopens the previously registered project without invoking a new picker, confirms original catalog/source hashes, and uninstalls. The new P04ReinstalledProject native UI scenario prevents fake project registration. This witness has NOT run; the absence of signed artifacts prevents success.
3. **Manual signed artifact test:** ide-p04-signed-reinstall.yml requires an explicitly reviewed source commit and signed candidate run, the existing restricted release environment and candidate verification. No automatic release signing or installation on a consumer computer. Passing a clean reinstall would NOT prove a different-version upgrade.
4. **Cross-version evidence verifier:** verify_p04_transition_evidence.py requires exact OS/workflow/job/source identifiers, actual signed old/new candidate SHA-256, source and catalog preservation, real version change for an upgrade, original signed version restoration on failed-upgrade rollback, and verified service/process cleanup with no automatic replay. Synthetic complete metadata NEVER grants release; p04-transition-evidence.json is empty/default-deny. Hostile unit tests enforce rejection. There are NO two-version upgrade, rollback, Linux signed reinstall or failed-upgrade native OS acceptance runs yet.
5. **Exact-source source checks:** ide-p04-source-acceptance.yml runs Windows/Linux Python hostile release/migration evidence fixtures and parses native PowerShell scripts. A green result only demonstrates source checks, not signed, installed or OS acceptance.

## Important security and product boundaries

- WO7 NSIS intentionally refuses in-place upgrades when an existing signed installation or protected service is present. Leave it blocked until a transactional migration and rollback design is reviewed and tested with two **distinct**, independently signed and licensed versions.
- Default unsigned development builds cannot turn on protected Run/Test/Debug or satisfy signed feature acceptance. Do not make the application a package manager, auto-download third-party components, weaken AppContainer/SCM or bypass native consent and OS picker.
- Actual signed WO7 packaging requires independently selected signer/certificate, matching full PE direct/transitive and dynamically loaded dependency inventory, genuine notices/licenses and signed binaries. There is no acceptable substitute in source-only CI.
- OS installation, real user UI operations, durable worker/cancellation/crash/restart, file/digest preservation and service teardown must all be proven at exact final artifact SHA. Source presence and a launched window are insufficient.
- The signed native reinstall witness exercises **same-version clean reinstall**. It does not exercise a failed upgrade transaction, an automatic rollback or migration across changing schema versions. Those remain explicitly open.

## Remaining P04 work and gates

- [ ] Fix real Windows WebView2 native accessibility and complete installed project picker, source edit/review/save/restart, external writer conflict, Git/ZIP workflow and clean uninstall evidence.
- [ ] Fix Linux installed GTK/AT-SPI native folder choice and complete installed project registration/editor save/restart/conflict and reviewed Git/ZIP evidence.
- [ ] Review all transitive/dynamically loaded runtime PE imports and authentic third-party license notices; provide independently reviewed signer, signed NSIS/service/desktop and actual binary SHA receipts.
- [ ] Prove protected signed installed Run/Test/Debug with per-turn consent, native OS containment, cancellation, service/host crashes, restart reconciliation and source tamper denial.
- [ ] Execute signed-only Windows user-state reinstall witness and independently inspect all observed OS/service/file signatures.
- [ ] Develop and verify two-version upgrade plus forced failure, safe rollback to original signed version and user-state preservation. Keep in-place upgrades explicitly denied until then.
- [ ] Verify actual authenticated GitHub run/job evidence, independent OS receipts and exact installed package signatures, then reassess fail-closed release manifests. Do not promote releaseApproved without all gates.

P04 must remain draft/open while these items are outstanding. Stop at this phase; do not move to P05 without further instruction.

## Native accessibility repair checkpoint (November 7)

The actual Windows CI tree had only 20 native descendants and three named nodes even though the installed WebView2 process remained alive. Research into Tauri/wry WebView2 UIA orphaned fragment roots (tauri-apps/tauri issue 15624) describes a native renderer HWND whose provider can remain reachable directly while the host top-level traversal dead-ends. The P04 installed UI witness now uses the real Win32 EnumChildWindows API scoped to the exact installed desktop window to enumerate at most eight Chrome_RenderWidgetHostHWND children, then inspects their actual OS UI Automation provider trees. It does NOT inject JavaScript, control the WebView through a debug port, issue Tauri commands or declare a missing control successful. This is an acceptance-harness workaround, not a claimed upstream accessibility fix. Actual OS retest must prove it works.

The Linux installed AT-SPI fixture previously tried the first globally accessible button named Select/Open/Choose Folder, which could incorrectly match an unrelated application control. It now requires a real AT-SPI native dialog/file-chooser ancestor before clicking an OS dialog confirmation; missing controls remain denied with only fixed-vocabulary role counts. Real OS confirmation, project catalog registration and source editor operations still require an exact native rerun.

The earlier P04 source/negative matrix run 42396238371 passed on SHA a34812cf. WO7 signed source/negative preflight also passed on that SHA. Neither result is installed UI, signed artifact or new native handler acceptance.

**Current P04 result:** implementation/review continued; native installed Windows/Linux retests and independently signed release evidence remain OPEN.

### Further P04 installed OS fault isolation — November 7

Native prior-head Windows test run [42396592041](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42396592041) observed exactly one WebView2 renderer HWND but a 20-element host UIA tree and no navigation heading after app interaction. This corroborates the orphaned top-down fragment problem; native OS foreground UIAutomation focus-chain traversal is now attempted **only after verifying the installed desktop owns foreground**, never against another app. This is a **native acceptance hypothesis**, not a pass.

Native prior-head Linux test run [42396592113](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42396592113) confirmed a real GTK dialog confirmation succeeded, but the expected project still was not in the accessibility tree. Fixed-vocabulary native acceptance diagnostics now distinguish an absent persisted Tauri catalog, the intended project registered, the parent registered incorrectly, an unexpected project registered, or a corrupt catalog. This does not substitute for the required native GUI controls or weaken the trusted folder picker.

### P04 release dependency closure hardening

The original WO7 PE import scanner resolved a private imported DLL if a sibling file existed—even when that file was absent from the release's SHA-pinned license/notice inventory. P04 now requires **every staged EXE and DLL**, including transitive imports such as the observed Windows `python310.dll`, to be included in the exact staged-tree inventory with verified SHA-256 and independently copied, digest-verified license notice. Unreviewed, changed or missing private DLLs fail closed; 4 new hostile fixtures cover all cases. This only establishes reviewed **static** import and notice closure; dynamically loaded modules and independent license approval remain open.

### P04 native source root-cause fix — November 7

Native OS Windows results on [run 42399940004](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42399940004) established that the actual installed desktop was foreground and the WebView2 renderer HWND existed, yet the tree became empty specifically after clicking the Job requests tab. Inspection of the actual React code then identified four `undefined === undefined` project ID comparisons in protected build/debug conditional JSX. With no active project, they wrongly render and dereference absent `buildStatus.current` or `debugStatus.current`, causing a React render crash. P04 now uses a shared `belongsToSelectedProject` guard with exact, nonempty identities and three Node regression tests. The WebView native UIA fallback remains a test aid; this repairs the actual app's rendering bug. Exact native installed Windows retest is still required.

The [Linux native OS run 42399939982](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42399939982) established a genuine GTK dialog accept action but the dialog remained open and the Tauri project catalog had **not been written**. GTK SELECT_FOLDER can interpret a first accept as navigating into a directory rather than finalizing folder choice. P04 now executes at most three genuine dialog-scoped AT-SPI confirmation actions, **only while** an actual file chooser remains open; if none closes, deny instead of pretending project registration. The project and app-data catalog are still verified afterward. Exact native Linux retest is required.

### Two-distinct-signed-version Windows transition source

`ide/app/fixtures/Test-P04SignedVersionTransition.ps1` and manual, protected `ide-p04-upgrade-native.yml` now support an independently signed old and new Windows candidate from different exact source SHAs. A disposable elevated runner must verify real Authenticode installer/service/desktop pins, install and register a genuine old-version IDE project, uninstall the old signed product, install a genuinely newer product version, re-open the persisted project through native UI, verify unchanged catalog and source hashes, inspect LocalService, and cleanly uninstall. A separately labelled `InjectedRecovery` scenario installs the new version and then exercises rollback to the exact previously reviewed, signed old version. This is **two-version sequential replacement and post-install fault recovery**, not an in-place NSIS upgrade and **not an actual failed-installer rollback**. The WO7 NSIS hooks still deny in-place upgrades; missing signed candidates prevent execution. The P04 release evidence checker deliberately refuses to elevate `InjectedRecovery` to a real broken-upgrade witness. The release flag remains OFF.

### Manually signed Debian two-version native acceptance source — November 7

The Linux test-only script `Test-P04SignedDebTransition.sh` and reviewed dispatch-only workflow `ide-p04-linux-upgrade.yml` now require two different actual Debian versions, separately SHA-pinned package bytes, independently supplied detached GPG signatures and a signer key/fingerprint from the protected `lu-ide-release-witness` environment (the key is not checked into GitHub). It installs the signed old package, tests actual GTK project registration/editing, upgrades the real `.deb` with `dpkg`, and reopens the persisted project from the new installed version through AT-SPI. An independent matrix mode deliberately downgrades after an injected post-install failure to test restoration of the signed prior build. It checks Debian package IDs and versions, filesystem and catalog hashes and actual package uninstall. **It is not a genuine failed-installer rollback**, nor automatic migration governance; no signed Linux artifacts, reviewed key or native result exist yet. The existing `wo8_linux_editor_atspi.py` gained a bounded `WO8_P04_REOPEN_ONLY` path that never re-registers or edits a project during verification.

Windows also has `Test-P04SignedVersionTransition.ps1` and `ide-p04-upgrade-native.yml`, requiring genuinely distinct signed NSIS releases, installed LocalService identity, catalog/source stability, and controlled recovery. Its version replacement is sequential uninstall/install, not the in-place installer support still prohibited by WO7 hooks. **All signing/release manifests retain default-deny.**

### P04 native signed transition witness repair (November 7)

The review-only Windows two-version workflow downloads the old/new signed candidates into exact GitHub workspace directories `signed-old` and `signed-new`, whereas its own signed transaction host helper originally allowed only `RUNNER_TEMP`. The source witness now admits **only** those two exact reviewed workspace artifact locations (or the fixed isolated CI temporary root), never arbitrary user-provided directories. The Debian GPG-signed transaction helper now passes literal `dpkg-query -f='${Version}'` and `-f='${Status}'` field format tokens instead of expanding them as undefined Bash variables under `set -u`. Both are source/acceptance repair only; two signed versions, actual native transitions and user data preservation are still awaiting real reviewed binaries and OS results.

### P03/P04 stacked branch synchronization

P04 draft PR #84 is stacked on P03 draft PR #80. On November 7, P03 repaired its invalid native E2 protected-clangd workflow, two Rust 1.99 format-only native-host differences, and added fixed-vocabulary debugger response-error diagnostics. P04 branch now carries the **identical content for these P03 files** to prevent a later P04 merge from undoing upstream security/source repairs. This is a source synchronization, **not** a merge, rebase, native OS acceptance or P03 closure; independent exact-head P03/P04 CI gates are still required.
