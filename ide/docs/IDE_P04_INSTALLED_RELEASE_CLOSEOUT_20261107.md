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
